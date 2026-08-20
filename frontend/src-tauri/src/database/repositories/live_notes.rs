use chrono::Utc;
use serde_json::Value;
use sqlx::SqlitePool;

pub struct LiveNotesRepository;

impl LiveNotesRepository {
    pub async fn get_state(
        pool: &SqlitePool,
        meeting_id: &str,
    ) -> Result<Option<Value>, sqlx::Error> {
        let state_json: Option<String> =
            sqlx::query_scalar("SELECT state_json FROM live_notes WHERE meeting_id = ?")
                .bind(meeting_id)
                .fetch_optional(pool)
                .await?;

        state_json
            .map(|json| {
                serde_json::from_str::<Value>(&json).map_err(|e| {
                    sqlx::Error::Protocol(format!("Failed to parse live notes JSON: {}", e))
                })
            })
            .transpose()
    }

    pub async fn save_state(
        pool: &SqlitePool,
        meeting_id: &str,
        state: &Value,
    ) -> Result<bool, sqlx::Error> {
        let meeting_exists = sqlx::query("SELECT 1 FROM meetings WHERE id = ?")
            .bind(meeting_id)
            .fetch_optional(pool)
            .await?
            .is_some();

        if !meeting_exists {
            return Ok(false);
        }

        let state_json = serde_json::to_string(state).map_err(|e| {
            sqlx::Error::Protocol(format!("Failed to serialize live notes JSON: {}", e))
        })?;
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO live_notes (meeting_id, state_json, created_at, updated_at)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(meeting_id) DO UPDATE SET
                state_json = excluded.state_json,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(meeting_id)
        .bind(state_json)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;

        Ok(true)
    }
}
