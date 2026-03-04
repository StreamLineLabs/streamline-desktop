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
    // Bundled alongside the app via tauri.conf.json `bundle.resources`.
    let mut path = std::env::current_exe().unwrap_or_default();
    path.pop(); // remove binary name
    #[cfg(target_os = "macos")]
    {
        // Inside .app bundle: Contents/MacOS/../Resources/streamline
        path.pop();
        path.push("Resources");
    }
    path.push("streamline");
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

/// Minimal HTTP GET without pulling in reqwest — uses the Tauri plugin-shell or
/// falls back to a blocking TCP request. For simplicity we shell out to curl.
async fn reqwest_get(url: &str) -> Result<String, String> {
    let output = tokio::process::Command::new("curl")
        .args(["-sf", url])
        .output()
        .await
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "HTTP {} from {}",
            output.status.code().unwrap_or(-1),
            url
        ));
    }
    String::from_utf8(output.stdout).map_err(|e| e.to_string())
}

/// HTTP POST helper.
async fn reqwest_post(url: &str, body: &str) -> Result<String, String> {
    let output = tokio::process::Command::new("curl")
        .args(["-sf", "-X", "POST", "-H", "Content-Type: application/json", "-d", body, url])
        .output()
        .await
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("HTTP POST failed ({}): {}", output.status.code().unwrap_or(-1), stderr));
    }
    String::from_utf8(output.stdout).map_err(|e| e.to_string())
}

/// HTTP DELETE helper.
async fn reqwest_delete(url: &str) -> Result<String, String> {
    let output = tokio::process::Command::new("curl")
        .args(["-sf", "-X", "DELETE", url])
        .output()
        .await
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "HTTP DELETE failed ({})",
            output.status.code().unwrap_or(-1),
        ));
    }
    String::from_utf8(output.stdout).map_err(|e| e.to_string())
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
            save_settings,
            load_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Streamline Desktop");
}
