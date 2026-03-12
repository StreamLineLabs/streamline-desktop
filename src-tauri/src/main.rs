// Streamline Desktop — Tauri backend
// Manages the embedded Streamline server and exposes Tauri commands.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::Mutex;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    Manager, State,
};

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

struct ServerState {
    process: Mutex<Option<Child>>,
    config: Mutex<ServerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ServerConfig {
    kafka_port: u16,
    http_port: u16,
    data_dir: String,
    log_level: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            kafka_port: 9092,
            http_port: 9094,
            data_dir: default_data_dir(),
            log_level: "info".into(),
        }
    }
}

fn default_data_dir() -> String {
    dirs_next_data_dir()
        .unwrap_or_else(|| PathBuf::from("./data"))
        .to_string_lossy()
        .into_owned()
}

/// Best-effort data directory without pulling in the `dirs` crate.
fn dirs_next_data_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        std::env::var("HOME")
            .ok()
            .map(|h| PathBuf::from(h).join("Library/Application Support/io.streamline.desktop"))
    }
    #[cfg(target_os = "linux")]
    {
        std::env::var("XDG_DATA_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| std::env::var("HOME").ok().map(|h| PathBuf::from(h).join(".local/share")))
            .map(|p| p.join("streamline-desktop"))
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var("APPDATA")
            .ok()
            .map(|a| PathBuf::from(a).join("streamline-desktop"))
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        None
    }
}

// ---------------------------------------------------------------------------
// Server lifecycle helpers
// ---------------------------------------------------------------------------

fn streamline_binary_path() -> PathBuf {
    // 1. Check STREAMLINE_BINARY env var (explicit override)
    if let Ok(env_path) = std::env::var("STREAMLINE_BINARY") {
        let p = PathBuf::from(env_path);
        if p.exists() {
            return p;
        }
    }

    // 2. Check bundled location (Tauri resource bundle)
    let mut path = std::env::current_exe().unwrap_or_default();
    path.pop(); // remove binary name
    #[cfg(target_os = "macos")]
    {
        // Inside .app bundle: Contents/MacOS/../Resources/streamline
        path.pop();
        path.push("Resources");
    }
    path.push("streamline");
    if path.exists() {
        return path;
    }

    // 3. Fall back to PATH lookup
    if let Ok(output) = std::process::Command::new("which")
        .arg("streamline")
        .output()
    {
        if output.status.success() {
            let found = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !found.is_empty() {
                return PathBuf::from(found);
            }
        }
    }

    // Return the bundled path (will produce a clear error in spawn_server)
    path
}

fn spawn_server(config: &ServerConfig) -> Result<Child, String> {
    let bin = streamline_binary_path();
    if !bin.exists() {
        return Err(format!("Streamline binary not found at {}", bin.display()));
    }

    std::fs::create_dir_all(&config.data_dir).map_err(|e| e.to_string())?;

    Command::new(&bin)
        .args([
            "--kafka-port",
            &config.kafka_port.to_string(),
            "--http-port",
            &config.http_port.to_string(),
            "--data-dir",
            &config.data_dir,
            "--log-level",
            &config.log_level,
        ])
        .spawn()
        .map_err(|e| format!("Failed to start Streamline: {e}"))
}

