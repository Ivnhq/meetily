use crate::state::AppState;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingCitation {
    pub meeting_id: String,
    pub meeting_title: String,
    pub transcript_id: String,
    pub timestamp: String,
    pub audio_start_time: Option<f64>,
    pub speaker: Option<String>,
    pub quote: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingAnswer {
    pub question: String,
    pub answer: String,
    pub citations: Vec<MeetingCitation>,
    pub grounded: bool,
}

#[derive(Debug, FromRow)]
struct CitationRow {
    meeting_id: String,
    meeting_title: String,
    transcript_id: String,
    timestamp: String,
    audio_start_time: Option<f64>,
    speaker: Option<String>,
    transcript: String,
}

#[tauri::command]
pub async fn api_ask_meetings(
    state: tauri::State<'_, AppState>,
    question: String,
    limit: Option<i64>,
) -> Result<MeetingAnswer, String> {
    ask_meetings(state.db_manager.pool(), &question, limit.unwrap_or(6))
        .await
        .map_err(|error| format!("Meeting Q&A failed: {error}"))
}

pub async fn ask_meetings(
    pool: &SqlitePool,
    question: &str,
    limit: i64,
) -> anyhow::Result<MeetingAnswer> {
    let query = prepare_question_query(question);
    if query.is_empty() {
        return Ok(MeetingAnswer {
            question: question.to_string(),
            answer: "Ask a more specific question using a person, project, decision, or topic from your meetings.".to_string(),
            citations: Vec::new(),
            grounded: false,
        });
    }

    let rows = sqlx::query_as::<_, CitationRow>(
        "SELECT m.id AS meeting_id, m.title AS meeting_title,
                t.id AS transcript_id, t.timestamp, t.audio_start_time,
                t.speaker, t.transcript
         FROM transcript_fts f
         JOIN transcripts t ON t.id = f.transcript_id
         JOIN meetings m ON m.id = t.meeting_id
         WHERE transcript_fts MATCH ?
         ORDER BY bm25(transcript_fts), m.created_at DESC
         LIMIT ?",
    )
    .bind(&query)
    .bind(limit.clamp(1, 20))
    .fetch_all(pool)
    .await?;

    let citations: Vec<MeetingCitation> = rows
        .into_iter()
        .map(|row| MeetingCitation {
            meeting_id: row.meeting_id,
            meeting_title: row.meeting_title,
            transcript_id: row.transcript_id,
            timestamp: row.timestamp,
            audio_start_time: row.audio_start_time,
            speaker: row.speaker,
            quote: row.transcript,
        })
        .collect();

    let answer = if citations.is_empty() {
        "I could not find a grounded answer in the saved meeting transcripts.".to_string()
    } else {
        let evidence = citations
            .iter()
            .enumerate()
            .map(|(index, citation)| {
                format!(
                    "[{}] {}: {}",
                    index + 1,
                    citation
                        .speaker
                        .as_deref()
                        .unwrap_or(&citation.meeting_title),
                    citation.quote.trim()
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!("Relevant evidence from your meetings:\n{evidence}")
    };

    Ok(MeetingAnswer {
        question: question.to_string(),
        answer,
        grounded: !citations.is_empty(),
        citations,
    })
}

fn prepare_question_query(question: &str) -> String {
    let stop_words: HashSet<&str> = [
        "a", "an", "and", "are", "did", "do", "for", "from", "how", "i", "in", "is", "it", "me",
        "of", "on", "our", "the", "to", "was", "we", "what", "when", "where", "who", "why", "with",
    ]
    .into_iter()
    .collect();

    question
        .split_whitespace()
        .map(|token| {
            token
                .chars()
                .filter(|character| character.is_alphanumeric() || matches!(character, '-' | '_'))
                .collect::<String>()
                .to_lowercase()
        })
        .filter(|token| token.len() > 1 && !stop_words.contains(token.as_str()))
        .map(|token| format!("\"{}\"", token.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" OR ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    #[test]
    fn removes_question_filler_but_keeps_subjects() {
        assert_eq!(
            prepare_question_query("What did we decide about Project Atlas?"),
            "\"decide\" OR \"about\" OR \"project\" OR \"atlas\""
        );
    }

    #[tokio::test]
    async fn returns_traceable_meeting_citations() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("CREATE TABLE meetings (id TEXT PRIMARY KEY, title TEXT, created_at TEXT)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("CREATE TABLE transcripts (id TEXT PRIMARY KEY, meeting_id TEXT, transcript TEXT, timestamp TEXT, audio_start_time REAL, speaker TEXT)").execute(&pool).await.unwrap();
        sqlx::query("CREATE VIRTUAL TABLE transcript_fts USING fts5(transcript_id UNINDEXED, meeting_id UNINDEXED, transcript, speaker, timestamp UNINDEXED)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO meetings VALUES ('m1', 'Launch Review', '2026-07-14T09:00:00Z')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO transcripts VALUES ('t1', 'm1', 'Atlas launch is approved for Friday', '09:15', 900.0, 'Avery')").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO transcript_fts VALUES ('t1', 'm1', 'Atlas launch is approved for Friday', 'Avery', '09:15')").execute(&pool).await.unwrap();

        let answer = ask_meetings(&pool, "When is Atlas launch?", 6)
            .await
            .unwrap();
        assert!(answer.grounded);
        assert_eq!(answer.citations[0].meeting_id, "m1");
        assert_eq!(answer.citations[0].transcript_id, "t1");
        assert!(answer.answer.contains("[1]"));
    }
}
