use crate::audio;
use crate::state::AppState;
use chrono::{DateTime, Duration, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CalendarEvent {
    pub id: String,
    pub title: String,
    pub starts_at: String,
    pub ends_at: String,
    pub location: Option<String>,
    pub join_url: Option<String>,
    pub notes: Option<String>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarImportResult {
    pub imported: usize,
    pub source_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingDetection {
    pub detected: bool,
    pub event: Option<CalendarEvent>,
    pub meeting_app_running: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessCheck {
    pub id: String,
    pub label: String,
    pub ready: bool,
    pub detail: String,
}

#[tauri::command]
pub async fn api_import_calendar_ics(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<CalendarImportResult, String> {
    let content = std::fs::read_to_string(&path)
        .map_err(|error| format!("Failed to read calendar file: {error}"))?;
    let events = parse_ics(&content)?;
    let mut transaction = state
        .db_manager
        .pool()
        .begin()
        .await
        .map_err(|error| format!("Failed to start calendar import: {error}"))?;

    for event in &events {
        sqlx::query(
            "INSERT INTO calendar_events
                (id, title, starts_at, ends_at, location, join_url, notes, source, imported_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                starts_at = excluded.starts_at,
                ends_at = excluded.ends_at,
                location = excluded.location,
                join_url = excluded.join_url,
                notes = excluded.notes,
                source = excluded.source,
                imported_at = excluded.imported_at",
        )
        .bind(&event.id)
        .bind(&event.title)
        .bind(&event.starts_at)
        .bind(&event.ends_at)
        .bind(&event.location)
        .bind(&event.join_url)
        .bind(&event.notes)
        .bind(&event.source)
        .bind(Utc::now().to_rfc3339())
        .execute(&mut *transaction)
        .await
        .map_err(|error| format!("Failed to import calendar event: {error}"))?;
    }

    transaction
        .commit()
        .await
        .map_err(|error| format!("Failed to finish calendar import: {error}"))?;

    Ok(CalendarImportResult {
        imported: events.len(),
        source_path: path,
    })
}

#[tauri::command]
pub async fn api_get_upcoming_calendar_events(
    state: tauri::State<'_, AppState>,
    hours: Option<i64>,
) -> Result<Vec<CalendarEvent>, String> {
    let now = Utc::now();
    let until = now + Duration::hours(hours.unwrap_or(24).clamp(1, 24 * 31));
    sqlx::query_as::<_, CalendarEvent>(
        "SELECT id, title, starts_at, ends_at, location, join_url, notes, source
         FROM calendar_events
         WHERE ends_at >= ? AND starts_at <= ?
         ORDER BY starts_at LIMIT 100",
    )
    .bind(now.to_rfc3339())
    .bind(until.to_rfc3339())
    .fetch_all(state.db_manager.pool())
    .await
    .map_err(|error| format!("Failed to load calendar events: {error}"))
}

#[tauri::command]
pub async fn api_detect_current_meeting(
    state: tauri::State<'_, AppState>,
) -> Result<MeetingDetection, String> {
    let now = Utc::now();
    let event = sqlx::query_as::<_, CalendarEvent>(
        "SELECT id, title, starts_at, ends_at, location, join_url, notes, source
         FROM calendar_events
         WHERE starts_at <= ? AND ends_at >= ?
         ORDER BY starts_at LIMIT 1",
    )
    .bind((now + Duration::minutes(10)).to_rfc3339())
    .bind((now - Duration::minutes(10)).to_rfc3339())
    .fetch_optional(state.db_manager.pool())
    .await
    .map_err(|error| format!("Failed to detect calendar meeting: {error}"))?;
    let meeting_app_running = meeting_app_running();

    let reason = match (&event, meeting_app_running) {
        (Some(_), true) => "A scheduled meeting is due and a meeting app is running",
        (Some(_), false) => "A scheduled meeting is due",
        (None, true) => {
            "A meeting app is running, but no matching imported calendar event was found"
        }
        (None, false) => "No meeting signal detected",
    };

    Ok(MeetingDetection {
        detected: event.is_some(),
        event,
        meeting_app_running,
        reason: reason.to_string(),
    })
}

#[tauri::command]
pub async fn api_get_meeting_readiness(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ReadinessCheck>, String> {
    let database_ready = sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(state.db_manager.pool())
        .await
        .is_ok();
    let devices = audio::list_audio_devices().await.unwrap_or_default();
    let microphone_ready = devices
        .iter()
        .any(|device| matches!(device.device_type, crate::audio::DeviceType::Input));
    let system_audio_ready = devices
        .iter()
        .any(|device| matches!(device.device_type, crate::audio::DeviceType::Output));
    let model_ready = crate::parakeet_engine::commands::parakeet_has_available_models()
        .await
        .unwrap_or(false)
        || crate::whisper_engine::commands::whisper_has_available_models()
            .await
            .unwrap_or(false);
    let calendar_count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM calendar_events WHERE ends_at >= ?")
            .bind(Utc::now().to_rfc3339())
            .fetch_one(state.db_manager.pool())
            .await
            .unwrap_or_default();

    Ok(vec![
        ReadinessCheck {
            id: "database".to_string(),
            label: "Meeting storage".to_string(),
            ready: database_ready,
            detail: if database_ready {
                "Ready"
            } else {
                "Database unavailable"
            }
            .to_string(),
        },
        ReadinessCheck {
            id: "microphone".to_string(),
            label: "Microphone".to_string(),
            ready: microphone_ready,
            detail: format!("{} audio devices found", devices.len()),
        },
        ReadinessCheck {
            id: "system_audio".to_string(),
            label: "System audio".to_string(),
            ready: system_audio_ready,
            detail: if system_audio_ready {
                "Capture source available"
            } else {
                "No output source found"
            }
            .to_string(),
        },
        ReadinessCheck {
            id: "model".to_string(),
            label: "Transcription model".to_string(),
            ready: model_ready,
            detail: if model_ready {
                "Local model available"
            } else {
                "Download a transcription model"
            }
            .to_string(),
        },
        ReadinessCheck {
            id: "calendar".to_string(),
            label: "Calendar context".to_string(),
            ready: calendar_count > 0,
            detail: if calendar_count > 0 {
                format!("{calendar_count} upcoming events")
            } else {
                "Import an .ics calendar file (optional)".to_string()
            },
        },
    ])
}

fn meeting_app_running() -> bool {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        std::process::Command::new("pgrep")
            .args(["-ifl", "zoom.us|Microsoft Teams|Webex|Around|Google Meet"])
            .output()
            .is_ok_and(|output| output.status.success() && !output.stdout.is_empty())
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("tasklist")
            .output()
            .is_ok_and(|output| {
                let processes = String::from_utf8_lossy(&output.stdout).to_lowercase();
                ["zoom.exe", "ms-teams.exe", "webex.exe"]
                    .iter()
                    .any(|name| processes.contains(name))
            })
    }
}

fn parse_ics(content: &str) -> Result<Vec<CalendarEvent>, String> {
    let unfolded = content.replace("\r\n ", "").replace("\n ", "");
    let mut events = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    let mut in_event = false;

    for line in unfolded.lines() {
        let line = line.trim_end_matches('\r');
        if line == "BEGIN:VEVENT" {
            in_event = true;
            current.clear();
        } else if line == "END:VEVENT" && in_event {
            if let Some(event) = parse_event(&current)? {
                events.push(event);
            }
            in_event = false;
        } else if in_event {
            current.push(line);
        }
    }
    Ok(events)
}

fn parse_event(lines: &[&str]) -> Result<Option<CalendarEvent>, String> {
    let value = |name: &str| {
        lines.iter().find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.split(';')
                .next()
                .is_some_and(|key| key == name)
                .then(|| unescape_ics(value))
        })
    };
    let starts_at = match value("DTSTART") {
        Some(value) => parse_ics_datetime(&value)?,
        None => return Ok(None),
    };
    let ends_at = value("DTEND")
        .map(|value| parse_ics_datetime(&value))
        .transpose()?
        .unwrap_or(starts_at + Duration::hours(1));
    let title = value("SUMMARY").unwrap_or_else(|| "Calendar meeting".to_string());
    let notes = value("DESCRIPTION");
    let location = value("LOCATION");
    let join_url = value("URL").or_else(|| {
        notes.as_ref().and_then(|notes| {
            notes
                .split_whitespace()
                .find(|word| word.starts_with("https://"))
                .map(|word| word.trim_end_matches([',', '.', ')']).to_string())
        })
    });
    let id = value("UID").unwrap_or_else(|| {
        format!(
            "ics-{}",
            uuid::Uuid::new_v5(
                &uuid::Uuid::NAMESPACE_OID,
                format!("{title}:{starts_at}").as_bytes()
            )
        )
    });

    Ok(Some(CalendarEvent {
        id,
        title,
        starts_at: starts_at.to_rfc3339(),
        ends_at: ends_at.to_rfc3339(),
        location,
        join_url,
        notes,
        source: "ics".to_string(),
    }))
}

fn parse_ics_datetime(value: &str) -> Result<DateTime<Utc>, String> {
    if let Some(value) = value.strip_suffix('Z') {
        return NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S")
            .map(|date| Utc.from_utc_datetime(&date))
            .map_err(|error| format!("Invalid calendar timestamp {value}: {error}"));
    }
    if let Ok(date) = NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S") {
        return Local
            .from_local_datetime(&date)
            .single()
            .map(|date| date.with_timezone(&Utc))
            .ok_or_else(|| format!("Ambiguous local calendar timestamp: {value}"));
    }
    NaiveDate::parse_from_str(value, "%Y%m%d")
        .map_err(|error| format!("Invalid calendar date {value}: {error}"))?
        .and_hms_opt(0, 0, 0)
        .and_then(|date| Local.from_local_datetime(&date).single())
        .map(|date| date.with_timezone(&Utc))
        .ok_or_else(|| format!("Ambiguous local calendar date: {value}"))
}

fn unescape_ics(value: &str) -> String {
    value
        .replace("\\n", "\n")
        .replace("\\N", "\n")
        .replace("\\,", ",")
        .replace("\\;", ";")
        .replace("\\\\", "\\")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_calendar_event_and_join_url() {
        let events = parse_ics("BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:abc\nDTSTART:20260714T090000Z\nDTEND:20260714T093000Z\nSUMMARY:Launch review\nDESCRIPTION:Join https://meet.example/abc\nEND:VEVENT\nEND:VCALENDAR").unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].title, "Launch review");
        assert_eq!(
            events[0].join_url.as_deref(),
            Some("https://meet.example/abc")
        );
    }
}
