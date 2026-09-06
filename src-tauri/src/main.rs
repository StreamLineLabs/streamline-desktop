// Streamline Desktop — Tauri backend
// Manages the embedded Streamline server and exposes Tauri commands.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Mutex,
};
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    Manager, State,
};

mod topic_api;

use topic_api::*;

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

struct ServerState {
    process: Mutex<ServerProcess>,
    config: Mutex<ServerConfig>,
    consume_cursor: Mutex<HashMap<String, usize>>,
    startup_sequence: AtomicU64,
    /// Set when persisted settings could not be loaded, so the UI can report it
    /// instead of silently behaving like a first launch.
    settings_warning: Mutex<Option<String>>,
    /// Last background/tray startup failure. The frontend drains this value so
    /// an auto-start error in a packaged app is visible instead of stderr-only.
    startup_error: Mutex<Option<String>>,
}

enum ServerProcess {
    Stopped,
    Starting {
        id: u64,
        child: Option<Child>,
        config: ServerConfig,
    },
    Running {
        child: Child,
        config: ServerConfig,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ServerConfig {
    #[serde(default = "default_host")]
    host: String,
    kafka_port: u16,
    http_port: u16,
    data_dir: String,
    log_level: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            kafka_port: 9092,
            http_port: 9094,
            data_dir: default_data_dir(),
            log_level: "info".into(),
        }
    }
}

fn default_host() -> String {
    "127.0.0.1".into()
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
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|h| PathBuf::from(h).join(".local/share"))
            })
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

fn streamline_binary_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "streamline.exe"
    } else {
        "streamline"
    }
}

/// Packaged (release) builds must run the sidecar that ships inside the bundle.
/// `STREAMLINE_BINARY` and `PATH` lookups stay available for development only,
/// where the sidecar is usually not staged next to the executable.
fn external_binaries_allowed() -> bool {
    cfg!(debug_assertions)
}

/// Path of the Tauri sidecar installed beside the application executable.
fn bundled_binary_path() -> Option<PathBuf> {
    let mut path = std::env::current_exe().ok()?;
    path.pop(); // remove the application binary name
    path.push(streamline_binary_name());
    path.exists().then_some(path)
}

/// Resolve which Streamline executable to spawn.
///
/// Release builds fail closed when the bundled sidecar is missing rather than
/// silently running an arbitrary `streamline` from the user's environment.
fn resolve_streamline_binary(
    bundled: Option<PathBuf>,
    env_override: Option<String>,
    allow_external: bool,
) -> Result<PathBuf, String> {
    if allow_external {
        if let Some(raw) = env_override.filter(|value| !value.trim().is_empty()) {
            let path = PathBuf::from(raw);
            if !path.exists() {
                return Err(format!(
                    "STREAMLINE_BINARY points at {}, which does not exist. \
                     Unset it or point it at a Streamline executable.",
                    path.display()
                ));
            }
            return Ok(path);
        }
    }

    if let Some(path) = bundled {
        return Ok(path);
    }

    if allow_external {
        // Let Command resolve the executable from PATH on every platform.
        return Ok(PathBuf::from(streamline_binary_name()));
    }

    Err(format!(
        "The bundled Streamline server ({}) is missing from this installation. \
         Reinstall Streamline Desktop from an official release; packaged builds do not \
         fall back to STREAMLINE_BINARY or PATH.",
        streamline_binary_name()
    ))
}

fn streamline_binary_path() -> Result<PathBuf, String> {
    resolve_streamline_binary(
        bundled_binary_path(),
        std::env::var("STREAMLINE_BINARY").ok(),
        external_binaries_allowed(),
    )
}

fn server_arguments(config: &ServerConfig) -> Vec<String> {
    vec![
        "--listen-addr".into(),
        format!("{}:{}", config.host, config.kafka_port),
        "--http-addr".into(),
        format!("{}:{}", config.host, config.http_port),
        "--data-dir".into(),
        config.data_dir.clone(),
        "--log-level".into(),
        config.log_level.clone(),
    ]
}

fn readiness_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(500))
        .build()
        .map_err(|e| format!("Failed to create readiness client: {e}"))
}

async fn server_is_ready(client: &reqwest::Client, readiness_url: &str) -> bool {
    client
        .get(readiness_url)
        .send()
        .await
        .is_ok_and(|response| response.status().is_success())
}

