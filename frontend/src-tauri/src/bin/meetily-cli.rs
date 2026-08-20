use app_lib::artifacts::{
    export_meeting_artifacts, load_meeting_artifact_bundle, ARTIFACT_SCHEMA_VERSION,
};
use app_lib::database::models::MeetingModel;
use app_lib::database::repositories::transcript::TranscriptsRepository;
use chrono::Utc;
use clap::{Parser, Subcommand};
use serde::Serialize;
use serde_json::json;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Parser)]
#[command(
    name = "meetily-cli",
    version,
    about = "Read and export local Meetily meetings"
)]
struct Cli {
    /// Override the Meetily SQLite database path.
    #[arg(long, global = true, env = "MEETILY_DB_PATH")]
    db_path: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Show the local data contract and resolved database path.
    Info,
    /// List, inspect, and export meetings.
    Meetings {
        #[command(subcommand)]
        command: MeetingsCommand,
    },
    /// Search transcript text with the local FTS5 index.
    Search { query: String },
    /// Serve a local MCP endpoint over stdin/stdout. Read access is denied unless explicitly granted.
    Mcp {
        #[arg(long, env = "MEETILY_MCP_ALLOW_READ", default_value_t = false)]
        allow_read: bool,
    },
}

#[derive(Debug, Subcommand)]
enum MeetingsCommand {
    List {
        #[arg(long, default_value_t = 20)]
        limit: i64,
    },
    Get {
        meeting_id: String,
    },
    Export {
        meeting_id: String,
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Envelope<T: Serialize> {
    ok: bool,
    command: String,
    data: T,
    meta: EnvelopeMeta,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EnvelopeMeta {
    schema_version: u32,
    artifact_schema_version: String,
    generated_at: String,
    db_path: String,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let db_path = cli.db_path.unwrap_or_else(default_db_path);
    if let Err(error) = execute(cli.command, &db_path).await {
        let body = json!({
            "ok": false,
            "error": {
                "code": "meetily_cli_error",
                "message": error.to_string(),
            },
            "meta": build_meta(&db_path),
        });
        eprintln!("{}", serde_json::to_string_pretty(&body).unwrap());
        std::process::exit(1);
    }
}

async fn execute(command: Command, db_path: &Path) -> anyhow::Result<()> {
    if matches!(command, Command::Info) {
        return print_envelope(
            "info",
            json!({
                "databaseExists": db_path.exists(),
                "capabilities": ["meetings.list", "meetings.get", "meetings.export", "search.fts5", "qa.cited", "mcp.stdio"],
            }),
            db_path,
        );
    }

    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .read_only(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(options)
        .await?;

    match command {
        Command::Info => unreachable!(),
        Command::Meetings { command } => match command {
            MeetingsCommand::List { limit } => {
                let limit = limit.clamp(1, 500);
                let meetings = sqlx::query_as::<_, MeetingModel>(
                    "SELECT id, title, created_at, updated_at, folder_path FROM meetings ORDER BY created_at DESC LIMIT ?",
                )
                .bind(limit)
                .fetch_all(&pool)
                .await?;
                print_envelope("meetings list", meetings, db_path)
            }
            MeetingsCommand::Get { meeting_id } => {
                let bundle = load_meeting_artifact_bundle(&pool, &meeting_id).await?;
                print_envelope("meetings get", bundle, db_path)
            }
            MeetingsCommand::Export { meeting_id, output } => {
                let result =
                    export_meeting_artifacts(&pool, &meeting_id, output.as_deref()).await?;
                print_envelope("meetings export", result, db_path)
            }
        },
        Command::Search { query } => {
            let results = TranscriptsRepository::search_transcripts(&pool, &query).await?;
            print_envelope("search", results, db_path)
        }
        Command::Mcp { allow_read } => run_mcp_server(&pool, allow_read).await,
    }
}

async fn run_mcp_server(pool: &SqlitePool, allow_read: bool) -> anyhow::Result<()> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let request: serde_json::Value = serde_json::from_str(&line)?;
        if request.get("id").is_none() {
            continue;
        }
        let response = handle_mcp_request(pool, &request, allow_read).await;
        serde_json::to_writer(&mut stdout, &response)?;
        stdout.write_all(b"\n")?;
        stdout.flush()?;
    }
    Ok(())
}

async fn handle_mcp_request(
    pool: &SqlitePool,
    request: &serde_json::Value,
    allow_read: bool,
) -> serde_json::Value {
    let id = request
        .get("id")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let method = request
        .get("method")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();

    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": "2025-03-26",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "meetily-local", "version": env!("CARGO_PKG_VERSION") },
            "instructions": "Read-only access to the user's local Meetily database. Start with --allow-read to grant transcript access for this process."
        })),
        "tools/list" => Ok(json!({ "tools": mcp_tools() })),
        "tools/call" if !allow_read => Err((
            -32001,
            "Meetily transcript access is denied. Restart this MCP process with --allow-read or MEETILY_MCP_ALLOW_READ=true.".to_string(),
        )),
        "tools/call" => execute_mcp_tool(pool, request).await,
        _ => Err((-32601, format!("Unknown method: {method}"))),
    };

    match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, message)) => {
            json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
        }
    }
}

