use crate::api::{TranscriptSearchResult, TranscriptSegment};
use chrono::Utc;
use sqlx::{Connection, Error as SqlxError, SqlitePool};
use tracing::{error, info};
use uuid::Uuid;

pub struct TranscriptsRepository;

impl TranscriptsRepository {
    /// Saves a new meeting and its associated transcript segments.
    /// This function uses a transaction to ensure that either both the meeting
    /// and all its transcripts are saved, or none of them are.
    pub async fn save_transcript(
        pool: &SqlitePool,
        meeting_title: &str,
        transcripts: &[TranscriptSegment],
        folder_path: Option<String>,
    ) -> Result<String, SqlxError> {
        let meeting_id = format!("meeting-{}", Uuid::new_v4());

        let mut conn = pool.acquire().await?;
        let mut transaction = conn.begin().await?;

        let now = Utc::now();

        // 1. Create the new meeting
        let result = sqlx::query(
            "INSERT INTO meetings (id, title, created_at, updated_at, folder_path) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&meeting_id)
        .bind(meeting_title)
        .bind(now)
        .bind(now)
        .bind(&folder_path)
        .execute(&mut *transaction)
        .await;

        if let Err(e) = result {
            error!("Failed to create meeting '{}': {}", meeting_title, e);
            transaction.rollback().await?;
            return Err(e);
        }

        info!("Successfully created meeting with id: {}", meeting_id);

        // Attach the nearest imported calendar event as local meeting context.
        // This is intentionally deterministic and never reaches a calendar network API.
        sqlx::query(
            "INSERT INTO meeting_context (meeting_id, calendar_event_id, context_json)
             SELECT ?, id, json_object(
                'title', title, 'starts_at', starts_at, 'ends_at', ends_at,
                'location', location, 'join_url', join_url, 'notes', notes)
             FROM calendar_events
             WHERE starts_at <= ?
               AND ends_at >= ?
             ORDER BY starts_at LIMIT 1",
        )
        .bind(&meeting_id)
        .bind((now + chrono::Duration::minutes(15)).to_rfc3339())
        .bind((now - chrono::Duration::minutes(15)).to_rfc3339())
        .execute(&mut *transaction)
        .await?;

        // 2. Save each transcript segment with audio timing fields
        for segment in transcripts {
            let transcript_id = format!("transcript-{}", Uuid::new_v4());
            let fingerprint_json = segment
                .speaker_fingerprint
                .as_ref()
                .and_then(|values| serde_json::to_string(values).ok());
            let remembered_profile = match (&segment.source, &segment.speaker_fingerprint) {
                (Some(source), Some(fingerprint)) => {
                    find_matching_speaker_profile(&mut transaction, source, fingerprint).await?
                }
                _ => None,
            };
            let result = sqlx::query(
                "INSERT INTO transcripts (id, meeting_id, transcript, timestamp, source, speaker, speaker_id, speaker_fingerprint_json, source_overlap, audio_start_time, audio_end_time, duration)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(&transcript_id)
            .bind(&meeting_id)
            .bind(&segment.text)
            .bind(&segment.timestamp)
            .bind(&segment.source)
            .bind(
                remembered_profile
                    .as_ref()
                    .map(|(_, name)| name)
                    .or(segment.speaker.as_ref()),
            )
            .bind(&segment.speaker_id)
            .bind(&fingerprint_json)
            .bind(segment.source_overlap.unwrap_or(false))
            .bind(segment.audio_start_time)
            .bind(segment.audio_end_time)
            .bind(segment.duration)
            .execute(&mut *transaction)
            .await;

            if let Err(e) = result {
                error!(
                    "Failed to save transcript segment for meeting {}: {}",
                    meeting_id, e
                );
                transaction.rollback().await?;
                return Err(e);
            }

            if let (Some(speaker_id), Some((profile_id, display_name))) =
                (&segment.speaker_id, &remembered_profile)
            {
                sqlx::query(
                    "INSERT INTO meeting_speaker_labels
                        (meeting_id, speaker_id, profile_id, display_name, created_at, updated_at)
                     VALUES (?, ?, ?, ?, ?, ?)
                     ON CONFLICT(meeting_id, speaker_id) DO UPDATE SET
                        profile_id = excluded.profile_id,
                        display_name = excluded.display_name,
                        updated_at = excluded.updated_at",
                )
                .bind(&meeting_id)
                .bind(speaker_id)
                .bind(profile_id)
                .bind(display_name)
                .bind(now)
                .bind(now)
                .execute(&mut *transaction)
                .await?;
            }
        }

        info!(
            "Successfully saved {} transcript segments for meeting {}",
            transcripts.len(),
            meeting_id
        );

        // Commit the transaction
        transaction.commit().await?;

        Ok(meeting_id)
    }

    /// Searches for a query string within the transcripts.
    /// It returns a list of matching transcripts with context.
    pub async fn search_transcripts(
        pool: &SqlitePool,
        query: &str,
    ) -> Result<Vec<TranscriptSearchResult>, SqlxError> {
        let fts_query = Self::prepare_fts_query(query);
        if fts_query.is_empty() {
            return Ok(Vec::new());
        }

        let rows = sqlx::query_as::<_, (String, String, String, String)>(
            "SELECT m.id, m.title, t.transcript, t.timestamp
             FROM transcript_fts f
             JOIN transcripts t ON t.id = f.transcript_id
             JOIN meetings m ON m.id = t.meeting_id
             WHERE transcript_fts MATCH ?
             ORDER BY bm25(transcript_fts), t.audio_start_time, t.timestamp
             LIMIT 100",
        )
        .bind(&fts_query)
        .fetch_all(pool)
        .await?;

        let results = rows
            .into_iter()
            .map(|(id, title, transcript, timestamp)| {
                let match_context = Self::get_match_context(&transcript, query);
                TranscriptSearchResult {
                    id,
                    title,
                    match_context,
                    timestamp,
                }
            })
            .collect();

        Ok(results)
    }

    /// Convert arbitrary user input into a safe FTS5 AND query. Quoting every
    /// token prevents punctuation in meeting names or customer terms from
    /// being interpreted as FTS operators.
    fn prepare_fts_query(query: &str) -> String {
        query
            .split_whitespace()
            .map(|token| {
                token
                    .chars()
                    .filter(|character| {
                        character.is_alphanumeric() || matches!(character, '-' | '_' | '.')
                    })
                    .collect::<String>()
            })
            .filter(|token| !token.is_empty())
            .map(|token| format!("\"{}\"", token.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(" AND ")
    }

    /// Helper function to extract a snippet of text around the first match of a query.
    fn get_match_context(transcript: &str, query: &str) -> String {
        let transcript_lower = transcript.to_lowercase();
        let query_lower = query.to_lowercase();

        match transcript_lower.find(&query_lower) {
            Some(match_index) => {
                let start_index = match_index.saturating_sub(100);
                let end_index = (match_index + query.len() + 100).min(transcript.len());

                let mut context = String::new();
                if start_index > 0 {
                    context.push_str("...");
                }
                context.push_str(&transcript[start_index..end_index]);
                if end_index < transcript.len() {
                    context.push_str("...");
                }
                context
            }
            None => transcript.chars().take(200).collect(), // Fallback to the start of the transcript
        }
    }
}

async fn find_matching_speaker_profile(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    source: &str,
    fingerprint: &[f32],
) -> Result<Option<(String, String)>, SqlxError> {
    let profiles = sqlx::query_as::<_, (String, String, String)>(
        "SELECT id, display_name, fingerprint_json FROM speaker_profiles
         WHERE source = ? AND fingerprint_json IS NOT NULL",
    )
    .bind(source)
    .fetch_all(&mut **transaction)
    .await?;

    Ok(profiles
        .into_iter()
        .filter_map(|(id, name, stored)| {
            let stored: Vec<f32> = serde_json::from_str(&stored).ok()?;
            let distance = fingerprint_distance(fingerprint, &stored)?;
            Some((distance, id, name))
        })
        .filter(|(distance, _, _)| *distance <= 0.12)
        .min_by(|(left, _, _), (right, _, _)| {
            left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(_, id, name)| (id, name)))
}

fn fingerprint_distance(left: &[f32], right: &[f32]) -> Option<f32> {
    if left.len() != 6 || right.len() != 6 {
        return None;
    }
    let weights = [0.45, 1.25, 1.25, 0.7, 0.65, 1.35];
    let weighted_sum = left
        .iter()
        .zip(right.iter())
        .zip(weights.iter())
        .map(|((a, b), weight)| (a - b).powi(2) * weight)
        .sum::<f32>();
    Some((weighted_sum / weights.iter().sum::<f32>()).sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    #[test]
    fn prepares_safe_full_text_queries() {
        assert_eq!(
            TranscriptsRepository::prepare_fts_query("budget Q3/2026"),
            "\"budget\" AND \"Q32026\""
        );
        assert_eq!(TranscriptsRepository::prepare_fts_query(" OR "), "\"OR\"");
        assert_eq!(TranscriptsRepository::prepare_fts_query("!!!"), "");
    }

    #[test]
    fn voice_fingerprint_distance_distinguishes_near_and_far_profiles() {
        let base = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6];
        let near = [0.11, 0.19, 0.31, 0.4, 0.49, 0.61];
        let far = [0.8, 0.7, 0.1, 0.9, 0.0, 0.2];
        assert!(fingerprint_distance(&base, &near).unwrap() < 0.12);
        assert!(fingerprint_distance(&base, &far).unwrap() > 0.12);
    }

    #[tokio::test]
    async fn full_text_search_returns_ranked_context() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("CREATE TABLE meetings (id TEXT PRIMARY KEY, title TEXT NOT NULL)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("CREATE TABLE transcripts (id TEXT PRIMARY KEY, meeting_id TEXT NOT NULL, transcript TEXT NOT NULL, timestamp TEXT NOT NULL, speaker TEXT, audio_start_time REAL)")
            .execute(&pool).await.unwrap();
        sqlx::query("CREATE VIRTUAL TABLE transcript_fts USING fts5(transcript_id UNINDEXED, meeting_id UNINDEXED, transcript, speaker, timestamp UNINDEXED)")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO meetings VALUES ('m1', 'Planning')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO transcripts VALUES ('t1', 'm1', 'The launch budget is approved', '10:00', 'Remote speaker 1', 1.0)")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO transcript_fts VALUES ('t1', 'm1', 'The launch budget is approved', 'Remote speaker 1', '10:00')")
            .execute(&pool).await.unwrap();

        let results = TranscriptsRepository::search_transcripts(&pool, "launch budget")
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "m1");
        assert!(results[0].match_context.contains("launch budget"));
    }
}