async fn wait_for_server(
    state: &ServerState,
    startup_id: u64,
    client: &reqwest::Client,
    config: &ServerConfig,
) -> Result<u32, String> {
    let readiness_url = format!("{}/health/ready", http_base_url(config));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);

    loop {
        {
            let mut process = state.process.lock().unwrap();
            let child = match &mut *process {
                ServerProcess::Starting {
                    id,
                    child: Some(child),
                    ..
                } if *id == startup_id => child,
                _ => return Err("Server startup was cancelled".into()),
            };
            if let Some(status) = child
                .try_wait()
                .map_err(|e| format!("Failed to inspect Streamline process: {e}"))?
            {
                *process = ServerProcess::Stopped;
                return Err(format!(
                    "Streamline exited during startup with status {status}"
                ));
            }
        }

        if server_is_ready(client, &readiness_url).await {
            let mut process = state.process.lock().unwrap();
            let starting = std::mem::replace(&mut *process, ServerProcess::Stopped);
            match starting {
                ServerProcess::Starting {
                    id,
                    child: Some(mut child),
                    config,
                } if id == startup_id => {
                    if let Some(status) = child
                        .try_wait()
                        .map_err(|e| format!("Failed to inspect Streamline process: {e}"))?
                    {
                        return Err(format!(
                            "Streamline exited during startup with status {status}"
                        ));
                    }
                    let pid = child.id();
                    *process = ServerProcess::Running { child, config };
                    return Ok(pid);
                }
                other => {
                    *process = other;
                    return Err("Server startup was cancelled".into());
                }
            }
        }

        if std::time::Instant::now() >= deadline {
            cancel_startup(state, startup_id, ());
            return Err(format!(
                "Streamline did not become ready at {readiness_url} within 15 seconds"
            ));
        }

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

fn spawn_server_process(config: &ServerConfig) -> Result<Child, String> {
    // Normalize before spawning so `--listen-addr`/`--http-addr` always receive
    // a canonical, parseable authority (notably bracketed IPv6 literals).
    let config = normalized_config(config)?;
    let bin = streamline_binary_path()?;
    if bin.components().count() > 1 && !bin.exists() {
        return Err(format!("Streamline binary not found at {}", bin.display()));
    }

    std::fs::create_dir_all(&config.data_dir).map_err(|e| e.to_string())?;

    Command::new(&bin)
        .args(server_arguments(&config))
        .spawn()
        .map_err(|e| format!("Failed to start Streamline: {e}"))
}

fn kill_server(process: &mut ServerProcess) {
    match process {
        ServerProcess::Starting {
            child: Some(child), ..
        }
        | ServerProcess::Running { child, .. } => {
            let _ = child.kill();
            let _ = child.wait();
        }
        ServerProcess::Stopped | ServerProcess::Starting { child: None, .. } => {}
    }
    *process = ServerProcess::Stopped;
}

fn process_status(process: &mut ServerProcess) -> (bool, Option<u32>, Option<ServerConfig>) {
    match process {
        ServerProcess::Stopped => (false, None, None),
        ServerProcess::Starting { config, .. } => (false, None, Some(config.clone())),
        ServerProcess::Running { child, config } => match child.try_wait() {
            Ok(None) => (true, Some(child.id()), Some(config.clone())),
            Ok(Some(_)) | Err(_) => {
                *process = ServerProcess::Stopped;
                (false, None, None)
            }
        },
    }
}

fn running_config(state: &ServerState) -> Result<ServerConfig, String> {
    match &*state.process.lock().unwrap() {
        ServerProcess::Running { config, .. } => Ok(config.clone()),
        ServerProcess::Starting { .. } => Err("Server is still starting".into()),
        ServerProcess::Stopped => Err("Server is not running".into()),
    }
}

fn cancel_startup<T>(state: &ServerState, startup_id: u64, error: T) -> T {
    let mut process = state.process.lock().unwrap();
    if matches!(&*process, ServerProcess::Starting { id, .. } if *id == startup_id) {
        kill_server(&mut process);
    }
    error
}

fn install_starting_child(
    state: &ServerState,
    startup_id: u64,
    child: Child,
) -> Result<(), String> {
    let mut process = state.process.lock().unwrap();
    if let ServerProcess::Starting {
        id,
        child: child_slot,
        ..
    } = &mut *process
    {
        if *id == startup_id && child_slot.is_none() {
            *child_slot = Some(child);
            return Ok(());
        }
    }

    let mut child = child;
    let _ = child.kill();
    let _ = child.wait();
    Err("Server startup was cancelled".into())
}

fn record_startup_error(state: &ServerState, error: &str) {
    let message = format!("Streamline server failed to start: {error}");
    eprintln!("[streamline-desktop] {message}");
    *state.startup_error.lock().unwrap() = Some(message);
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
    members: usize,
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
    version: i32,
    id: i32,
    schema_type: String,
    schema: String,
    #[serde(default)]
    compatibility: String,
}

#[tauri::command]
fn get_server_status(state: State<'_, ServerState>) -> ServerStatus {
    let mut process = state.process.lock().unwrap();
    let (running, pid, active_config) = process_status(&mut process);
    drop(process);
    let config = active_config.unwrap_or_else(|| state.config.lock().unwrap().clone());
    ServerStatus {
        running,
        pid,
        kafka_port: config.kafka_port,
        http_port: config.http_port,
    }
}

#[tauri::command]
async fn start_server(state: State<'_, ServerState>) -> Result<ServerStatus, String> {
    let config = state.config.lock().unwrap().clone();
    let startup_id = state.startup_sequence.fetch_add(1, Ordering::Relaxed) + 1;
    {
        let mut process = state.process.lock().unwrap();
        match &*process {
            ServerProcess::Stopped => {
                *process = ServerProcess::Starting {
                    id: startup_id,
                    child: None,
                    config: config.clone(),
                };
            }
            ServerProcess::Starting { .. } => return Err("Server is already starting".into()),
            ServerProcess::Running { .. } => return Err("Server is already running".into()),
        }
    }

    let client = readiness_client().map_err(|error| cancel_startup(&state, startup_id, error))?;
    let readiness_url = format!("{}/health/ready", http_base_url(&config));
    if server_is_ready(&client, &readiness_url).await {
        return Err(cancel_startup(
            &state,
            startup_id,
            format!("A Streamline server is already responding at {readiness_url}"),
        ));
    }

    let child =
        spawn_server_process(&config).map_err(|error| cancel_startup(&state, startup_id, error))?;
    install_starting_child(&state, startup_id, child)?;
    let pid = wait_for_server(&state, startup_id, &client, &config).await?;

    let status = ServerStatus {
        running: true,
        pid: Some(pid),
        kafka_port: config.kafka_port,
        http_port: config.http_port,
    };
    Ok(status)
}

#[tauri::command]
fn stop_server(state: State<'_, ServerState>) -> Result<(), String> {
    let mut process = state.process.lock().unwrap();
    match &*process {
        ServerProcess::Stopped => return Err("Server is not running".into()),
        ServerProcess::Starting { .. } | ServerProcess::Running { .. } => {}
    }
    kill_server(&mut process);
    Ok(())
}

#[tauri::command]
async fn get_topics(state: State<'_, ServerState>) -> Result<Vec<TopicInfo>, String> {
    let config = running_config(&state)?;
    let paths = TopicApiPaths::new(http_base_url(&config));
    let body = reqwest_get(&paths.topics()).await?;
    Ok(user_topics(parse_topics(&body)?))
}

#[derive(Serialize, Deserialize)]
struct ServerInfo {
    version: String,
    uptime_secs: f64,
    kafka_port: u16,
    http_port: u16,
}

#[derive(Deserialize)]
struct ServerInfoResponse {
    version: String,
    uptime_seconds: f64,
}

fn parse_server_info(body: &str, config: &ServerConfig) -> Result<ServerInfo, String> {
    let info = serde_json::from_str::<ServerInfoResponse>(body).map_err(|e| e.to_string())?;
    Ok(ServerInfo {
        version: info.version,
        uptime_secs: info.uptime_seconds,
        kafka_port: config.kafka_port,
        http_port: config.http_port,
    })
}

#[tauri::command]
async fn get_server_info(state: State<'_, ServerState>) -> Result<ServerInfo, String> {
    let config = running_config(&state)?;
    let url = format!("{}/info", http_base_url(&config));
    let body = reqwest_get(&url).await?;
    parse_server_info(&body, &config)
}

/// Build the HTTP base URL from config.
fn http_base_url(config: &ServerConfig) -> String {
    format!("http://{}:{}", config.host, config.http_port)
}

fn encode_path_segment(segment: &str) -> String {
    utf8_percent_encode(segment, NON_ALPHANUMERIC).to_string()
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

    response
        .text()
        .await
        .map_err(|e| format!("Failed to read response body: {e}"))
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

    response
        .text()
        .await
        .map_err(|e| format!("Failed to read response body: {e}"))
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

    response
        .text()
        .await
        .map_err(|e| format!("Failed to read response body: {e}"))
}

#[tauri::command]
async fn produce_message(
    state: State<'_, ServerState>,
    topic: String,
    key: Option<String>,
    value: String,
) -> Result<(), String> {
    validate_user_topic(&topic)?;
    let config = running_config(&state)?;
    let url = TopicApiPaths::new(http_base_url(&config)).messages(&topic);
    let body =
        serde_json::to_string(&build_produce_request(key, value)).map_err(|e| e.to_string())?;
    reqwest_post(&url, &body).await?;
    Ok(())
}

#[tauri::command]
async fn consume_messages(
    state: State<'_, ServerState>,
    topic: String,
    limit: Option<u32>,
) -> Result<Vec<ConsumedMessage>, String> {
    validate_user_topic(&topic)?;
    let config = running_config(&state)?;
    let limit = limit.unwrap_or(50) as usize;
    let paths = TopicApiPaths::new(http_base_url(&config));
    let topics = parse_topics(&reqwest_get(&paths.topics()).await?)?;
    let partition_count = topics
        .iter()
        .find(|entry| entry.name == topic)
        .map(|entry| entry.partitions)
        .ok_or_else(|| format!("Topic '{topic}' was not found"))?;
    if partition_count == 0 {
        return Ok(Vec::new());
    }

    let partition_order = {
        let mut cursors = state.consume_cursor.lock().unwrap();
        let cursor = cursors.entry(topic.clone()).or_default();
        let start = *cursor % partition_count;
        *cursor = (start + limit.max(1).min(partition_count)) % partition_count;
        rotated_partition_order(partition_count, start)
    };

    let mut partition_messages = Vec::with_capacity(partition_count);
    for partition in partition_order {
        let url = paths.partition_messages(&topic, partition, limit);
        let body = reqwest_get(&url).await?;
        partition_messages.push(parse_consumed_messages(&body)?);
    }

    Ok(merge_partition_messages(partition_messages, limit))
}

#[tauri::command]
async fn create_topic(
    state: State<'_, ServerState>,
    name: String,
    partitions: Option<u32>,
) -> Result<(), String> {
    validate_user_topic(&name)?;
    let config = running_config(&state)?;
    let url = TopicApiPaths::new(http_base_url(&config)).topics();
    let body = serde_json::json!({
        "name": name,
        "partitions": partitions.unwrap_or(1),
    })
    .to_string();
    reqwest_post(&url, &body).await?;
    Ok(())
}

#[tauri::command]
async fn delete_topic(state: State<'_, ServerState>, name: String) -> Result<(), String> {
    validate_user_topic(&name)?;
    let config = running_config(&state)?;
    let url = TopicApiPaths::new(http_base_url(&config)).topic(&name);
    reqwest_delete(&url).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Consumer group commands
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ConsumerGroupInfoResponse {
    group_id: String,
    state: String,
    member_count: usize,
}

#[derive(Deserialize)]
struct ConsumerGroupDetailResponse {
    group_id: String,
    state: String,
    protocol: String,
    members: Vec<GroupMemberResponse>,
}

#[derive(Deserialize)]
struct GroupMemberResponse {
    member_id: String,
    client_id: String,
    client_host: String,
    assignments: Vec<MemberAssignmentResponse>,
}

#[derive(Deserialize)]
struct MemberAssignmentResponse {
    topic: String,
    partitions: Vec<i32>,
}

#[derive(Deserialize)]
struct ConsumerGroupLagResponse {
    partitions: Vec<GroupOffsetResponse>,
}

#[derive(Deserialize)]
struct GroupOffsetResponse {
    topic: String,
    partition: i32,
    current_offset: i64,
    log_end_offset: i64,
    lag: i64,
}

fn parse_consumer_groups(body: &str) -> Result<Vec<ConsumerGroupInfo>, String> {
    let groups =
        serde_json::from_str::<Vec<ConsumerGroupInfoResponse>>(body).map_err(|e| e.to_string())?;
    Ok(groups
        .into_iter()
        .map(|group| ConsumerGroupInfo {
            group_id: group.group_id,
            state: group.state,
            members: group.member_count,
            topics: Vec::new(),
        })
        .collect())
}

fn parse_consumer_group_detail(
    detail_body: &str,
    lag_body: &str,
) -> Result<ConsumerGroupDetail, String> {
    let detail = serde_json::from_str::<ConsumerGroupDetailResponse>(detail_body)
        .map_err(|e| e.to_string())?;
    let lag =
        serde_json::from_str::<ConsumerGroupLagResponse>(lag_body).map_err(|e| e.to_string())?;

    let members = detail
        .members
        .into_iter()
        .map(|member| {
            let assignments = member
                .assignments
                .into_iter()
                .flat_map(|assignment| {
                    assignment
                        .partitions
                        .into_iter()
                        .map(move |partition| format!("{}-{}", assignment.topic, partition))
                })
                .collect();
            GroupMember {
                member_id: member.member_id,
                client_id: member.client_id,
                host: member.client_host,
                assignments,
            }
        })
        .collect();

    let offsets = lag
        .partitions
        .into_iter()
        .map(|offset| GroupOffset {
            topic: offset.topic,
            partition: offset.partition,
            current_offset: offset.current_offset,
            log_end_offset: offset.log_end_offset,
            lag: offset.lag,
        })
        .collect();

    Ok(ConsumerGroupDetail {
        group_id: detail.group_id,
        state: detail.state,
        protocol: detail.protocol,
        members,
        offsets,
    })
}

#[tauri::command]
async fn list_consumer_groups(
    state: State<'_, ServerState>,
) -> Result<Vec<ConsumerGroupInfo>, String> {
    let config = running_config(&state)?;
    let url = format!("{}/api/v1/consumer-groups", http_base_url(&config));
    let body = reqwest_get(&url).await?;
    parse_consumer_groups(&body)
}

#[tauri::command]
async fn describe_consumer_group(
    state: State<'_, ServerState>,
    group_id: String,
) -> Result<ConsumerGroupDetail, String> {
    let config = running_config(&state)?;
    let group_id = encode_path_segment(&group_id);
    let detail_url = format!(
        "{}/api/v1/consumer-groups/{}",
        http_base_url(&config),
        group_id,
    );
    let lag_url = format!("{detail_url}/lag");
    let detail_body = reqwest_get(&detail_url).await?;
    let lag_body = reqwest_get(&lag_url).await?;
    parse_consumer_group_detail(&detail_body, &lag_body)
}

#[tauri::command]
async fn delete_consumer_group(
    state: State<'_, ServerState>,
    group_id: String,
) -> Result<(), String> {
    let config = running_config(&state)?;
    let url = format!(
        "{}/api/v1/consumer-groups/{}",
        http_base_url(&config),
        encode_path_segment(&group_id)
    );
    reqwest_delete(&url).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Schema registry commands
// ---------------------------------------------------------------------------

fn parse_schema_subjects(body: &str) -> Result<Vec<SchemaSubject>, String> {
    let subjects = serde_json::from_str::<Vec<String>>(body).map_err(|e| e.to_string())?;
    Ok(subjects
        .into_iter()
        .map(|subject| SchemaSubject {
            subject,
            version: 0,
            schema_type: String::new(),
        })
        .collect())
}

#[derive(Deserialize)]
struct SchemaDetailResponse {
    subject: String,
    version: i32,
    id: i32,
    #[serde(rename = "schemaType", default = "default_schema_type")]
    schema_type: String,
    schema: String,
}

fn default_schema_type() -> String {
    "AVRO".into()
}

fn parse_schema_detail(body: &str) -> Result<SchemaDetail, String> {
    let detail = serde_json::from_str::<SchemaDetailResponse>(body).map_err(|e| e.to_string())?;
    Ok(SchemaDetail {
        subject: detail.subject,
        version: detail.version,
        id: detail.id,
        schema_type: detail.schema_type,
        schema: detail.schema,
        compatibility: String::new(),
    })
}

#[tauri::command]
async fn list_schemas(state: State<'_, ServerState>) -> Result<Vec<SchemaSubject>, String> {
    let config = running_config(&state)?;
    let url = format!("{}/subjects", http_base_url(&config));
    let body = reqwest_get(&url).await?;
    parse_schema_subjects(&body)
}

#[tauri::command]
async fn get_schema(
    state: State<'_, ServerState>,
    subject: String,
) -> Result<SchemaDetail, String> {
    let config = running_config(&state)?;
    let url = format!(
        "{}/subjects/{}/versions/latest",
        http_base_url(&config),
        encode_path_segment(&subject)
    );
    let body = reqwest_get(&url).await?;
    parse_schema_detail(&body)
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

/// Streamline Desktop runs a local, unauthenticated server, so it must only ever
/// bind to the loopback interface.
///
/// Returns the canonical form that is safe to persist and reuse verbatim:
/// surrounding whitespace is removed and IPv6 literals are always bracketed, so
/// the same string is valid both in a URL authority (`http://[::1]:9094`) and in
/// the server's `--listen-addr` argument (`[::1]:9092`).
fn normalize_host(host: &str) -> Result<String, String> {
    let trimmed = host.trim();
    if trimmed.eq_ignore_ascii_case("localhost") {
        return Ok(trimmed.to_string());
    }

    let bracketed = trimmed.strip_prefix('[').and_then(|h| h.strip_suffix(']'));
    let normalized = match bracketed.unwrap_or(trimmed).parse::<IpAddr>() {
        // Brackets are the IPv6 literal syntax; `[127.0.0.1]` is not an address.
        Ok(IpAddr::V4(ip)) if ip.is_loopback() && bracketed.is_none() => Some(ip.to_string()),
        Ok(IpAddr::V6(ip)) if ip.is_loopback() => Some(format!("[{ip}]")),
        _ => None,
    };

    normalized.ok_or_else(|| {
        format!(
            "Host \"{trimmed}\" is not a loopback address. Streamline Desktop only serves the \
             local machine — use 127.0.0.1, localhost, or ::1."
        )
    })
}

const VALID_LOG_LEVELS: [&str; 5] = ["trace", "debug", "info", "warn", "error"];

/// Reject settings that would produce an unstartable or unsafe server, and
/// return the canonical configuration to persist and use.
fn normalized_config(config: &ServerConfig) -> Result<ServerConfig, String> {
    let host = normalize_host(&config.host)?;

    for (label, port) in [
        ("Kafka port", config.kafka_port),
        ("HTTP port", config.http_port),
    ] {
        if port == 0 {
            return Err(format!("{label} must be between 1 and 65535."));
        }
    }

    if config.kafka_port == config.http_port {
        return Err(format!(
            "Kafka port and HTTP port must differ (both are {}).",
            config.kafka_port
        ));
    }

    let data_dir = config.data_dir.trim();
    if data_dir.is_empty() {
        return Err("Data directory must not be empty.".into());
    }
    if !Path::new(data_dir).is_absolute() {
        return Err(format!(
            "Data directory must be an absolute path (got \"{data_dir}\")."
        ));
    }

    if !VALID_LOG_LEVELS.contains(&config.log_level.as_str()) {
        return Err(format!(
            "Log level \"{}\" is not supported. Use one of: {}.",
            config.log_level,
            VALID_LOG_LEVELS.join(", ")
        ));
    }

    Ok(ServerConfig {
        host,
        ..config.clone()
    })
}

/// Result of reading persisted settings: never silently indistinguishable from a
/// first launch.
struct SettingsLoad {
    config: ServerConfig,
    warning: Option<String>,
}

/// Move an unusable settings file aside so the user's data is preserved instead
/// of being overwritten by the next save.
fn quarantine_settings_file(path: &Path) -> Result<PathBuf, String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    let mut quarantined = path.to_path_buf();
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "settings.json".into());
    quarantined.set_file_name(format!("{name}.invalid-{stamp}"));
    std::fs::rename(path, &quarantined).map_err(|e| e.to_string())?;
    Ok(quarantined)
}

fn recover_invalid_settings(path: &Path, reason: String) -> SettingsLoad {
    let warning = match quarantine_settings_file(path) {
        Ok(quarantined) => format!(
            "Saved settings in {} could not be used ({reason}). The file was preserved as {} and \
             default settings are in use.",
            path.display(),
            quarantined.display()
        ),
        Err(rename_error) => format!(
            "Saved settings in {} could not be used ({reason}) and could not be moved aside \
             ({rename_error}). Default settings are in use; saving settings will overwrite the file.",
            path.display()
        ),
    };
    SettingsLoad {
        config: ServerConfig::default(),
        warning: Some(warning),
    }
}

fn load_settings_from_path(path: &Path) -> SettingsLoad {
    let data = match std::fs::read_to_string(path) {
        Ok(data) => data,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return SettingsLoad {
                config: ServerConfig::default(),
                warning: None,
            };
        }
        Err(error) => {
            return SettingsLoad {
                config: ServerConfig::default(),
                warning: Some(format!(
                    "Saved settings in {} could not be read ({error}). Default settings are in use.",
                    path.display()
                )),
            };
        }
    };

    match serde_json::from_str::<ServerConfig>(&data) {
        Ok(config) => match normalized_config(&config) {
            Ok(config) => SettingsLoad {
                config,
                warning: None,
            },
            Err(reason) => recover_invalid_settings(path, reason),
        },
        Err(error) => recover_invalid_settings(path, format!("invalid JSON: {error}")),
    }
}

#[tauri::command]
fn save_settings(state: State<'_, ServerState>, settings: ServerConfig) -> Result<(), String> {
    // Persist the canonical form so the stored file, the running server and the
    // HTTP base URL can never disagree about the host.
    let settings = normalized_config(&settings)?;
    let path = settings_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create {}: {e}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("Failed to write {}: {e}", path.display()))?;
    *state.config.lock().unwrap() = settings;
    *state.settings_warning.lock().unwrap() = None;
    Ok(())
}