fn kill_server(process: &mut Option<Child>) {
    if let Some(ref mut child) = process {
        let _ = child.kill();
        let _ = child.wait();
    }
    *process = None;
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct ServerStatus {
    running: bool,
    pid: Option<u32>,
    kafka_port: u16,
    http_port: u16,
}

// Consumer group types
#[derive(Serialize, Deserialize)]
struct ConsumerGroupInfo {
    group_id: String,
    state: String,
    members: u32,
    #[serde(default)]
    topics: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct ConsumerGroupDetail {
    group_id: String,
    state: String,
    protocol: String,
    members: Vec<GroupMember>,
    #[serde(default)]
    offsets: Vec<GroupOffset>,
}

#[derive(Serialize, Deserialize)]
struct GroupMember {
    member_id: String,
    client_id: String,
    #[serde(default)]
    host: String,
    #[serde(default)]
    assignments: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct GroupOffset {
    topic: String,
    partition: i32,
    current_offset: i64,
    log_end_offset: i64,
    lag: i64,
}

// Schema registry types
#[derive(Serialize, Deserialize)]
struct SchemaSubject {
    subject: String,
    #[serde(default)]
    version: u32,
    #[serde(default)]
    schema_type: String,
}

#[derive(Serialize, Deserialize)]
struct SchemaDetail {
    subject: String,
    version: u32,
    id: u32,
    schema_type: String,
    schema: String,
    #[serde(default)]
    compatibility: String,
}

#[tauri::command]
fn get_server_status(state: State<'_, ServerState>) -> ServerStatus {
    let proc = state.process.lock().unwrap();
    let config = state.config.lock().unwrap();
    ServerStatus {
        running: proc.is_some(),
        pid: proc.as_ref().map(|c| c.id()),
        kafka_port: config.kafka_port,
        http_port: config.http_port,
    }
}

#[tauri::command]
fn start_server(state: State<'_, ServerState>) -> Result<ServerStatus, String> {
    let mut proc = state.process.lock().unwrap();
    if proc.is_some() {
        return Err("Server is already running".into());
    }
    let config = state.config.lock().unwrap().clone();
    let child = spawn_server(&config)?;
    let status = ServerStatus {
        running: true,
        pid: Some(child.id()),
        kafka_port: config.kafka_port,
        http_port: config.http_port,
    };
    *proc = Some(child);
    Ok(status)
}

#[tauri::command]
fn stop_server(state: State<'_, ServerState>) -> Result<(), String> {
    let mut proc = state.process.lock().unwrap();
    if proc.is_none() {
        return Err("Server is not running".into());
    }
    kill_server(&mut proc);
    Ok(())
}

#[derive(Serialize)]
struct TopicInfo {
    name: String,
    partitions: u32,
}

#[tauri::command]
async fn get_topics(state: State<'_, ServerState>) -> Result<Vec<TopicInfo>, String> {
    let config = state.config.lock().unwrap().clone();
    let url = format!("http://127.0.0.1:{}/api/topics", config.http_port);
    let body = reqwest_get(&url).await?;
    serde_json::from_str::<Vec<TopicInfo>>(&body).map_err(|e| e.to_string())
}

#[derive(Serialize)]
struct ServerInfo {
    version: String,
    uptime_secs: u64,
    kafka_port: u16,
    http_port: u16,
}

#[tauri::command]
async fn get_server_info(state: State<'_, ServerState>) -> Result<ServerInfo, String> {
    let config = state.config.lock().unwrap().clone();
    let url = format!("http://127.0.0.1:{}/api/info", config.http_port);
    let body = reqwest_get(&url).await?;
    serde_json::from_str::<ServerInfo>(&body).map_err(|e| e.to_string())
}

/// HTTP GET using reqwest client.
async fn reqwest_get(url: &str) -> Result<String, String> {
    let response = reqwest::get(url)
        .await
        .map_err(|e| format!("HTTP GET failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("HTTP {status} from {url}: {body}"));
    }

    response.text().await.map_err(|e| format!("Failed to read response body: {e}"))
}

/// HTTP POST helper using reqwest client.
async fn reqwest_post(url: &str, body: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    let response = client
        .post(url)
        .header("Content-Type", "application/json")
        .body(body.to_owned())
        .send()
        .await
        .map_err(|e| format!("HTTP POST failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let err_body = response.text().await.unwrap_or_default();
        return Err(format!("HTTP POST {status}: {err_body}"));
    }

    response.text().await.map_err(|e| format!("Failed to read response body: {e}"))
}

/// HTTP DELETE helper using reqwest client.
async fn reqwest_delete(url: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    let response = client
        .delete(url)
        .send()
        .await
        .map_err(|e| format!("HTTP DELETE failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let err_body = response.text().await.unwrap_or_default();
        return Err(format!("HTTP DELETE {status}: {err_body}"));
    }

    response.text().await.map_err(|e| format!("Failed to read response body: {e}"))
}

#[derive(Serialize, Deserialize)]
struct ProduceRequest {
    key: Option<String>,
    value: String,
}

#[tauri::command]
async fn produce_message(
    state: State<'_, ServerState>,
    topic: String,
    key: Option<String>,
    value: String,
) -> Result<(), String> {
    let config = state.config.lock().unwrap().clone();
    let url = format!("http://127.0.0.1:{}/api/topics/{}/messages", config.http_port, topic);
    let body = serde_json::to_string(&ProduceRequest { key, value })
        .map_err(|e| e.to_string())?;
    reqwest_post(&url, &body).await?;
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct ConsumedMessage {
    key: String,
    value: String,
    offset: u64,
}

#[tauri::command]
async fn consume_messages(
    state: State<'_, ServerState>,
    topic: String,
    limit: Option<u32>,
) -> Result<Vec<ConsumedMessage>, String> {
    let config = state.config.lock().unwrap().clone();
    let limit = limit.unwrap_or(50);
    let url = format!(
        "http://127.0.0.1:{}/api/topics/{}/messages?limit={}",
        config.http_port, topic, limit
    );
    let body = reqwest_get(&url).await?;
    serde_json::from_str::<Vec<ConsumedMessage>>(&body).map_err(|e| e.to_string())
}

#[tauri::command]
async fn create_topic(
    state: State<'_, ServerState>,
    name: String,
    partitions: Option<u32>,
) -> Result<(), String> {
    let config = state.config.lock().unwrap().clone();
    let url = format!("http://127.0.0.1:{}/api/topics", config.http_port);
    let body = serde_json::json!({
        "name": name,
        "partitions": partitions.unwrap_or(1),
    })
    .to_string();
    reqwest_post(&url, &body).await?;
    Ok(())
}

#[tauri::command]
async fn delete_topic(
    state: State<'_, ServerState>,
    name: String,
) -> Result<(), String> {
    let config = state.config.lock().unwrap().clone();
    let url = format!("http://127.0.0.1:{}/api/topics/{}", config.http_port, name);
    reqwest_delete(&url).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Consumer group commands
// ---------------------------------------------------------------------------

#[tauri::command]
async fn list_consumer_groups(state: State<'_, ServerState>) -> Result<Vec<ConsumerGroupInfo>, String> {
    let config = state.config.lock().unwrap().clone();
    let url = format!("http://127.0.0.1:{}/api/consumer-groups", config.http_port);
    let body = reqwest_get(&url).await?;
    serde_json::from_str::<Vec<ConsumerGroupInfo>>(&body).map_err(|e| e.to_string())
}

#[tauri::command]
async fn describe_consumer_group(
    state: State<'_, ServerState>,
    group_id: String,
) -> Result<ConsumerGroupDetail, String> {
    let config = state.config.lock().unwrap().clone();
    let url = format!(
        "http://127.0.0.1:{}/api/consumer-groups/{}",
        config.http_port, group_id
    );
    let body = reqwest_get(&url).await?;
    serde_json::from_str::<ConsumerGroupDetail>(&body).map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_consumer_group(
    state: State<'_, ServerState>,
    group_id: String,
) -> Result<(), String> {
    let config = state.config.lock().unwrap().clone();
    let url = format!(
        "http://127.0.0.1:{}/api/consumer-groups/{}",
        config.http_port, group_id
    );
    reqwest_delete(&url).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Schema registry commands
// ---------------------------------------------------------------------------

#[tauri::command]
async fn list_schemas(state: State<'_, ServerState>) -> Result<Vec<SchemaSubject>, String> {
    let config = state.config.lock().unwrap().clone();
    let url = format!("http://127.0.0.1:{}/api/schemas/subjects", config.http_port);
    let body = reqwest_get(&url).await?;
    // The API may return just subject names as strings or full objects
    if let Ok(subjects) = serde_json::from_str::<Vec<String>>(&body) {
        Ok(subjects
            .into_iter()
            .map(|s| SchemaSubject {
                subject: s,
                version: 0,
                schema_type: String::new(),
            })
            .collect())
    } else {
        serde_json::from_str::<Vec<SchemaSubject>>(&body).map_err(|e| e.to_string())
    }
}

#[tauri::command]
async fn get_schema(
    state: State<'_, ServerState>,
    subject: String,
) -> Result<SchemaDetail, String> {
    let config = state.config.lock().unwrap().clone();
    let url = format!(
        "http://127.0.0.1:{}/api/schemas/subjects/{}/versions/latest",
        config.http_port, subject
    );
    let body = reqwest_get(&url).await?;
    serde_json::from_str::<SchemaDetail>(&body).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

fn settings_path() -> PathBuf {
    let mut p = dirs_next_data_dir().unwrap_or_else(|| PathBuf::from("."));
    std::fs::create_dir_all(&p).ok();
    p.push("settings.json");
    p
}

#[tauri::command]
fn save_settings(state: State<'_, ServerState>, settings: ServerConfig) -> Result<(), String> {
    let json = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    std::fs::write(settings_path(), json).map_err(|e| e.to_string())?;
    *state.config.lock().unwrap() = settings;
    Ok(())
}

#[tauri::command]
fn load_settings(state: State<'_, ServerState>) -> ServerConfig {
    if let Ok(data) = std::fs::read_to_string(settings_path()) {
        if let Ok(config) = serde_json::from_str::<ServerConfig>(&data) {
            *state.config.lock().unwrap() = config.clone();
            return config;
        }
    }
    state.config.lock().unwrap().clone()
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let initial_config = std::fs::read_to_string(settings_path())
        .ok()
        .and_then(|data| serde_json::from_str::<ServerConfig>(&data).ok())
        .unwrap_or_default();

    let state = ServerState {
        process: Mutex::new(None),
        config: Mutex::new(initial_config),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(state)
        .setup(|app| {
            // --- System tray ---------------------------------------------------
            let start_item = MenuItemBuilder::with_id("start", "Start Server").build(app)?;
            let stop_item = MenuItemBuilder::with_id("stop", "Stop Server").build(app)?;
            let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

            let tray_menu = MenuBuilder::new(app)
                .item(&start_item)
                .item(&stop_item)
                .separator()
                .item(&quit_item)
                .build()?;

            TrayIconBuilder::new()
                .menu(&tray_menu)
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "start" => {
                        let state = app.state::<ServerState>();
                        let _ = start_server(state);
                    }
                    "stop" => {
                        let state = app.state::<ServerState>();
                        let _ = stop_server(state);
                    }
                    "quit" => {
                        let state = app.state::<ServerState>();
                        let mut proc = state.process.lock().unwrap();
                        kill_server(&mut proc);
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app)?;

            // Auto-start the server on launch
            let state = app.state::<ServerState>();
            if let Err(e) = start_server(state.clone()) {
                eprintln!("Auto-start failed (expected during development): {e}");
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_server_status,
            start_server,
            stop_server,
            get_topics,
            get_server_info,
            produce_message,
            consume_messages,
            create_topic,
            delete_topic,
            list_consumer_groups,
            describe_consumer_group,
            delete_consumer_group,
            list_schemas,
            get_schema,
            save_settings,
            load_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Streamline Desktop");
}


/// Application-level error type for Tauri commands.
#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Operation timed out")]
    Timeout,
    #[error("Internal error: {0}")]
    Internal(String),
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}


/// TLS settings managed through the desktop app UI.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct TlsSettings {
    enabled: bool,
    ca_cert_path: Option<String>,
    client_cert_path: Option<String>,
    client_key_path: Option<String>,
    skip_verify: bool,
}

impl Default for TlsSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            ca_cert_path: None,
            client_cert_path: None,
            client_key_path: None,
            skip_verify: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_config_default_ports() {
        let config = ServerConfig::default();
        assert_eq!(config.kafka_port, 9092);
        assert_eq!(config.http_port, 9094);
    }

    #[test]
    fn test_server_config_default_log_level() {
        let config = ServerConfig::default();
        assert_eq!(config.log_level, "info");
    }

    #[test]
    fn test_server_config_serialization_roundtrip() {
        let config = ServerConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: ServerConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.kafka_port, config.kafka_port);
        assert_eq!(parsed.http_port, config.http_port);
        assert_eq!(parsed.log_level, config.log_level);
    }

    #[test]
    fn test_tls_settings_default() {
        let tls = TlsSettings::default();
        assert!(!tls.enabled);
        assert!(tls.ca_cert_path.is_none());
        assert!(tls.client_cert_path.is_none());
        assert!(tls.client_key_path.is_none());
        assert!(!tls.skip_verify);
    }

    #[test]
    fn test_tls_settings_serialization_roundtrip() {
        let tls = TlsSettings {
            enabled: true,
            ca_cert_path: Some("/path/to/ca.pem".into()),
            client_cert_path: Some("/path/to/cert.pem".into()),
            client_key_path: Some("/path/to/key.pem".into()),
            skip_verify: false,
        };
        let json = serde_json::to_string(&tls).unwrap();
        let parsed: TlsSettings = serde_json::from_str(&json).unwrap();
        assert!(parsed.enabled);
        assert_eq!(parsed.ca_cert_path.unwrap(), "/path/to/ca.pem");
    }

    #[test]
    fn test_server_status_serialization() {
        let status = ServerStatus {
            running: true,
            pid: Some(1234),
            kafka_port: 9092,
            http_port: 9094,
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"running\":true"));
        assert!(json.contains("\"pid\":1234"));
        assert!(json.contains("\"kafka_port\":9092"));
        assert!(json.contains("\"http_port\":9094"));
    }

    #[test]
    fn test_server_status_stopped() {
        let status = ServerStatus {
            running: false,
            pid: None,
            kafka_port: 9092,
            http_port: 9094,
        };
        assert!(!status.running);
        assert!(status.pid.is_none());
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"running\":false"));
    }

    #[test]
    fn test_topic_info_serialization() {
        let topic = TopicInfo {
            name: "events".into(),
            partitions: 3,
        };
        let json = serde_json::to_string(&topic).unwrap();
        assert!(json.contains("\"name\":\"events\""));
        assert!(json.contains("\"partitions\":3"));
    }

    #[test]
    fn test_consumed_message_serialization() {
        let msg = ConsumedMessage {
            key: "k1".into(),
            value: "hello world".into(),
            offset: 42,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: ConsumedMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.key, "k1");
        assert_eq!(parsed.value, "hello world");
        assert_eq!(parsed.offset, 42);
    }

    #[test]
    fn test_consumed_message_empty_key() {
        let msg = ConsumedMessage {
            key: String::new(),
            value: "data".into(),
            offset: 0,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"key\":\"\""));
    }

    #[test]
    fn test_consumer_group_info_serialization() {
        let group = ConsumerGroupInfo {
            group_id: "my-group".into(),
            state: "Stable".into(),
            members: 2,
            topics: vec!["events".into(), "logs".into()],
        };
        let json = serde_json::to_string(&group).unwrap();
        let parsed: ConsumerGroupInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.group_id, "my-group");
        assert_eq!(parsed.topics.len(), 2);
    }

    #[test]
    fn test_group_offset_lag_calculation_data() {
        let offset = GroupOffset {
            topic: "events".into(),
            partition: 0,
            current_offset: 100,
            log_end_offset: 150,
            lag: 50,
        };
        assert_eq!(offset.log_end_offset - offset.current_offset, offset.lag);
    }

    #[test]
    fn test_schema_detail_serialization() {
        let schema = SchemaDetail {
            subject: "events-value".into(),
            version: 1,
            id: 42,
            schema_type: "AVRO".into(),
            schema: r#"{"type":"record","name":"Event"}"#.into(),
            compatibility: "BACKWARD".into(),
        };
        let json = serde_json::to_string(&schema).unwrap();
        let parsed: SchemaDetail = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.subject, "events-value");
        assert_eq!(parsed.schema_type, "AVRO");
    }

    #[test]
    fn test_consumer_group_detail_roundtrip() {
        let detail = ConsumerGroupDetail {
            group_id: "grp-1".into(),
            state: "Stable".into(),
            protocol: "range".into(),
            members: vec![GroupMember {
                member_id: "m1".into(),
                client_id: "c1".into(),
                host: "127.0.0.1".into(),
                assignments: vec!["events-0".into()],
            }],
            offsets: vec![GroupOffset {
                topic: "events".into(),
                partition: 0,
                current_offset: 50,
                log_end_offset: 100,
                lag: 50,
            }],
        };
        let json = serde_json::to_string(&detail).unwrap();
        let parsed: ConsumerGroupDetail = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.members.len(), 1);
        assert_eq!(parsed.offsets.len(), 1);
        assert_eq!(parsed.offsets[0].lag, 50);
    }

    #[test]
    fn test_schema_subject_deserialization() {
        let json = r#"{"subject":"events-value","version":1,"schema_type":"AVRO"}"#;
        let parsed: SchemaSubject = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.subject, "events-value");
        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.schema_type, "AVRO");
    }

    #[test]
    fn test_schema_subject_defaults() {
        let json = r#"{"subject":"events-key"}"#;
        let parsed: SchemaSubject = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.subject, "events-key");
        assert_eq!(parsed.version, 0);
        assert_eq!(parsed.schema_type, "");
    }

    #[test]
    fn test_default_data_dir_is_not_empty() {
        let dir = default_data_dir();
        assert!(
            !dir.is_empty(),
            "default_data_dir should return a non-empty string"
        );
    }
}
