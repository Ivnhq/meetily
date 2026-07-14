use crate::artifacts::{export_meeting_artifacts, export_meeting_to_obsidian};
use crate::state::AppState;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PostMeetingHook {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub destination: String,
    pub folder: Option<String>,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[tauri::command]
pub async fn api_list_post_meeting_hooks(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<PostMeetingHook>, String> {
    sqlx::query_as::<_, PostMeetingHook>(
        "SELECT id, name, kind, destination, folder, enabled, created_at, updated_at
         FROM post_meeting_hooks ORDER BY name COLLATE NOCASE",
    )
    .fetch_all(state.db_manager.pool())
    .await
    .map_err(|error| format!("Failed to load hooks: {error}"))
}

#[tauri::command]
pub async fn api_save_post_meeting_hook(
    state: tauri::State<'_, AppState>,
    id: Option<String>,
    name: String,
    kind: String,
    destination: String,
    folder: Option<String>,
    enabled: bool,
) -> Result<PostMeetingHook, String> {
    if !matches!(kind.as_str(), "artifact_export" | "obsidian_export") {
        return Err("Hook kind must be artifact_export or obsidian_export".to_string());
    }
    if name.trim().is_empty() || destination.trim().is_empty() {
        return Err("Hook name and destination are required".to_string());
    }
    let destination_path = Path::new(&destination);
    if !destination_path.is_absolute() {
        return Err("Hook destination must be an absolute local path".to_string());
    }
    let id = id.unwrap_or_else(|| format!("hook-{}", Uuid::new_v4()));
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO post_meeting_hooks
            (id, name, kind, destination, folder, enabled, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET name = excluded.name, kind = excluded.kind,
            destination = excluded.destination, folder = excluded.folder,
            enabled = excluded.enabled, updated_at = excluded.updated_at",
    )
    .bind(&id)
    .bind(name.trim())
    .bind(&kind)
    .bind(destination.trim())
    .bind(
        folder
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty()),
    )
    .bind(enabled)
    .bind(&now)
    .bind(&now)
    .execute(state.db_manager.pool())
    .await
    .map_err(|error| format!("Failed to save hook: {error}"))?;

    sqlx::query_as::<_, PostMeetingHook>(
        "SELECT id, name, kind, destination, folder, enabled, created_at, updated_at
         FROM post_meeting_hooks WHERE id = ?",
    )
    .bind(id)
    .fetch_one(state.db_manager.pool())
    .await
    .map_err(|error| format!("Failed to read saved hook: {error}"))
}

#[tauri::command]
pub async fn api_delete_post_meeting_hook(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<bool, String> {
    sqlx::query("DELETE FROM post_meeting_hooks WHERE id = ?")
        .bind(id)
        .execute(state.db_manager.pool())
        .await
        .map(|result| result.rows_affected() > 0)
        .map_err(|error| format!("Failed to delete hook: {error}"))
}

pub async fn run_post_meeting_hooks(pool: &SqlitePool, meeting_id: &str) {
    let hooks = sqlx::query_as::<_, PostMeetingHook>(
        "SELECT id, name, kind, destination, folder, enabled, created_at, updated_at
         FROM post_meeting_hooks WHERE enabled = 1 ORDER BY created_at",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    for hook in hooks {
        let result = match hook.kind.as_str() {
            "artifact_export" => {
                let output = PathBuf::from(&hook.destination).join(meeting_id);
                export_meeting_artifacts(pool, meeting_id, Some(&output))
                    .await
                    .map(|result| result.output_directory)
            }
            "obsidian_export" => export_meeting_to_obsidian(
                pool,
                meeting_id,
                Path::new(&hook.destination),
                hook.folder.as_deref(),
            )
            .await
            .map(|path| path.to_string_lossy().to_string()),
            _ => continue,
        };
        let (status, detail) = match result {
            Ok(detail) => ("success", detail),
            Err(error) => ("failed", error.to_string()),
        };
        let _ = sqlx::query(
            "INSERT INTO post_meeting_hook_runs (id, hook_id, meeting_id, status, detail, created_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(format!("hook-run-{}", Uuid::new_v4()))
        .bind(&hook.id)
        .bind(meeting_id)
        .bind(status)
        .bind(detail)
        .bind(Utc::now().to_rfc3339())
        .execute(pool)
        .await;
    }
}