#[tauri::command]
fn load_settings(state: State<'_, ServerState>) -> ServerConfig {
    let loaded = load_settings_from_path(&settings_path());
    if loaded.warning.is_some() {
        *state.settings_warning.lock().unwrap() = loaded.warning;
        return state.config.lock().unwrap().clone();
    }
    *state.config.lock().unwrap() = loaded.config.clone();
    loaded.config
}

/// Non-fatal settings problem detected at startup or on load, for the UI to show.
#[tauri::command]
fn get_settings_warning(state: State<'_, ServerState>) -> Option<String> {
    state.settings_warning.lock().unwrap().clone()
}

/// Drain a startup error recorded by background auto-start or the tray menu.
///
/// Taking rather than cloning prevents the five-second frontend poll from
/// showing the same error repeatedly.
#[tauri::command]
fn take_startup_error(state: State<'_, ServerState>) -> Option<String> {
    state.startup_error.lock().unwrap().take()
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let initial = load_settings_from_path(&settings_path());
    if let Some(warning) = &initial.warning {
        eprintln!("[streamline-desktop] {warning}");
    }

    let state = ServerState {
        process: Mutex::new(ServerProcess::Stopped),
        config: Mutex::new(initial.config),
        consume_cursor: Mutex::new(HashMap::new()),
        startup_sequence: AtomicU64::new(0),
        settings_warning: Mutex::new(initial.warning),
        startup_error: Mutex::new(None),
    };

    let app = tauri::Builder::default()
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
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let state = app.state::<ServerState>();
                            if let Err(error) = start_server(state.clone()).await {
                                record_startup_error(&state, &error);
                            }
                        });
                    }
                    "stop" => {
                        let state = app.state::<ServerState>();
                        let _ = stop_server(state);
                    }
                    "quit" => {
                        let state = app.state::<ServerState>();
                        let mut process = state.process.lock().unwrap();
                        kill_server(&mut process);
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app)?;

            // Auto-start the server on launch
            let app = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let state = app.state::<ServerState>();
                if let Err(error) = start_server(state.clone()).await {
                    record_startup_error(&state, &error);
                }
            });

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
            get_settings_warning,
            take_startup_error,
        ])
        .build(tauri::generate_context!())
        .expect("error while running Streamline Desktop");

    app.run(|app, event| {
        if matches!(
            event,
            tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit
        ) {
            let state = app.state::<ServerState>();
            kill_server(&mut state.process.lock().unwrap());
        }
    });
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
    fn test_server_config_defaults_missing_host() {
        let config: ServerConfig = serde_json::from_str(
            r#"{"kafka_port":9092,"http_port":9094,"data_dir":"./data","log_level":"info"}"#,
        )
        .unwrap();
        assert_eq!(config.host, "127.0.0.1");
    }

    #[test]
    fn test_server_arguments_use_supported_address_flags() {
        let arguments = server_arguments(&ServerConfig::default());
        assert_eq!(
            arguments,
            vec![
                "--listen-addr".to_string(),
                "127.0.0.1:9092".to_string(),
                "--http-addr".to_string(),
                "127.0.0.1:9094".to_string(),
                "--data-dir".to_string(),
                default_data_dir(),
                "--log-level".to_string(),
                "info".to_string(),
            ]
        );
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
            messages: 0,
            internal: false,
        };
        let json = serde_json::to_string(&topic).unwrap();
        assert!(json.contains("\"name\":\"events\""));
        assert!(json.contains("\"partitions\":3"));
    }

    #[test]
    fn test_parse_topics_uses_streamline_api_fields() {
        let topics = parse_topics(
            r#"[{"name":"events","partition_count":3,"replication_factor":1,"is_internal":false,"total_messages":42,"total_bytes":1024}]"#,
        )
        .unwrap();
        assert_eq!(topics.len(), 1);
        assert_eq!(topics[0].partitions, 3);
        assert_eq!(topics[0].messages, 42);
        assert!(!topics[0].internal);
    }

    #[test]
    fn test_internal_topics_are_rejected() {
        assert!(validate_user_topic("_schemas").is_err());
        assert!(validate_user_topic("__consumer_offsets").is_err());
        assert!(validate_user_topic("events").is_ok());
    }

    #[test]
    fn test_reserved_topics_are_hidden_even_without_internal_flag() {
        let topics = parse_topics(
            r#"[{"name":"_schemas","partition_count":1,"replication_factor":1,"is_internal":false,"total_messages":1,"total_bytes":1},{"name":"events","partition_count":1,"replication_factor":1,"is_internal":false,"total_messages":1,"total_bytes":1}]"#,
        )
        .unwrap();
        let visible = user_topics(topics);
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].name, "events");
    }

    #[test]
    fn test_parse_server_info_maps_wire_contract() {
        let config = ServerConfig::default();
        let info = parse_server_info(
            r#"{"version":"0.3.0","listen_addr":"127.0.0.1:9092","http_addr":"127.0.0.1:9094","data_dir":"./data","topics":2,"uptime_seconds":12.5}"#,
            &config,
        )
        .unwrap();
        assert_eq!(info.version, "0.3.0");
        assert_eq!(info.uptime_secs, 12.5);
        assert_eq!(info.kafka_port, 9092);
        assert_eq!(info.http_port, 9094);
    }

    #[test]
    fn test_build_produce_request_matches_streamline_api() {
        let request = build_produce_request(Some("key".into()), r#"{"event":"created"}"#.into());
        let value = serde_json::to_value(request).unwrap();
        assert_eq!(value["records"][0]["key"], "key");
        assert_eq!(value["records"][0]["value"]["event"], "created");
        assert!(value["records"][0]["partition"].is_null());
        assert_eq!(value["records"][0]["headers"], serde_json::json!({}));
    }

    #[test]
    fn test_consumed_message_serialization() {
        let msg = ConsumedMessage {
            key: "k1".into(),
            value: "hello world".into(),
            partition: 1,
            offset: 42,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: ConsumedMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.key, "k1");
        assert_eq!(parsed.value, "hello world");
        assert_eq!(parsed.offset, 42);
    }

    #[test]
    fn test_parse_consumed_messages_maps_streamline_api() {
        let messages = parse_consumed_messages(
            r#"{"topic":"events","partition":0,"records":[{"offset":7,"timestamp":1,"key":null,"value":{"event":"created"},"headers":{}}],"next_offset":8}"#,
        )
        .unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].key, "");
        assert_eq!(messages[0].value, r#"{"event":"created"}"#);
        assert_eq!(messages[0].partition, 0);
        assert_eq!(messages[0].offset, 7);
    }

    #[test]
    fn test_merge_partition_messages_is_fair() {
        let message = |partition, offset| ConsumedMessage {
            key: String::new(),
            value: String::new(),
            partition,
            offset,
        };
        let messages = merge_partition_messages(
            vec![
                vec![message(0, 0), message(0, 1), message(0, 2)],
                vec![message(1, 0), message(1, 1)],
            ],
            4,
        );
        let positions: Vec<(i32, i64)> = messages
            .into_iter()
            .map(|message| (message.partition, message.offset))
            .collect();
        assert_eq!(positions, vec![(0, 0), (1, 0), (0, 1), (1, 1)]);
    }

    #[test]
    fn test_partition_order_rotates() {
        assert_eq!(rotated_partition_order(4, 0), vec![0, 1, 2, 3]);
        assert_eq!(rotated_partition_order(4, 3), vec![3, 0, 1, 2]);
    }

    #[test]
    fn test_consumed_message_empty_key() {
        let msg = ConsumedMessage {
            key: String::new(),
            value: "data".into(),
            partition: 0,
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
    fn test_parse_consumer_group_contracts() {
        let groups = parse_consumer_groups(
            r#"[{"group_id":"analytics","state":"Stable","member_count":1,"coordinator":1,"protocol_type":"consumer"}]"#,
        )
        .unwrap();
        assert_eq!(groups[0].members, 1);

        let detail = parse_consumer_group_detail(
            r#"{"group_id":"analytics","state":"Stable","protocol_type":"consumer","protocol":"range","coordinator":1,"members":[{"member_id":"m1","client_id":"c1","client_host":"127.0.0.1","assignments":[{"topic":"events","partitions":[0]}]}],"offsets":[]}"#,
            r#"{"group_id":"analytics","state":"Stable","partitions":[{"topic":"events","partition":0,"current_offset":10,"log_end_offset":15,"lag":5}],"total_lag":5}"#,
        )
        .unwrap();
        assert_eq!(detail.members[0].assignments, vec!["events-0"]);
        assert_eq!(detail.offsets[0].lag, 5);
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
    fn test_parse_schema_contracts() {
        let subjects = parse_schema_subjects(r#"["events-value"]"#).unwrap();
        assert_eq!(subjects[0].subject, "events-value");

        let avro = parse_schema_detail(
            r#"{"subject":"events-value","version":1,"id":42,"schema":"{\"type\":\"record\"}"}"#,
        )
        .unwrap();
        assert_eq!(avro.schema_type, "AVRO");

        let json = parse_schema_detail(
            r#"{"subject":"events-value","version":2,"id":43,"schemaType":"JSON","schema":"{}"}"#,
        )
        .unwrap();
        assert_eq!(json.schema_type, "JSON");
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

    // -- Sidecar resolution policy -----------------------------------------

    #[test]
    fn test_release_builds_require_the_bundled_sidecar() {
        let error = resolve_streamline_binary(None, Some("/opt/streamline".into()), false)
            .expect_err("packaged builds must not fall back to an external binary");
        assert!(error.contains("bundled Streamline server"), "{error}");
        assert!(error.contains("Reinstall"), "{error}");
    }

    #[test]
    fn test_release_builds_use_the_bundled_sidecar() {
        let bundled = PathBuf::from("/Applications/Streamline.app/Contents/MacOS/streamline");
        let resolved =
            resolve_streamline_binary(Some(bundled.clone()), Some("/opt/streamline".into()), false)
                .unwrap();
        assert_eq!(resolved, bundled);
    }

    #[test]
    fn test_development_prefers_existing_env_override() {
        let override_path = std::env::current_exe().unwrap();
        let resolved = resolve_streamline_binary(
            Some(PathBuf::from("/bundled/streamline")),
            Some(override_path.to_string_lossy().into_owned()),
            true,
        )
        .unwrap();
        assert_eq!(resolved, override_path);
    }

    #[test]
    fn test_development_rejects_missing_env_override() {
        let error =
            resolve_streamline_binary(None, Some("/definitely/not/here/streamline".into()), true)
                .expect_err("a broken STREAMLINE_BINARY should be reported, not ignored");
        assert!(error.contains("STREAMLINE_BINARY"), "{error}");
    }

    #[test]
    fn test_development_falls_back_to_path_lookup() {
        let resolved = resolve_streamline_binary(None, None, true).unwrap();
        assert_eq!(resolved, PathBuf::from(streamline_binary_name()));
    }

    #[test]
    fn test_blank_env_override_is_ignored() {
        let resolved = resolve_streamline_binary(None, Some("   ".into()), true).unwrap();
        assert_eq!(resolved, PathBuf::from(streamline_binary_name()));
    }

    // -- Settings validation ------------------------------------------------

    fn valid_config() -> ServerConfig {
        ServerConfig {
            host: "127.0.0.1".into(),
            kafka_port: 9092,
            http_port: 9094,
            data_dir: if cfg!(target_os = "windows") {
                "C:\\ProgramData\\streamline".into()
            } else {
                "/var/lib/streamline".into()
            },
            log_level: "info".into(),
        }
    }

    #[test]
    fn test_default_config_is_valid() {
        let config = ServerConfig::default();
        // The OS data directory is absolute on every supported platform; the
        // relative "./data" fallback only appears when HOME/APPDATA is unset.
        if Path::new(&config.data_dir).is_absolute() {
            normalized_config(&config).unwrap();
        }
    }

    #[test]
    fn test_loopback_hosts_are_accepted() {
        for host in ["127.0.0.1", "localhost", "LOCALHOST", "::1", "[::1]"] {
            let config = ServerConfig {
                host: host.into(),
                ..valid_config()
            };
            normalized_config(&config).unwrap_or_else(|e| panic!("{host} should be valid: {e}"));
        }
    }

    #[test]
    fn test_host_is_trimmed_before_persistence_and_use() {
        for raw in ["  127.0.0.1 ", "\t127.0.0.1\n"] {
            let normalized = normalized_config(&ServerConfig {
                host: raw.into(),
                ..valid_config()
            })
            .unwrap_or_else(|e| panic!("{raw:?} should be valid: {e}"));

            assert_eq!(normalized.host, "127.0.0.1");
            assert_eq!(http_base_url(&normalized), "http://127.0.0.1:9094");
            assert_eq!(server_arguments(&normalized)[1], "127.0.0.1:9092");
        }
    }

    /// A bare `::1` is a valid address but not a valid URL authority or
    /// `host:port` pair, so it must be normalized to the bracketed form.
    #[test]
    fn test_bare_ipv6_loopback_is_normalized_to_brackets() {
        for raw in ["::1", " ::1 ", "[::1]", "0:0:0:0:0:0:0:1"] {
            let normalized = normalized_config(&ServerConfig {
                host: raw.into(),
                ..valid_config()
            })
            .unwrap_or_else(|e| panic!("{raw:?} should be valid: {e}"));

            assert_eq!(normalized.host, "[::1]", "{raw:?}");
            assert_eq!(http_base_url(&normalized), "http://[::1]:9094");
            let arguments = server_arguments(&normalized);
            assert_eq!(arguments[1], "[::1]:9092");
            assert_eq!(arguments[3], "[::1]:9094");
            // The normalized authority must round-trip through a real parser.
            reqwest::Url::parse(&http_base_url(&normalized)).unwrap();
            arguments[1].parse::<std::net::SocketAddr>().unwrap();
        }
    }

    #[test]
    fn test_localhost_and_ipv4_keep_working_end_to_end() {
        for (raw, expected) in [
            ("localhost", "localhost"),
            ("LOCALHOST", "LOCALHOST"),
            ("127.0.0.1", "127.0.0.1"),
        ] {
            let normalized = normalized_config(&ServerConfig {
                host: raw.into(),
                ..valid_config()
            })
            .unwrap_or_else(|e| panic!("{raw} should be valid: {e}"));

            assert_eq!(normalized.host, expected);
            assert_eq!(
                http_base_url(&normalized),
                format!("http://{expected}:9094")
            );
            assert_eq!(server_arguments(&normalized)[1], format!("{expected}:9092"));
            reqwest::Url::parse(&http_base_url(&normalized)).unwrap();
        }
    }

    #[test]
    fn test_non_loopback_hosts_are_rejected() {
        for host in ["0.0.0.0", "192.168.1.10", "example.com", ""] {
            let config = ServerConfig {
                host: host.into(),
                ..valid_config()
            };
            assert!(
                normalized_config(&config).is_err(),
                "{host} must not be accepted"
            );
        }
    }

    #[test]
    fn test_malformed_and_non_loopback_literals_are_rejected() {
        for host in [
            "   ",
            "[127.0.0.1]",    // brackets are IPv6-only syntax
            "[::1",           // unbalanced
            "::1]",           // unbalanced
            "[localhost]",    // not an IP literal
            "[::2]",          // not loopback
            "::",             // unspecified, not loopback
            "127.0.0.1:9092", // host only, no port
            "[fe80::1]",      // link-local, not loopback
            "local host",
        ] {
            assert!(
                normalized_config(&ServerConfig {
                    host: host.into(),
                    ..valid_config()
                })
                .is_err(),
                "{host:?} must not be accepted"
            );
        }
    }

    #[test]
    fn test_conflicting_ports_are_rejected() {
        let config = ServerConfig {
            http_port: 9092,
            ..valid_config()
        };
        let error = normalized_config(&config).unwrap_err();
        assert!(error.contains("must differ"), "{error}");
    }

    #[test]
    fn test_zero_ports_are_rejected() {
        let kafka = ServerConfig {
            kafka_port: 0,
            ..valid_config()
        };
        assert!(normalized_config(&kafka).is_err());
        let http = ServerConfig {
            http_port: 0,
            ..valid_config()
        };
        assert!(normalized_config(&http).is_err());
    }

    #[test]
    fn test_empty_and_relative_data_dirs_are_rejected() {
        for data_dir in ["", "   ", "./data", "data"] {
            let config = ServerConfig {
                data_dir: data_dir.into(),
                ..valid_config()
            };
            assert!(
                normalized_config(&config).is_err(),
                "{data_dir:?} must not be accepted"
            );
        }
    }

    #[test]
    fn test_unknown_log_level_is_rejected() {
        let config = ServerConfig {
            log_level: "verbose".into(),
            ..valid_config()
        };
        assert!(normalized_config(&config).is_err());
    }

    // -- Settings loading and recovery --------------------------------------

    /// Unique scratch directory; avoids adding a dev-dependency for temp files.
    fn scratch_dir(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("streamline-desktop-{name}-{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Settings fixture whose `data_dir` is absolute on the current platform.
    /// Hand-written JSON with a Unix path ("/tmp/x") is a *relative* path on
    /// Windows, so such a fixture would be rejected for the wrong reason there.
    /// Serializing through serde also escapes the backslashes in Windows paths.
    fn settings_json_with_host(host: &str) -> String {
        serde_json::to_string(&ServerConfig {
            host: host.into(),
            ..valid_config()
        })
        .unwrap()
    }

    #[test]
    fn test_missing_settings_file_is_a_clean_first_launch() {
        let dir = scratch_dir("missing");
        let loaded = load_settings_from_path(&dir.join("settings.json"));
        assert!(loaded.warning.is_none());
        assert_eq!(loaded.config.kafka_port, 9092);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_valid_settings_file_round_trips() {
        let dir = scratch_dir("valid");
        let path = dir.join("settings.json");
        let config = valid_config();
        std::fs::write(&path, serde_json::to_string(&config).unwrap()).unwrap();

        let loaded = load_settings_from_path(&path);
        assert!(loaded.warning.is_none());
        assert_eq!(loaded.config.data_dir, config.data_dir);
        assert!(path.exists(), "valid settings must not be quarantined");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_corrupt_settings_file_is_quarantined_and_reported() {
        let dir = scratch_dir("corrupt");
        let path = dir.join("settings.json");
        std::fs::write(&path, "{ not json").unwrap();

        let loaded = load_settings_from_path(&path);
        let warning = loaded.warning.expect("corruption must be surfaced");
        assert!(warning.contains("invalid JSON"), "{warning}");
        assert!(!path.exists(), "corrupt file must be moved aside");

        let preserved: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("settings.json.invalid-")
            })
            .collect();
        assert_eq!(preserved.len(), 1, "the original bytes must be preserved");
        assert_eq!(
            std::fs::read_to_string(preserved[0].path()).unwrap(),
            "{ not json"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_invalid_settings_values_are_quarantined() {
        let dir = scratch_dir("invalid-values");
        let path = dir.join("settings.json");
        std::fs::write(&path, settings_json_with_host("0.0.0.0")).unwrap();

        let loaded = load_settings_from_path(&path);
        let warning = loaded.warning.expect("invalid settings must be surfaced");
        assert!(warning.contains("loopback"), "{warning}");
        assert_eq!(loaded.config.host, "127.0.0.1");
        assert!(!path.exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_loaded_settings_host_is_normalized_not_quarantined() {
        let dir = scratch_dir("normalize-host");
        let path = dir.join("settings.json");
        std::fs::write(&path, settings_json_with_host(" ::1 ")).unwrap();

        let loaded = load_settings_from_path(&path);
        assert!(loaded.warning.is_none(), "{:?}", loaded.warning);
        assert_eq!(loaded.config.host, "[::1]");
        assert_eq!(http_base_url(&loaded.config), "http://[::1]:9094");
        assert_eq!(server_arguments(&loaded.config)[1], "[::1]:9092");
        assert!(path.exists(), "a normalizable host must not be quarantined");
        std::fs::remove_dir_all(&dir).ok();
    }
}
