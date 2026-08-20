use crate::state::AppState;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SpeakerProfile {
    pub id: String,
    pub display_name: String,
    pub source: Option<String>,
    pub fingerprint_json: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameSpeakerResult {
    pub updated_segments: u64,
    pub profile_saved: bool,
}

#[tauri::command]
pub async fn api_list_speaker_profiles(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<SpeakerProfile>, String> {
    sqlx::query_as::<_, SpeakerProfile>(
        "SELECT id, display_name, source, fingerprint_json, created_at, updated_at
         FROM speaker_profiles ORDER BY display_name COLLATE NOCASE",
    )
    .fetch_all(state.db_manager.pool())
    .await
    .map_err(|error| format!("Failed to load speaker profiles: {error}"))
}

#[tauri::command]
pub async fn api_rename_meeting_speaker(
    state: tauri::State<'_, AppState>,
    meeting_id: String,
    speaker_id: String,
    display_name: String,
    remember: bool,
) -> Result<RenameSpeakerResult, String> {
    rename_meeting_speaker(
        state.db_manager.pool(),
        &meeting_id,
        &speaker_id,
        &display_name,
        remember,
    )
    .await
}

async fn rename_meeting_speaker(
    pool: &sqlx::SqlitePool,
    meeting_id: &str,
    speaker_id: &str,
    display_name: &str,
    remember: bool,
) -> Result<RenameSpeakerResult, String> {
    let display_name = display_name.trim();
    if display_name.is_empty() {
        return Err("Speaker name cannot be empty".to_string());
    }

    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| format!("Failed to start speaker update: {error}"))?;
    let now = Utc::now().to_rfc3339();

    let source = sqlx::query_scalar::<_, Option<String>>(
        "SELECT source FROM transcripts
         WHERE meeting_id = ? AND speaker_id = ? LIMIT 1",
    )
    .bind(&meeting_id)
    .bind(&speaker_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|error| format!("Failed to inspect speaker: {error}"))?
    .flatten();

    let update =
        sqlx::query("UPDATE transcripts SET speaker = ? WHERE meeting_id = ? AND speaker_id = ?")
            .bind(display_name)
            .bind(&meeting_id)
            .bind(&speaker_id)
            .execute(&mut *transaction)
            .await
            .map_err(|error| format!("Failed to rename speaker: {error}"))?;

    if update.rows_affected() == 0 {
        return Err("No transcript segments matched that speaker".to_string());
    }

    let existing_profile_id = sqlx::query_scalar::<_, String>(
        "SELECT profile_id FROM meeting_speaker_labels
         WHERE meeting_id = ? AND speaker_id = ? AND profile_id IS NOT NULL",
    )
    .bind(meeting_id)
    .bind(speaker_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|error| format!("Failed to inspect remembered speaker: {error}"))?;
    let profile_id = remember.then(|| {
        existing_profile_id.unwrap_or_else(|| format!("speaker-profile-{}", uuid::Uuid::new_v4()))
    });
    let fingerprint_json = average_speaker_fingerprint(&mut transaction, meeting_id, speaker_id)
        .await
        .map_err(|error| format!("Failed to build speaker voice profile: {error}"))?;
    if remember {
        sqlx::query(
            "INSERT INTO speaker_profiles
                (id, display_name, source, fingerprint_json, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                display_name = excluded.display_name,
                source = COALESCE(excluded.source, speaker_profiles.source),
                fingerprint_json = COALESCE(excluded.fingerprint_json, speaker_profiles.fingerprint_json),
                updated_at = excluded.updated_at",
        )
        .bind(profile_id.as_deref())
        .bind(display_name)
        .bind(&source)
        .bind(&fingerprint_json)
        .bind(&now)
        .bind(&now)
        .execute(&mut *transaction)
        .await
        .map_err(|error| format!("Failed to remember speaker profile: {error}"))?;
    }

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
    .bind(&speaker_id)
    .bind(profile_id.as_deref())
    .bind(display_name)
    .bind(&now)
    .bind(&now)
    .execute(&mut *transaction)
    .await
    .map_err(|error| format!("Failed to save meeting speaker label: {error}"))?;

    transaction
        .commit()
        .await
        .map_err(|error| format!("Failed to commit speaker update: {error}"))?;

    Ok(RenameSpeakerResult {
        updated_segments: update.rows_affected(),
        profile_saved: remember,
    })
}

async fn average_speaker_fingerprint(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    meeting_id: &str,
    speaker_id: &str,
) -> Result<Option<String>, sqlx::Error> {
    let values = sqlx::query_scalar::<_, String>(
        "SELECT speaker_fingerprint_json FROM transcripts
         WHERE meeting_id = ? AND speaker_id = ? AND speaker_fingerprint_json IS NOT NULL",
    )
    .bind(meeting_id)
    .bind(speaker_id)
    .fetch_all(&mut **transaction)
    .await?;
    let fingerprints: Vec<Vec<f32>> = values
        .into_iter()
        .filter_map(|value| serde_json::from_str(&value).ok())
        .filter(|value: &Vec<f32>| value.len() == 6)
        .collect();
    if fingerprints.is_empty() {
        return Ok(None);
    }
    let mut average = vec![0.0f32; 6];
    for fingerprint in &fingerprints {
        for (current, value) in average.iter_mut().zip(fingerprint.iter()) {
            *current += *value;
        }
    }
    for value in &mut average {
        *value /= fingerprints.len() as f32;
    }
    Ok(serde_json::to_string(&average).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    #[tokio::test]
    async fn renames_all_segments_and_remembers_profile() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("CREATE TABLE transcripts (meeting_id TEXT, speaker_id TEXT, speaker TEXT, source TEXT, speaker_fingerprint_json TEXT)")
            .execute(&pool).await.unwrap();
        sqlx::query("CREATE TABLE speaker_profiles (id TEXT PRIMARY KEY, display_name TEXT, source TEXT, fingerprint_json TEXT, created_at TEXT, updated_at TEXT)")
            .execute(&pool).await.unwrap();
        sqlx::query("CREATE TABLE meeting_speaker_labels (meeting_id TEXT, speaker_id TEXT, profile_id TEXT, display_name TEXT, created_at TEXT, updated_at TEXT, PRIMARY KEY (meeting_id, speaker_id))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO transcripts VALUES ('m1', 'remote-speaker-1', 'Remote speaker 1', 'System', '[0.1,0.2,0.3,0.4,0.5,0.6]'), ('m1', 'remote-speaker-1', 'Remote speaker 1', 'System', '[0.2,0.3,0.4,0.5,0.6,0.7]')")
            .execute(&pool).await.unwrap();

        let result = rename_meeting_speaker(&pool, "m1", "remote-speaker-1", "Avery", true)
            .await
            .unwrap();
        assert_eq!(result.updated_segments, 2);
        assert!(result.profile_saved);
        let (name, fingerprint): (String, String) =
            sqlx::query_as("SELECT display_name, fingerprint_json FROM speaker_profiles LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(name, "Avery");
        assert!(fingerprint.contains("0.15"));
    }
}