fn mcp_tools() -> Vec<serde_json::Value> {
    vec![
        json!({
            "name": "meetily_list_meetings",
            "description": "List recent local Meetily meetings.",
            "inputSchema": { "type": "object", "properties": { "limit": { "type": "integer", "minimum": 1, "maximum": 100 } } }
        }),
        json!({
            "name": "meetily_get_meeting",
            "description": "Read one meeting with transcripts, summary, and live notes.",
            "inputSchema": { "type": "object", "properties": { "meeting_id": { "type": "string" } }, "required": ["meeting_id"] }
        }),
        json!({
            "name": "meetily_search",
            "description": "Full-text search saved meeting transcripts.",
            "inputSchema": { "type": "object", "properties": { "query": { "type": "string" } }, "required": ["query"] }
        }),
        json!({
            "name": "meetily_ask",
            "description": "Return a grounded, cited answer from saved meeting transcripts.",
            "inputSchema": { "type": "object", "properties": { "question": { "type": "string" }, "limit": { "type": "integer", "minimum": 1, "maximum": 20 } }, "required": ["question"] }
        }),
    ]
}

async fn execute_mcp_tool(
    pool: &SqlitePool,
    request: &serde_json::Value,
) -> Result<serde_json::Value, (i64, String)> {
    let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
    let name = params
        .get("name")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| (-32602, "Tool name is required".to_string()))?;
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    let data = match name {
        "meetily_list_meetings" => {
            let limit = arguments
                .get("limit")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(20)
                .clamp(1, 100);
            serde_json::to_value(
                sqlx::query_as::<_, MeetingModel>("SELECT id, title, created_at, updated_at, folder_path FROM meetings ORDER BY created_at DESC LIMIT ?")
                    .bind(limit).fetch_all(pool).await.map_err(mcp_db_error)?
            ).map_err(mcp_json_error)?
        }
        "meetily_get_meeting" => {
            let meeting_id = required_string(&arguments, "meeting_id")?;
            serde_json::to_value(
                load_meeting_artifact_bundle(pool, meeting_id)
                    .await
                    .map_err(mcp_anyhow_error)?,
            )
            .map_err(mcp_json_error)?
        }
        "meetily_search" => {
            let query = required_string(&arguments, "query")?;
            serde_json::to_value(
                TranscriptsRepository::search_transcripts(pool, query)
                    .await
                    .map_err(mcp_db_error)?,
            )
            .map_err(mcp_json_error)?
        }
        "meetily_ask" => {
            let question = required_string(&arguments, "question")?;
            let limit = arguments
                .get("limit")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(6);
            serde_json::to_value(
                app_lib::knowledge::ask_meetings(pool, question, limit)
                    .await
                    .map_err(mcp_anyhow_error)?,
            )
            .map_err(mcp_json_error)?
        }
        _ => return Err((-32602, format!("Unknown tool: {name}"))),
    };
    Ok(json!({
        "content": [{ "type": "text", "text": serde_json::to_string_pretty(&data).unwrap_or_else(|_| data.to_string()) }],
        "structuredContent": data,
        "isError": false
    }))
}

fn required_string<'a>(value: &'a serde_json::Value, key: &str) -> Result<&'a str, (i64, String)> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| (-32602, format!("{key} is required")))
}

fn mcp_db_error(error: sqlx::Error) -> (i64, String) {
    (-32002, error.to_string())
}

fn mcp_anyhow_error(error: anyhow::Error) -> (i64, String) {
    (-32002, error.to_string())
}

fn mcp_json_error(error: serde_json::Error) -> (i64, String) {
    (-32603, error.to_string())
}

fn print_envelope<T: Serialize>(command: &str, data: T, db_path: &Path) -> anyhow::Result<()> {
    let envelope = Envelope {
        ok: true,
        command: command.to_string(),
        data,
        meta: build_meta(db_path),
    };
    println!("{}", serde_json::to_string_pretty(&envelope)?);
    Ok(())
}

fn build_meta(db_path: &Path) -> EnvelopeMeta {
    EnvelopeMeta {
        schema_version: 1,
        artifact_schema_version: ARTIFACT_SCHEMA_VERSION.to_string(),
        generated_at: Utc::now().to_rfc3339(),
        db_path: db_path.to_string_lossy().to_string(),
    }
}

fn default_db_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("com.meetily.ai")
        .join("meeting_minutes.sqlite")
}
