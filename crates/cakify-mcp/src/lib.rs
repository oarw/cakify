//! MCP client actor and tool routing for Cakify.

use std::{
    collections::{HashMap, HashSet},
    env,
    ffi::OsString,
    io::{self, Write},
    path::PathBuf,
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, RecvTimeoutError, SyncSender},
        Arc, RwLock,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use async_channel::{Receiver as AsyncReceiver, Sender as AsyncSender, TrySendError};
use cakify_core::{ToolDefinition, ToolExecutionError, ToolExecutor};
use cakify_storage::{McpServerRecord, McpTransport};
use process_wrap::tokio::{CommandWrap, KillOnDrop};
#[cfg(windows)]
use process_wrap::tokio::{CreationFlags, JobObject};
use rmcp::{
    model::{CallToolRequestParams, CallToolResult, ContentBlock, PaginatedRequestParams},
    service::{Peer, RunningService},
    transport::{which_command, StreamableHttpClientTransport, TokioChildProcess},
    RoleClient, ServiceExt,
};
use serde::Serialize;
use serde_json::Value;
use thiserror::Error;
use url::Url;
#[cfg(windows)]
use windows::Win32::System::Threading::CREATE_NO_WINDOW;

const COMMAND_CAPACITY: usize = 64;
const EVENT_CAPACITY: usize = 256;
const MAX_SERVERS: usize = 32;
const MAX_TOOLS_PER_SERVER: usize = 128;
const MAX_SCHEMA_BYTES: usize = 64 * 1_024;
const MAX_TOOL_ARGUMENT_BYTES: usize = 64 * 1_024;
const MAX_TOOL_RESULT_BYTES: usize = 64 * 1_024;
const MAX_TOOL_NAME_BYTES: usize = 1_024;
const MAX_TOOL_DESCRIPTION_BYTES: usize = 4 * 1_024;
const MAX_SERVER_ID_BYTES: usize = 128;
const MAX_DISPLAY_NAME_BYTES: usize = 200;
const MAX_COMMAND_BYTES: usize = 4 * 1_024;
const MAX_ARGUMENTS: usize = 128;
const MAX_ARGUMENT_BYTES: usize = 4 * 1_024;
const MAX_IN_FLIGHT_CALLS: usize = 8;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const CALL_TIMEOUT: Duration = Duration::from_secs(60);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);
const CANCELLATION_POLL: Duration = Duration::from_millis(50);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum McpTransportConfig {
    Stdio { command: String, args: Vec<String> },
    StreamableHttp { url: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct McpServerConfig {
    pub id: String,
    pub display_name: String,
    pub transport: McpTransportConfig,
}

impl TryFrom<&McpServerRecord> for McpServerConfig {
    type Error = McpConfigError;

    fn try_from(record: &McpServerRecord) -> Result<Self, Self::Error> {
        validate_required_text(&record.id, MAX_SERVER_ID_BYTES)?;
        validate_required_text(&record.display_name, MAX_DISPLAY_NAME_BYTES)?;
        let config: Value = serde_json::from_str(&record.config_json)
            .map_err(|_| McpConfigError::InvalidStoredConfig)?;
        if !config.is_object() {
            return Err(McpConfigError::InvalidStoredConfig);
        }
        let transport = match record.transport {
            McpTransport::Stdio => {
                let command = required_string(&config, "command")?;
                validate_required_text(&command, MAX_COMMAND_BYTES)?;
                let args = match config.get("args") {
                    None => Vec::new(),
                    Some(Value::Array(args)) => args
                        .iter()
                        .map(|argument| {
                            argument
                                .as_str()
                                .map(str::to_owned)
                                .ok_or(McpConfigError::InvalidStoredConfig)
                        })
                        .collect::<Result<Vec<_>, _>>(),
                    Some(_) => return Err(McpConfigError::InvalidStoredConfig),
                };
                if args.len() > MAX_ARGUMENTS
                    || args.iter().any(|argument| {
                        argument.len() > MAX_ARGUMENT_BYTES
                            || argument.chars().any(char::is_control)
                    })
                {
                    return Err(McpConfigError::InvalidStoredConfig);
                }
                McpTransportConfig::Stdio { command, args }
            }
            McpTransport::StreamableHttp => {
                let url = required_string(&config, "url")?;
                validate_streamable_http_url(&url)?;
                McpTransportConfig::StreamableHttp { url }
            }
        };
        Ok(Self {
            id: record.id.clone(),
            display_name: record.display_name.clone(),
            transport,
        })
    }
}

fn validate_required_text(value: &str, max_bytes: usize) -> Result<(), McpConfigError> {
    if value.trim().is_empty() || value.len() > max_bytes || value.chars().any(char::is_control) {
        return Err(McpConfigError::InvalidStoredConfig);
    }
    Ok(())
}

fn validate_streamable_http_url(value: &str) -> Result<(), McpConfigError> {
    validate_required_text(value, 2_048)?;
    let parsed = Url::parse(value).map_err(|_| McpConfigError::InvalidStoredConfig)?;
    let loopback = matches!(
        parsed.host_str(),
        Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
    );
    let allowed_scheme = parsed.scheme() == "https" || (parsed.scheme() == "http" && loopback);
    if !parsed.has_host()
        || !allowed_scheme
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(McpConfigError::InvalidStoredConfig);
    }
    Ok(())
}

fn required_string(config: &Value, field: &str) -> Result<String, McpConfigError> {
    config
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or(McpConfigError::InvalidStoredConfig)
}

#[derive(Debug, Error)]
pub enum McpConfigError {
    #[error("MCP Server 配置不可用")]
    InvalidStoredConfig,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum McpEvent {
    ServerConnecting {
        server_id: String,
    },
    ServerConnected {
        server_id: String,
        tool_count: usize,
    },
    ServerFailed {
        server_id: String,
        message: String,
    },
    ServerDisconnected {
        server_id: String,
    },
}

#[derive(Debug, Error)]
pub enum McpStartError {
    #[error("failed to create MCP async runtime: {0}")]
    Runtime(#[source] std::io::Error),
    #[error("failed to start MCP client thread: {0}")]
    Thread(#[source] std::io::Error),
}

#[derive(Debug, Error)]
pub enum McpDispatchError {
    #[error("MCP command queue is full")]
    Full,
    #[error("MCP client is stopped")]
    Closed,
}

#[derive(Clone)]
pub struct McpHandle {
    commands: AsyncSender<Command>,
    routes: Arc<RwLock<HashMap<String, ToolRoute>>>,
}

impl McpHandle {
    pub fn connect(&self, config: McpServerConfig) -> Result<(), McpDispatchError> {
        self.try_send(Command::Connect { config })
    }

    pub fn disconnect(&self, server_id: impl Into<String>) -> Result<(), McpDispatchError> {
        self.try_send(Command::Disconnect {
            server_id: server_id.into(),
        })
    }

    pub fn tool_definitions(&self) -> Vec<ToolDefinition> {
        let Ok(routes) = self.routes.read() else {
            return Vec::new();
        };
        let mut definitions = routes
            .values()
            .map(|route| route.definition.clone())
            .collect::<Vec<_>>();
        definitions.sort_by(|left, right| left.name.cmp(&right.name));
        definitions
    }

    fn try_send(&self, command: Command) -> Result<(), McpDispatchError> {
        self.commands
            .try_send(command)
            .map_err(|error| match error {
                TrySendError::Full(_) => McpDispatchError::Full,
                TrySendError::Closed(_) => McpDispatchError::Closed,
            })
    }
}

impl ToolExecutor for McpHandle {
    fn execute(
        &self,
        name: &str,
        arguments_json: &str,
        cancellation: Arc<AtomicBool>,
    ) -> Result<String, ToolExecutionError> {
        if cancellation.load(Ordering::Acquire) {
            return Err(ToolExecutionError::new("MCP 工具调用已取消"));
        }
        if arguments_json.len() > MAX_TOOL_ARGUMENT_BYTES {
            return Err(ToolExecutionError::new("MCP 工具参数超过安全上限"));
        }
        if !self
            .routes
            .read()
            .is_ok_and(|routes| routes.contains_key(name))
        {
            return Err(ToolExecutionError::new("请求的 MCP 工具不可用"));
        }
        let (reply, receiver) = mpsc::sync_channel(1);
        self.commands
            .try_send(Command::Execute {
                name: name.to_owned(),
                arguments_json: arguments_json.to_owned(),
                cancellation: cancellation.clone(),
                reply,
            })
            .map_err(|error| match error {
                TrySendError::Full(_) => ToolExecutionError::new("MCP 命令队列繁忙"),
                TrySendError::Closed(_) => ToolExecutionError::new("MCP client 已停止"),
            })?;
        loop {
            if cancellation.load(Ordering::Acquire) {
                return Err(ToolExecutionError::new("MCP 工具调用已取消"));
            }
            match receiver.recv_timeout(CANCELLATION_POLL) {
                Ok(result) => return result,
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(ToolExecutionError::new("MCP 工具执行通道已关闭"));
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct McpEvents {
    events: AsyncReceiver<McpEvent>,
}

impl McpEvents {
    pub fn receiver(&self) -> AsyncReceiver<McpEvent> {
        self.events.clone()
    }
}

pub struct McpRuntime {
    handle: McpHandle,
    events: McpEvents,
    shutdown: AsyncSender<()>,
    join: Option<JoinHandle<()>>,
}

impl McpRuntime {
    pub fn start() -> Result<Self, McpStartError> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(McpStartError::Runtime)?;
        let (commands, receiver) = async_channel::bounded(COMMAND_CAPACITY);
        let (shutdown, shutdown_receiver) = async_channel::bounded(1);
        let (event_sender, events) = async_channel::bounded(EVENT_CAPACITY);
        let routes = Arc::new(RwLock::new(HashMap::new()));
        let actor_routes = routes.clone();
        let join = thread::Builder::new()
            .name("cakify-mcp".to_owned())
            .spawn(move || {
                runtime.block_on(run_actor(
                    receiver,
                    shutdown_receiver,
                    event_sender,
                    actor_routes,
                ));
            })
            .map_err(McpStartError::Thread)?;
        Ok(Self {
            handle: McpHandle { commands, routes },
            events: McpEvents { events },
            shutdown,
            join: Some(join),
        })
    }

    pub fn handle(&self) -> McpHandle {
        self.handle.clone()
    }

    pub fn events(&self) -> McpEvents {
        self.events.clone()
    }
}

impl Drop for McpRuntime {
    fn drop(&mut self) {
        let _ = self.shutdown.try_send(());
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

enum Command {
    Connect {
        config: McpServerConfig,
    },
    Disconnect {
        server_id: String,
    },
    Execute {
        name: String,
        arguments_json: String,
        cancellation: Arc<AtomicBool>,
        reply: SyncSender<Result<String, ToolExecutionError>>,
    },
}

#[derive(Clone)]
struct ToolRoute {
    server_id: String,
    remote_name: String,
    definition: ToolDefinition,
}

type Client = RunningService<RoleClient, ()>;

struct ConnectedServer {
    client: Client,
    peer: Peer<RoleClient>,
}

impl ConnectedServer {
    fn new(client: Client) -> Self {
        let peer = client.peer().clone();
        Self { client, peer }
    }
}

enum BackgroundResult {
    Connected {
        server_id: String,
        generation: u64,
        result: Result<(Client, Vec<ToolRoute>), String>,
    },
    Executed,
    Closed,
}

async fn run_actor(
    commands: AsyncReceiver<Command>,
    shutdown: AsyncReceiver<()>,
    events: AsyncSender<McpEvent>,
    routes: Arc<RwLock<HashMap<String, ToolRoute>>>,
) {
    let mut servers = HashMap::<String, ConnectedServer>::new();
    let mut desired = HashMap::<String, u64>::new();
    let mut pending_connects = HashMap::<String, (u64, tokio::task::AbortHandle)>::new();
    let mut tasks = tokio::task::JoinSet::<BackgroundResult>::new();
    let mut generation = 0_u64;
    let mut in_flight_calls = 0_usize;

    loop {
        tokio::select! {
            _ = shutdown.recv() => break,
            command = commands.recv() => {
                let Ok(command) = command else {
                    break;
                };
                match command {
                    Command::Connect { config } => {
                        if let Err(message) = validate_server_config(&config) {
                            emit(&events, McpEvent::ServerFailed {
                                server_id: config.id,
                                message,
                            });
                            continue;
                        }
                        if desired.len() >= MAX_SERVERS && !desired.contains_key(&config.id) {
                            emit(&events, McpEvent::ServerFailed {
                                server_id: config.id,
                                message: "MCP Server 数量超过安全上限".to_owned(),
                            });
                            continue;
                        }

                        generation = generation.wrapping_add(1).max(1);
                        let current_generation = generation;
                        desired.insert(config.id.clone(), current_generation);
                        if let Some((_, pending)) = pending_connects.remove(&config.id) {
                            pending.abort();
                        }
                        remove_routes(&routes, &config.id);
                        if let Some(server) = servers.remove(&config.id) {
                            spawn_close(&mut tasks, server.client);
                        }
                        emit(&events, McpEvent::ServerConnecting {
                            server_id: config.id.clone(),
                        });

                        let server_id = config.id.clone();
                        let pending_server_id = server_id.clone();
                        let pending = tasks.spawn(async move {
                            let result = connect_server(&config).await;
                            BackgroundResult::Connected {
                                server_id,
                                generation: current_generation,
                                result,
                            }
                        });
                        pending_connects
                            .insert(pending_server_id, (current_generation, pending));
                    }
                    Command::Disconnect { server_id } => {
                        desired.remove(&server_id);
                        if let Some((_, pending)) = pending_connects.remove(&server_id) {
                            pending.abort();
                        }
                        remove_routes(&routes, &server_id);
                        if let Some(server) = servers.remove(&server_id) {
                            spawn_close(&mut tasks, server.client);
                        }
                        emit(&events, McpEvent::ServerDisconnected { server_id });
                    }
                    Command::Execute {
                        name,
                        arguments_json,
                        cancellation,
                        reply,
                    } => {
                        if in_flight_calls >= MAX_IN_FLIGHT_CALLS {
                            let _ = reply.send(Err(ToolExecutionError::new(
                                "MCP 工具并发数达到安全上限",
                            )));
                            continue;
                        }
                        let resolved = resolve_call(&servers, &routes, &name);
                        match resolved {
                            Ok((peer, route)) => {
                                in_flight_calls += 1;
                                let _ = tasks.spawn(async move {
                                    let result = execute_tool(
                                        peer,
                                        route,
                                        arguments_json,
                                        cancellation,
                                    )
                                    .await;
                                    let _ = reply.send(result);
                                    BackgroundResult::Executed
                                });
                            }
                            Err(error) => {
                                let _ = reply.send(Err(error));
                            }
                        }
                    }
                }
            }
            completed = tasks.join_next(), if !tasks.is_empty() => {
                match completed {
                    Some(Ok(BackgroundResult::Executed)) => {
                        in_flight_calls = in_flight_calls.saturating_sub(1);
                    }
                    Some(Ok(completed)) => {
                        handle_background(
                            completed,
                            &desired,
                            &mut pending_connects,
                            &mut servers,
                            &mut tasks,
                            &events,
                            &routes,
                        );
                    }
                    Some(Err(_)) | None => {}
                }
            }
        }
    }

    if let Ok(mut routes) = routes.write() {
        routes.clear();
    }
    tasks.abort_all();
    let mut clients = servers
        .drain()
        .map(|(_, server)| server.client)
        .collect::<Vec<_>>();
    while let Some(completed) = tasks.join_next().await {
        if let Ok(BackgroundResult::Connected {
            result: Ok((client, _)),
            ..
        }) = completed
        {
            clients.push(client);
        }
    }
    close_clients(clients).await;
}

fn validate_server_config(config: &McpServerConfig) -> Result<(), String> {
    validate_required_text(&config.id, MAX_SERVER_ID_BYTES)
        .map_err(|_| "MCP Server ID 不可用".to_owned())?;
    validate_required_text(&config.display_name, MAX_DISPLAY_NAME_BYTES)
        .map_err(|_| "MCP Server 名称不可用".to_owned())?;
    match &config.transport {
        McpTransportConfig::Stdio { command, args } => {
            validate_required_text(command, MAX_COMMAND_BYTES)
                .map_err(|_| "MCP stdio 命令不可用".to_owned())?;
            if args.len() > MAX_ARGUMENTS
                || args.iter().any(|argument| {
                    argument.len() > MAX_ARGUMENT_BYTES || argument.chars().any(char::is_control)
                })
            {
                return Err("MCP stdio 参数超过安全上限".to_owned());
            }
        }
        McpTransportConfig::StreamableHttp { url } => {
            validate_streamable_http_url(url)
                .map_err(|_| "MCP Streamable HTTP URL 不可用".to_owned())?;
        }
    }
    Ok(())
}

fn resolve_call(
    servers: &HashMap<String, ConnectedServer>,
    routes: &RwLock<HashMap<String, ToolRoute>>,
    name: &str,
) -> Result<(Peer<RoleClient>, ToolRoute), ToolExecutionError> {
    let route = routes
        .read()
        .ok()
        .and_then(|routes| routes.get(name).cloned())
        .ok_or_else(|| ToolExecutionError::new("请求的 MCP 工具不可用"))?;
    let peer = servers
        .get(&route.server_id)
        .map(|server| server.peer.clone())
        .ok_or_else(|| ToolExecutionError::new("MCP Server 未连接"))?;
    Ok((peer, route))
}

fn handle_background(
    completed: BackgroundResult,
    desired: &HashMap<String, u64>,
    pending_connects: &mut HashMap<String, (u64, tokio::task::AbortHandle)>,
    servers: &mut HashMap<String, ConnectedServer>,
    tasks: &mut tokio::task::JoinSet<BackgroundResult>,
    events: &AsyncSender<McpEvent>,
    routes: &RwLock<HashMap<String, ToolRoute>>,
) {
    let BackgroundResult::Connected {
        server_id,
        generation,
        result,
    } = completed
    else {
        return;
    };

    if pending_connects
        .get(&server_id)
        .is_some_and(|(pending_generation, _)| *pending_generation == generation)
    {
        pending_connects.remove(&server_id);
    }

    if desired.get(&server_id) != Some(&generation) {
        if let Ok((client, _)) = result {
            spawn_close(tasks, client);
        }
        return;
    }

    match result {
        Ok((client, discovered)) => {
            let tool_count = discovered.len();
            if let Err(message) = replace_routes(routes, &server_id, discovered) {
                spawn_close(tasks, client);
                emit(events, McpEvent::ServerFailed { server_id, message });
                return;
            }
            if let Some(previous) = servers.insert(server_id.clone(), ConnectedServer::new(client))
            {
                spawn_close(tasks, previous.client);
            }
            emit(
                events,
                McpEvent::ServerConnected {
                    server_id,
                    tool_count,
                },
            );
        }
        Err(message) => {
            remove_routes(routes, &server_id);
            emit(events, McpEvent::ServerFailed { server_id, message });
        }
    }
}

fn spawn_close(tasks: &mut tokio::task::JoinSet<BackgroundResult>, mut client: Client) {
    let _ = tasks.spawn(async move {
        let _ = client.close_with_timeout(SHUTDOWN_TIMEOUT).await;
        BackgroundResult::Closed
    });
}

async fn close_clients(clients: Vec<Client>) {
    let mut closing = tokio::task::JoinSet::new();
    for mut client in clients {
        let _ = closing.spawn(async move {
            let _ = client.close_with_timeout(SHUTDOWN_TIMEOUT).await;
        });
    }
    let close_all = async { while let Some(_result) = closing.join_next().await {} };
    if tokio::time::timeout(SHUTDOWN_TIMEOUT + Duration::from_millis(250), close_all)
        .await
        .is_err()
    {
        closing.abort_all();
    }
}

async fn connect_server(config: &McpServerConfig) -> Result<(Client, Vec<ToolRoute>), String> {
    let connect = async {
        match &config.transport {
            McpTransportConfig::Stdio { command, args } => {
                let workdir = mcp_working_directory(&config.id)?;
                std::fs::create_dir_all(&workdir)
                    .map_err(|_| "无法创建 MCP 隔离工作目录".to_owned())?;
                let mut process =
                    which_command(command).map_err(|_| "无法解析 MCP stdio 命令".to_owned())?;
                process
                    .args(args)
                    .env_clear()
                    .envs(allowlisted_environment())
                    .env("CAKIFY_MCP_SERVER_ID", &config.id)
                    .current_dir(workdir)
                    .stderr(Stdio::null());
                let mut command = CommandWrap::from(process);
                #[cfg(windows)]
                command.wrap(CreationFlags(CREATE_NO_WINDOW));
                command.wrap(KillOnDrop);
                #[cfg(windows)]
                command.wrap(JobObject);
                let transport = TokioChildProcess::new(command)
                    .map_err(|_| "无法启动 MCP stdio Server".to_owned())?;
                ().serve(transport)
                    .await
                    .map_err(|_| "MCP stdio 初始化失败".to_owned())
            }
            McpTransportConfig::StreamableHttp { url } => {
                let transport = StreamableHttpClientTransport::from_uri(url.clone());
                ().serve(transport)
                    .await
                    .map_err(|_| "MCP Streamable HTTP 初始化失败".to_owned())
            }
        }
    };
    let mut client = tokio::time::timeout(CONNECT_TIMEOUT, connect)
        .await
        .map_err(|_| "MCP Server 连接超时".to_owned())??;
    let peer = client.peer().clone();
    let tools = match tokio::time::timeout(CONNECT_TIMEOUT, list_tools_bounded(&peer)).await {
        Ok(Ok(tools)) => tools,
        Ok(Err(message)) => {
            let _ = client.close_with_timeout(SHUTDOWN_TIMEOUT).await;
            return Err(message);
        }
        Err(_) => {
            let _ = client.close_with_timeout(SHUTDOWN_TIMEOUT).await;
            return Err("MCP 工具发现超时".to_owned());
        }
    };
    let routes = match build_routes(config, tools) {
        Ok(routes) => routes,
        Err(message) => {
            let _ = client.close_with_timeout(SHUTDOWN_TIMEOUT).await;
            return Err(message);
        }
    };
    Ok((client, routes))
}

async fn list_tools_bounded(peer: &Peer<RoleClient>) -> Result<Vec<rmcp::model::Tool>, String> {
    let mut tools = Vec::new();
    let mut cursor = None;
    loop {
        let page = peer
            .list_tools(Some(PaginatedRequestParams::default().with_cursor(cursor)))
            .await
            .map_err(|_| "MCP 工具发现失败".to_owned())?;
        if page.tools.len() > MAX_TOOLS_PER_SERVER.saturating_sub(tools.len()) {
            return Err("MCP Server 返回的工具数量超过安全上限".to_owned());
        }
        tools.extend(page.tools);
        cursor = page.next_cursor;
        if cursor.is_none() {
            return Ok(tools);
        }
    }
}

fn allowlisted_environment() -> Vec<(OsString, OsString)> {
    const NAMES: &[&str] = &[
        "APPDATA",
        "COMSPEC",
        "HOME",
        "HOMEDRIVE",
        "HOMEPATH",
        "LANG",
        "LC_ALL",
        "LOCALAPPDATA",
        "PATH",
        "PATHEXT",
        "PROGRAMDATA",
        "SYSTEMROOT",
        "TEMP",
        "TMP",
        "TMPDIR",
        "USERPROFILE",
        "WINDIR",
    ];
    NAMES
        .iter()
        .filter_map(|name| env::var_os(name).map(|value| (OsString::from(name), value)))
        .collect()
}

fn mcp_working_directory(server_id: &str) -> Result<PathBuf, String> {
    let base = env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir);
    if base.as_os_str().is_empty() {
        return Err("MCP 隔离工作目录不可用".to_owned());
    }
    Ok(base.join("Cakify").join("mcp-work").join(format!(
        "{:016x}",
        stable_route_hash(server_id, "workspace")
    )))
}

fn build_routes(
    config: &McpServerConfig,
    tools: Vec<rmcp::model::Tool>,
) -> Result<Vec<ToolRoute>, String> {
    if tools.len() > MAX_TOOLS_PER_SERVER {
        return Err("MCP Server 返回的工具数量超过安全上限".to_owned());
    }
    let mut names = HashSet::with_capacity(tools.len());
    tools
        .into_iter()
        .map(|tool| {
            let remote_name = tool.name.into_owned();
            if remote_name.trim().is_empty()
                || remote_name.len() > MAX_TOOL_NAME_BYTES
                || remote_name.chars().any(char::is_control)
            {
                return Err("MCP 工具名称不可用或超过安全上限".to_owned());
            }
            let name = namespaced_tool_name(&config.id, &remote_name);
            if !names.insert(name.clone()) {
                return Err("MCP 工具名称发生冲突".to_owned());
            }
            let parameters_json = encode_json_bounded(&*tool.input_schema, MAX_SCHEMA_BYTES)
                .map_err(|_| "MCP 工具 Schema 超过安全上限".to_owned())?;
            let description = tool
                .description
                .map(|description| description.into_owned())
                .or(tool.title)
                .unwrap_or_else(|| format!("MCP tool from {}", config.display_name));
            if description.len() > MAX_TOOL_DESCRIPTION_BYTES {
                return Err("MCP 工具描述超过安全上限".to_owned());
            }
            if description.chars().any(char::is_control) {
                return Err("MCP 工具描述包含控制字符".to_owned());
            }
            Ok(ToolRoute {
                server_id: config.id.clone(),
                remote_name,
                definition: ToolDefinition {
                    name,
                    description,
                    parameters_json,
                },
            })
        })
        .collect()
}

async fn execute_tool(
    peer: Peer<RoleClient>,
    route: ToolRoute,
    arguments_json: String,
    cancellation: Arc<AtomicBool>,
) -> Result<String, ToolExecutionError> {
    if cancellation.load(Ordering::Acquire) {
        return Err(ToolExecutionError::new("MCP 工具调用已取消"));
    }
    if arguments_json.len() > MAX_TOOL_ARGUMENT_BYTES {
        return Err(ToolExecutionError::new("MCP 工具参数超过安全上限"));
    }
    let arguments = serde_json::from_str::<Value>(&arguments_json)
        .ok()
        .and_then(Value::as_object)
        .cloned()
        .ok_or_else(|| ToolExecutionError::new("MCP 工具参数不是 JSON object"))?;
    let call =
        peer.call_tool(CallToolRequestParams::new(route.remote_name).with_arguments(arguments));
    tokio::pin!(call);
    tokio::select! {
        result = &mut call => {
            let result = result.map_err(|_| ToolExecutionError::new("MCP 工具调用失败"))?;
            project_tool_result(&result)
        }
        _ = wait_for_cancellation(cancellation) => {
            Err(ToolExecutionError::new("MCP 工具调用已取消"))
        }
        _ = tokio::time::sleep(CALL_TIMEOUT) => {
            Err(ToolExecutionError::new("MCP 工具调用超时"))
        }
    }
}

fn project_tool_result(result: &CallToolResult) -> Result<String, ToolExecutionError> {
    if result.is_error == Some(true) {
        return Err(ToolExecutionError::new("MCP 工具返回了错误"));
    }
    if let Some(structured) = &result.structured_content {
        return encode_json_bounded(structured, MAX_TOOL_RESULT_BYTES)
            .map_err(|_| ToolExecutionError::new("MCP 工具结果超过安全上限"));
    }

    let mut output = String::new();
    let mut omitted_non_text = false;
    for content in &result.content {
        if let ContentBlock::Text(text) = content {
            let separator = usize::from(!output.is_empty());
            let next_len = output
                .len()
                .checked_add(separator)
                .and_then(|len| len.checked_add(text.text.len()))
                .ok_or_else(|| ToolExecutionError::new("MCP 工具结果超过安全上限"))?;
            if next_len > MAX_TOOL_RESULT_BYTES {
                return Err(ToolExecutionError::new("MCP 工具结果超过安全上限"));
            }
            if separator != 0 {
                output.push('\n');
            }
            output.push_str(&text.text);
        } else {
            omitted_non_text = true;
        }
    }
    if output.is_empty() && omitted_non_text {
        Ok(r#"{"content":"non_text_content_omitted"}"#.to_owned())
    } else if output.is_empty() {
        Ok("{}".to_owned())
    } else {
        Ok(output)
    }
}

struct BoundedJsonWriter {
    bytes: Vec<u8>,
    limit: usize,
}

impl Write for BoundedJsonWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if buffer.len() > self.limit.saturating_sub(self.bytes.len()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "JSON exceeds configured limit",
            ));
        }
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn encode_json_bounded<T: Serialize>(value: &T, limit: usize) -> Result<String, ()> {
    let mut writer = BoundedJsonWriter {
        bytes: Vec::with_capacity(limit.min(4 * 1_024)),
        limit,
    };
    serde_json::to_writer(&mut writer, value).map_err(|_| ())?;
    String::from_utf8(writer.bytes).map_err(|_| ())
}

async fn wait_for_cancellation(cancellation: Arc<AtomicBool>) {
    while !cancellation.load(Ordering::Acquire) {
        tokio::time::sleep(CANCELLATION_POLL).await;
    }
}

fn replace_routes(
    routes: &RwLock<HashMap<String, ToolRoute>>,
    server_id: &str,
    replacements: Vec<ToolRoute>,
) -> Result<(), String> {
    let mut routes = routes
        .write()
        .map_err(|_| "MCP 工具路由状态不可用".to_owned())?;
    if replacements.iter().any(|replacement| {
        routes
            .get(&replacement.definition.name)
            .is_some_and(|existing| existing.server_id != server_id)
    }) {
        return Err("MCP 工具名称与其他 Server 冲突".to_owned());
    }
    routes.retain(|_, route| route.server_id != server_id);
    for route in replacements {
        routes.insert(route.definition.name.clone(), route);
    }
    Ok(())
}

fn remove_routes(routes: &RwLock<HashMap<String, ToolRoute>>, server_id: &str) {
    if let Ok(mut routes) = routes.write() {
        routes.retain(|_, route| route.server_id != server_id);
    }
}

fn emit(events: &AsyncSender<McpEvent>, event: McpEvent) {
    // UI 状态事件不能反向阻塞网络 actor 或应用关闭。
    let _ = events.try_send(event);
}

fn namespaced_tool_name(server_id: &str, tool_name: &str) -> String {
    let server = sanitize_name(server_id, 16);
    let tool = sanitize_name(tool_name, 24);
    let hash = stable_route_hash(server_id, tool_name);
    format!("mcp_{server}_{tool}_{hash:016x}")
}

fn stable_route_hash(server_id: &str, tool_name: &str) -> u64 {
    // FNV-1a with length framing keeps names stable across Rust releases.
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in (server_id.len() as u64)
        .to_le_bytes()
        .into_iter()
        .chain(server_id.bytes())
        .chain((tool_name.len() as u64).to_le_bytes())
        .chain(tool_name.bytes())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn sanitize_name(value: &str, limit: usize) -> String {
    let mut sanitized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '_' | '-') {
                character
            } else {
                '_'
            }
        })
        .take(limit)
        .collect::<String>();
    if sanitized.is_empty() {
        sanitized.push_str("tool");
    }
    sanitized
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::Tool;
    use serde_json::{json, Map};

    fn server_config(id: &str) -> McpServerConfig {
        McpServerConfig {
            id: id.to_owned(),
            display_name: format!("Server {id}"),
            transport: McpTransportConfig::Stdio {
                command: "server.exe".to_owned(),
                args: Vec::new(),
            },
        }
    }

    fn storage_record(transport: McpTransport, config_json: impl Into<String>) -> McpServerRecord {
        McpServerRecord {
            id: "server".to_owned(),
            display_name: "Server".to_owned(),
            transport,
            config_json: config_json.into(),
            enabled: true,
            capabilities_json: None,
            schema_hash: None,
            last_error: None,
            created_at: 1,
            updated_at: 1,
        }
    }

    fn tool(name: impl Into<String>, description: impl Into<String>, schema: Value) -> Tool {
        let schema = serde_json::from_value::<Map<String, Value>>(schema).expect("object schema");
        Tool::new(name.into(), description.into(), schema)
    }

    #[test]
    fn namespaced_tool_names_are_bounded_valid_and_collision_resistant() {
        let first = namespaced_tool_name("server alpha", "search/files");
        let second = namespaced_tool_name("server alpha", "search_files");
        assert_ne!(first, second);
        assert!(first.len() <= 64);
        assert!(first
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-')));
        assert_eq!(first, "mcp_server_alpha_search_files_ac91c88a9c4c857a");
    }

    #[test]
    fn route_building_rejects_duplicate_names_and_unbounded_metadata() {
        let config = server_config("alpha");
        let duplicate = vec![
            tool("search", "one", json!({"type": "object"})),
            tool("search", "two", json!({"type": "object"})),
        ];
        assert!(build_routes(&config, duplicate).is_err());

        let long_description = "x".repeat(MAX_TOOL_DESCRIPTION_BYTES + 1);
        assert!(build_routes(
            &config,
            vec![tool(
                "oversized",
                long_description,
                json!({"type": "object"})
            )]
        )
        .is_err());

        let long_schema_value = "x".repeat(MAX_SCHEMA_BYTES);
        assert!(build_routes(
            &config,
            vec![tool(
                "oversized_schema",
                "schema",
                json!({"type": "object", "const": long_schema_value})
            )]
        )
        .is_err());
    }

    #[test]
    fn route_replacement_is_atomic_across_servers() {
        let routes = RwLock::new(HashMap::new());
        let definition = ToolDefinition {
            name: "mcp_collision".to_owned(),
            description: "first".to_owned(),
            parameters_json: "{}".to_owned(),
        };
        replace_routes(
            &routes,
            "first",
            vec![ToolRoute {
                server_id: "first".to_owned(),
                remote_name: "tool".to_owned(),
                definition: definition.clone(),
            }],
        )
        .expect("first route");
        let result = replace_routes(
            &routes,
            "second",
            vec![ToolRoute {
                server_id: "second".to_owned(),
                remote_name: "other".to_owned(),
                definition,
            }],
        );
        assert!(result.is_err());
        assert_eq!(
            routes
                .read()
                .expect("routes")
                .get("mcp_collision")
                .expect("original route")
                .server_id,
            "first"
        );
    }

    #[test]
    fn storage_records_convert_to_typed_configs() {
        let record = storage_record(
            McpTransport::Stdio,
            r#"{"command":"server.exe","args":["--stdio"]}"#,
        );
        assert_eq!(
            McpServerConfig::try_from(&record)
                .expect("typed config")
                .transport,
            McpTransportConfig::Stdio {
                command: "server.exe".to_owned(),
                args: vec!["--stdio".to_owned()],
            }
        );
    }

    #[test]
    fn malformed_stored_configs_are_rejected_defensively() {
        for config_json in [
            r#"{"command":"","args":[]}"#,
            r#"{"command":"server.exe","args":"--stdio"}"#,
            r#"{"command":"server.exe","args":[1]}"#,
            "[]",
        ] {
            let record = storage_record(McpTransport::Stdio, config_json);
            assert!(McpServerConfig::try_from(&record).is_err(), "{config_json}");
        }

        for url in [
            "",
            "http://example.com/mcp",
            "https://user:secret@example.com/mcp",
            "https://example.com/mcp?token=secret",
        ] {
            let record = storage_record(
                McpTransport::StreamableHttp,
                json!({ "url": url }).to_string(),
            );
            assert!(McpServerConfig::try_from(&record).is_err(), "{url}");
        }
        for url in [
            "https://example.com/mcp",
            "http://localhost:3000/mcp",
            "http://127.0.0.1:3000/mcp",
        ] {
            let record = storage_record(
                McpTransport::StreamableHttp,
                json!({ "url": url }).to_string(),
            );
            assert!(McpServerConfig::try_from(&record).is_ok(), "{url}");
        }
    }

    #[test]
    fn direct_configs_are_checked_before_connecting() {
        let mut config = server_config(" ");
        assert!(validate_server_config(&config).is_err());
        config.id = "server".to_owned();
        config.transport = McpTransportConfig::Stdio {
            command: "server.exe".to_owned(),
            args: vec!["x".repeat(MAX_ARGUMENT_BYTES + 1)],
        };
        assert!(validate_server_config(&config).is_err());
    }

    #[test]
    fn idle_runtime_starts_and_stops_without_external_processes() {
        let runtime = McpRuntime::start().expect("start runtime");
        assert!(runtime.handle().tool_definitions().is_empty());
        drop(runtime);
    }

    #[test]
    fn dropping_runtime_closes_held_handles() {
        let runtime = McpRuntime::start().expect("start runtime");
        let handle = runtime.handle();
        drop(runtime);
        assert!(matches!(
            handle.disconnect("server"),
            Err(McpDispatchError::Closed)
        ));
    }

    #[test]
    fn pre_cancelled_tool_call_returns_without_dispatch() {
        let runtime = McpRuntime::start().expect("start runtime");
        let cancellation = Arc::new(AtomicBool::new(true));
        let error = runtime
            .handle()
            .execute("missing", "{}", cancellation)
            .expect_err("cancelled");
        assert_eq!(error.to_string(), "MCP 工具调用已取消");
    }

    #[test]
    fn full_event_queue_never_blocks_the_actor() {
        let (sender, receiver) = async_channel::bounded(1);
        emit(
            &sender,
            McpEvent::ServerDisconnected {
                server_id: "first".to_owned(),
            },
        );
        emit(
            &sender,
            McpEvent::ServerDisconnected {
                server_id: "second".to_owned(),
            },
        );
        assert_eq!(receiver.len(), 1);
    }

    #[test]
    fn model_facing_tool_results_exclude_metadata_and_binary_content() {
        let mut result = CallToolResult::default();
        result.content = vec![
            ContentBlock::text("visible text"),
            ContentBlock::image("synthetic-base64", "image/png"),
        ];
        result.meta = Some(Default::default());
        assert_eq!(
            project_tool_result(&result).expect("projection"),
            "visible text"
        );

        result.is_error = Some(true);
        assert_eq!(
            project_tool_result(&result)
                .expect_err("tool error")
                .to_string(),
            "MCP 工具返回了错误"
        );
    }

    #[test]
    fn bounded_json_encoder_rejects_oversized_output() {
        let value = json!({ "payload": "x".repeat(128) });
        assert!(encode_json_bounded(&value, 32).is_err());
        assert_eq!(
            encode_json_bounded(&json!({ "ok": true }), 32).expect("bounded JSON"),
            r#"{"ok":true}"#
        );
    }
}
