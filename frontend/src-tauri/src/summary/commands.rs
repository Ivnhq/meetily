use crate::database::repositories::{
    live_notes::LiveNotesRepository,
    meeting::MeetingsRepository, summary::SummaryProcessesRepository,
    transcript_chunk::TranscriptChunksRepository,
};
use crate::state::AppState;
use crate::summary::llm_client::{generate_summary, LLMProvider};
use crate::summary::processor::clean_llm_markdown_output;
use crate::summary::service::SummaryService;
use log::{error as log_error, info as log_info, warn as log_warn};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::{AppHandle, Manager, Runtime};

const LIVE_NOTES_GENERATION_TIMEOUT: Duration = Duration::from_secs(120);
const LIVE_NOTES_DEFAULT_MAX_TOKENS: u32 = 512;
const LIVE_NOTES_DEFAULT_TEMPERATURE: f32 = 0.1;
const LIVE_NOTES_DEFAULT_TOP_P: f32 = 0.9;

#[derive(Debug, Serialize, Deserialize)]
pub struct SummaryResponse {
    pub status: String,
    #[serde(rename = "meetingName")]
    pub meeting_name: Option<String>,
    pub meeting_id: String,
    pub start: Option<String>,
    pub end: Option<String>,
    pub data: Option<serde_json::Value>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProcessTranscriptResponse {
    pub message: String,
    pub process_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveNotesActionItem {
    pub text: String,
    pub owner: Option<String>,
    pub due_date: Option<String>,
    pub transcript_start_ms: Option<f64>,
    pub transcript_end_ms: Option<f64>,
    pub confidence: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveNotesDecision {
    pub text: String,
    pub transcript_start_ms: Option<f64>,
    pub transcript_end_ms: Option<f64>,
    pub confidence: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveNotesHighlight {
    pub text: Option<String>,
    pub transcript_start_ms: Option<f64>,
    pub transcript_end_ms: Option<f64>,
    pub note: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveNotesUpdate {
    pub current_topic: Option<String>,
    pub rolling_summary: Option<Vec<String>>,
    pub key_points: Option<Vec<String>>,
    pub decisions: Option<Vec<LiveNotesDecision>>,
    pub action_items: Option<Vec<LiveNotesActionItem>>,
    pub open_questions: Option<Vec<String>>,
    pub risks: Option<Vec<String>>,
    pub highlights: Option<Vec<LiveNotesHighlight>>,
}

#[tauri::command]
pub async fn api_get_live_notes<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
) -> Result<Option<serde_json::Value>, String> {
    if meeting_id.trim().is_empty() {
        return Err("meeting_id cannot be empty".to_string());
    }

    let pool = state.db_manager.pool();
    LiveNotesRepository::get_state(pool, &meeting_id)
        .await
        .map_err(|e| format!("Failed to get live notes: {}", e))
}

#[tauri::command]
pub async fn api_save_live_notes<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
    live_notes: serde_json::Value,
) -> Result<serde_json::Value, String> {
    if meeting_id.trim().is_empty() {
        return Err("meeting_id cannot be empty".to_string());
    }

    let pool = state.db_manager.pool();
    match LiveNotesRepository::save_state(pool, &meeting_id, &live_notes).await {
        Ok(saved) => Ok(serde_json::json!({ "saved": saved })),
        Err(e) => Err(format!("Failed to save live notes: {}", e)),
    }
}

/// Generates a small structured update for in-meeting live notes.
///
/// This is intentionally separate from the final summary pipeline: it is fast,
/// stateless on the Rust side, and designed for rolling frontend state merges.
#[tauri::command]
pub async fn api_generate_live_notes_update<R: Runtime>(
    app: AppHandle<R>,
    transcript_chunk: String,
    current_state: serde_json::Value,
    model: String,
    model_name: String,
    ollama_endpoint: Option<String>,
    custom_openai_endpoint: Option<String>,
    custom_openai_api_key: Option<String>,
    custom_openai_max_tokens: Option<u32>,
    custom_openai_temperature: Option<f32>,
    custom_openai_top_p: Option<f32>,
) -> Result<LiveNotesUpdate, String> {
    if transcript_chunk.trim().is_empty() {
        return Err("No transcript text available for live notes".to_string());
    }

    let provider = LLMProvider::from_str(&model)?;
    if !matches!(
        provider,
        LLMProvider::Ollama | LLMProvider::BuiltInAI | LLMProvider::CustomOpenAI
    ) {
        return Err(format!(
            "Live Notes currently supports local Ollama, built-in AI, or custom OpenAI-compatible providers. Current provider: {}",
            model
        ));
    }

    let api_key = if provider == LLMProvider::CustomOpenAI {
        custom_openai_api_key.unwrap_or_default()
    } else {
        String::new()
    };

    let system_prompt = r#"You update live meeting notes during an active call.

Return only valid JSON. Do not use markdown fences. Do not include commentary.
Keep items short, specific, and operational. Do not invent people, owners, or deadlines.
Use transcript timestamp ranges to set transcriptStartMs and transcriptEndMs in milliseconds when evidence is available.
Only include generated highlights when both transcriptStartMs and transcriptEndMs are known.
If the transcript chunk has no useful new information, return empty arrays.

JSON shape:
{
  "currentTopic": "short current topic or null",
  "rollingSummary": ["1-2 concise summary bullets"],
  "keyPoints": ["important facts from the new chunk"],
  "decisions": [{"text": "decision", "transcriptStartMs": 0, "transcriptEndMs": 0, "confidence": 0.0}],
  "actionItems": [{"text": "action", "owner": null, "dueDate": null, "transcriptStartMs": 0, "transcriptEndMs": 0, "confidence": 0.0}],
  "openQuestions": ["unresolved questions"],
  "risks": ["risks or concerns"],
  "highlights": [{"text": "notable moment", "transcriptStartMs": 0, "transcriptEndMs": 0}]
}"#;

    let user_prompt = format!(
        "Current live notes state:\n{}\n\nNew transcript chunk:\n{}\n\nUpdate the live notes using only the new transcript chunk and current state.",
        current_state,
        transcript_chunk
    );

    let client = reqwest::Client::new();
    let app_data_dir = app.path().app_data_dir().ok();
    let generation = generate_summary(
        &client,
        &provider,
        &model_name,
        &api_key,
        system_prompt,
        &user_prompt,
        ollama_endpoint.as_deref(),
        custom_openai_endpoint.as_deref(),
        custom_openai_max_tokens.or(Some(LIVE_NOTES_DEFAULT_MAX_TOKENS)),
        custom_openai_temperature.or(Some(LIVE_NOTES_DEFAULT_TEMPERATURE)),
        custom_openai_top_p.or(Some(LIVE_NOTES_DEFAULT_TOP_P)),
        app_data_dir.as_ref(),
        None,
    );

    let raw = tokio::time::timeout(LIVE_NOTES_GENERATION_TIMEOUT, generation)
        .await
        .map_err(|_| {
            format!(
                "Live Notes generation timed out after {} seconds. Try a smaller/faster local model or a GPU/accelerated provider.",
                LIVE_NOTES_GENERATION_TIMEOUT.as_secs()
            )
        })??;

    let cleaned = clean_llm_markdown_output(&raw);
    serde_json::from_str::<LiveNotesUpdate>(&cleaned).map_err(|e| {
        format!(
            "Live notes model returned invalid JSON: {}. Raw response: {}",
            e, cleaned
        )
    })
}

/// Saves a meeting summary (Native SQLx implementation)
///
/// Expected format: { "markdown": "...", "summary_json": [...BlockNote blocks...] }
#[tauri::command]
pub async fn api_save_meeting_summary<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
    summary: serde_json::Value,
    _auth_token: Option<String>,
) -> Result<serde_json::Value, String> {
    log_info!(
        "api_save_meeting_summary (native) called for meeting_id: {}",
        meeting_id
    );
    let pool = state.db_manager.pool();

    match SummaryProcessesRepository::update_meeting_summary(pool, &meeting_id, &summary).await {
        Ok(true) => {
            log_info!("Summary saved successfully for meeting_id: {}", meeting_id);
            Ok(serde_json::json!({
                "message": "Meeting summary saved successfully"
            }))
        }
        Ok(false) => {
            log_warn!(
                "Meeting not found or invalid JSON for meeting_id: {}",
                meeting_id
            );
            Err("Meeting not found or can't convert the json".into())
        }
        Err(e) => {
            log_error!("Failed to save meeting summary for {}: {}", meeting_id, e);
            Err(e.to_string())
        }
    }
}

/// Gets summary status and data (Native SQLx implementation)
///
/// Returns summary status (pending/processing/completed/failed) and parsed result data
#[tauri::command]
pub async fn api_get_summary<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
    _auth_token: Option<String>,
) -> Result<SummaryResponse, String> {
    log_info!(
        "api_get_summary (native) called for meeting_id: {}",
        meeting_id
    );
    let pool = state.db_manager.pool();

    match SummaryProcessesRepository::get_summary_data_for_meeting(pool, &meeting_id).await {
        Ok(Some(process)) => {
            let status = process.status.to_lowercase();
            let error = process.error;

            // Parse result data if it exists (regardless of status)
            // This allows displaying restored summaries after cancellation or failure
            let data = if let Some(result_str) = process.result {
                match serde_json::from_str::<serde_json::Value>(&result_str) {
                    Ok(parsed) => Some(parsed),
                    Err(e) => {
                        log_error!("Failed to parse summary result JSON: {}", e);
                        None
                    }
                }
            } else {
                None
            };

            // Fetch meeting title from database
            let meeting_name = match MeetingsRepository::get_meeting(pool, &meeting_id).await {
                Ok(Some(meeting_details)) => {
                    log_info!("Fetched meeting title: {}", &meeting_details.title);
                    Some(meeting_details.title)
                }
                Ok(None) => {
                    log_warn!("Meeting not found for meeting_id: {}", meeting_id);
                    None
                }
                Err(e) => {
                    log_error!("Failed to fetch meeting title: {}", e);
                    None
                }
            };

            let response = SummaryResponse {
                status: status.clone(),
                meeting_name,
                meeting_id: meeting_id.clone(),
                start: process.start_time.map(|t| t.to_rfc3339()),
                end: process.end_time.map(|t| t.to_rfc3339()),
                data,
                error,
            };

            log_info!(
                "Summary status for {}: {}, has_data: {}, meeting_name: {:?}",
                meeting_id,
                status,
                response.data.is_some(),
                response.meeting_name
            );
            Ok(response)
        }
        Ok(None) => {
            log_info!("No summary process found for meeting_id: {}", meeting_id);

            // Still fetch meeting title for idle state
            let meeting_name = match MeetingsRepository::get_meeting(pool, &meeting_id).await {
                Ok(Some(meeting_details)) => Some(meeting_details.title),
                _ => None,
            };

            Ok(SummaryResponse {
                status: "idle".to_string(),
                meeting_name,
                meeting_id,
                start: None,
                end: None,
                data: None,
                error: None,
            })
        }
        Err(e) => {
            log_error!("Error retrieving summary for {}: {}", meeting_id, e);
            Err(format!("Failed to retrieve summary: {}", e))
        }
    }
}

/// Processes transcript and generates summary (Native SQLx implementation)
///
/// Spawns a background task and returns immediately with process_id
#[tauri::command]
pub async fn api_process_transcript<R: Runtime>(
    app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    text: String,
    model: String,
    model_name: String,
    meeting_id: Option<String>,
    _chunk_size: Option<i32>,
    _overlap: Option<i32>,
    custom_prompt: Option<String>,
    template_id: Option<String>,
    _auth_token: Option<String>,
) -> Result<ProcessTranscriptResponse, String> {
    use uuid::Uuid;

    let m_id = meeting_id.unwrap_or_else(|| format!("meeting-{}", Uuid::new_v4()));
    log_info!(
        "api_process_transcript (native) called for meeting_id: {}, model: {}",
        &m_id,
        &model
    );

    let pool = state.db_manager.pool().clone();
    let final_prompt = custom_prompt.unwrap_or_else(|| "".to_string());
    let final_template_id = template_id.unwrap_or_else(|| "daily_standup".to_string());

    // Create or reset the process entry in the database
    SummaryProcessesRepository::create_or_reset_process(&pool, &m_id)
        .await
        .map_err(|e| format!("Failed to initialize process: {}", e))?;

    log_info!("✓ Summary process initialized for meeting_id: {}", &m_id);

    // Save transcript chunks data (matching Python backend behavior)
    let chunk_size = _chunk_size.unwrap_or(40000);
    let overlap = _overlap.unwrap_or(1000);

    TranscriptChunksRepository::save_transcript_data(
        &pool,
        &m_id,
        &text,
        &model,
        &model_name,
        chunk_size,
        overlap,
    )
    .await
    .map_err(|e| format!("Failed to save transcript data: {}", e))?;

    log_info!("✓ Transcript chunks saved for meeting_id: {}", &m_id);

    // Spawn background task for actual processing
    let meeting_id_clone = m_id.clone();
    tauri::async_runtime::spawn(async move {
        SummaryService::process_transcript_background(
            app,
            pool,
            meeting_id_clone.clone(),
            text,
            model,
            model_name,
            final_prompt,
            final_template_id,
        )
        .await;
    });

    log_info!("🚀 Background task spawned for meeting_id: {}", &m_id);

    Ok(ProcessTranscriptResponse {
        message: "Summary generation started".to_string(),
        process_id: m_id,
    })
}

/// Cancels an ongoing summary generation process
///
/// This command triggers the cancellation token for the specified meeting,
/// stopping the summary generation gracefully.
#[tauri::command]
pub async fn api_cancel_summary<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
) -> Result<serde_json::Value, String> {
    log_info!("api_cancel_summary called for meeting_id: {}", meeting_id);

    // Trigger cancellation via the service
    let cancelled = SummaryService::cancel_summary(&meeting_id);

    if cancelled {
        // Update database status to cancelled
        let pool = state.db_manager.pool();
        if let Err(e) = SummaryProcessesRepository::update_process_cancelled(pool, &meeting_id).await {
            log_error!("Failed to update DB status to cancelled for {}: {}", meeting_id, e);
            return Err(format!("Failed to update cancellation status: {}", e));
        }

        log_info!("Successfully cancelled summary generation for meeting_id: {}", meeting_id);
        Ok(serde_json::json!({
            "message": "Summary generation cancelled successfully",
            "meeting_id": meeting_id,
        }))
    } else {
        log_warn!("No active summary generation found for meeting_id: {}", meeting_id);
        Ok(serde_json::json!({
            "message": "No active summary generation to cancel",
            "meeting_id": meeting_id,
        }))
    }
}
