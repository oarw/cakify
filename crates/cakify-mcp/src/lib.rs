//! MCP client actor and tool routing for Cakify.

use std::{
    collections::{hash_map::DefaultHasher, HashMap, HashSet},
    hash::{Hash, Hasher},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, RecvTimeoutError, SyncSender, TrySendError},
        Arc, RwLock,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use async_channel::{Receiver as EventReceiver, Sender as EventSender};
use cakify_core::{ToolDefinition, ToolExecutionError, ToolExecutor};
use cakify_storage::{McpServerRecord, McpTransport};
use process_wrap::tokio::{CommandWrap, KillOnDrop};
#[cfg(windows)]
use process_wrap::tokio::JobObject;
use rmcp::{
    model::CallToolRequestParams,
    service::RunningService,
    transport::{StreamableHttpClientTransport, TokioChildProcess},
    RoleClient, ServiceExt,
};
use serde_json::Value;
use thiserror::Error;

const COMMAND_CAPACITY: usize = 64;
const EVENT_CAPACITY: usize = 256;
const MAX_SERVERS: usize = 32;
const MAX_TOOLS_PER_SERVER: usize = 128;
const MAX_SCHEMA_BYTES: usize = 64 * 1_024;
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
        let config: Value = serde_json::from_str(&record.config_json)
            .map_err(|_| McpConfigError::InvalidStoredConfig)?;
        let transport = match record.transport {
            McpTransport::Stdio => {
                let command = required_string(&config, "command")?;
                let args = config
                    .get("args")
                    .and_then(Value::as_array)
                    .map(|args| {
                        args.iter()
                            .map(|argument| {
                                argument
                                    .as_str()
                                    .map(str::to_owned)
                                    .ok_or(McpConfigError::InvalidStoredConfig)
                            })
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .transpose()?
                    .unwrap_or_default();
                McpTransportConfig::Stdio { command, args }
            }
            McpTransport::StreamableHttp => McpTransportConfig::StreamableHttp {
                url: required_string(&config, "url")?,
            },
        };
        Ok(Self {
            id: record.id.clone(),
            display_name: record.display_name.clone(),
            transport,
        })
    }
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
    ServerConnecting { server_id: String },
    ServerConnected { server_id: String, tool_count: usize },
    ServerFailed { server_id: String, message: String },
    ServerDisconnected { server_id: String },
}

#[derive(Debug, Error)]
pub enum McpStartError {
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
    commands: SyncSender<Command>,
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
        self.commands.try_send(command).map_err(|error| match error {
            TrySendError::Full(_) => McpDispatchError::Full,
            TrySendError::Disconnected(_) => McpDispatchError::Closed,
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
                TrySendError::Disconnected(_) => ToolExecutionError::new("MCP client 已停止"),
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
    events: EventReceiver<McpEvent>,
}

impl McpEvents {
    pub fn receiver(&self) -> EventReceiver<McpEvent> {
        self.events.clone()
    }
}

pub struct McpRuntime {
    handle: McpHandle,
    events: McpEvents,
    join: Option<JoinHandle<()>>,
}

impl McpRuntime {
    pub fn start() -> Result<Self, McpStartError> {
        let (commands, receiver) = mpsc::sync_channel(COMMAND_CAPACITY);
        let (event_sender, events) = async_channel::bounded(EVENT_CAPACITY);
        let routes = Arc::new(RwLock::new(HashMap::new()));
        let actor_routes = routes.clone();
        let join = thread::Builder::new()
            .name("cakify-mcp".to_owned())
            .spawn(move || run_actor(receiver, event_sender, actor_routes))
            .map_err(McpStartError::Thread)?;
        Ok(Self {
            handle: McpHandle { commands, routes },
            events: McpEvents { events },
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
        let _ = self.handle.commands.send(Command::Shutdown);
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
    Shutdown,
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
}

fn run_actor(
    commands: mpsc::Receiver<Command>,
    events: EventSender<McpEvent>,
    routes: Arc<RwLock<HashMap<String, ToolRoute>>>,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => return,
    };
    let mut servers = HashMap::<String, ConnectedServer>::new();
    while let Ok(command) = commands.recv() {
        match command {
            Command::Connect { config } => {
                if servers.len() >= MAX_SERVERS && !servers.contains_key(&config.id) {
                    emit(
                        &events,
                        McpEvent::ServerFailed {
                            server_id: config.id,
                            message: "MCP Server 数量超过安全上限".to_owned(),
                        },
                    );
                    continue;
                }
                emit(
                    &events,
                    McpEvent::ServerConnecting {
                        server_id: config.id.clone(),
                    },
                );
                disconnect_server(&runtime, &config.id, &mut servers, &routes);
                match runtime.block_on(connect_server(&config)) {
                    Ok((client, discovered)) => {
                        let tool_count = discovered.len();
                        replace_routes(&routes, &config.id, discovered);
                        servers.insert(config.id.clone(), ConnectedServer { client });
                        emit(
                            &events,
                            McpEvent::ServerConnected {
                                server_id: config.id,
                                tool_count,
                            },
                        );
                    }
                    Err(message) => {
                        remove_routes(&routes, &config.id);
                        emit(
                            &events,
                            McpEvent::ServerFailed {
                                server_id: config.id,
                                message,
                            },
                        );
                    }
                }
            }
            Command::Disconnect { server_id } => {
                disconnect_server(&runtime, &server_id, &mut servers, &routes);
                emit(&events, McpEvent::ServerDisconnected { server_id });
            }
            Command::Execute {
                name,
                arguments_json,
                cancellation,
                reply,
            } => {
                let result = runtime.block_on(execute_tool(
                    &servers,
                    &routes,
                    &name,
                    &arguments_json,
                    cancellation,
                ));
                let _ = reply.send(result);
            }
            Command::Shutdown => {
                for (_, mut server) in servers.drain() {
                    let _ = runtime.block_on(server.client.close_with_timeout(SHUTDOWN_TIMEOUT));
                }
                if let Ok(mut routes) = routes.write() {
                    routes.clear();
                }
                break;
            }
        }
    }
}

async fn connect_server(
    config: &McpServerConfig,
) -> Result<(Client, Vec<ToolRoute>), String> {
    let connect = async {
        match &config.transport {
            McpTransportConfig::Stdio { command, args } => {
                let mut command = CommandWrap::with_new(command, |process| {
                    process.args(args);
                });
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
    let tools = match tokio::time::timeout(CONNECT_TIMEOUT, client.list_all_tools()).await {
        Ok(Ok(tools)) => tools,
        Ok(Err(_)) => {
            let _ = client.close_with_timeout(SHUTDOWN_TIMEOUT).await;
            return Err("MCP 工具发现失败".to_owned());
        }
        Err(_) => {
            let _ = client.close_with_timeout(SHUTDOWN_TIMEOUT).await;
            return Err("MCP 工具发现超时".to_owned());
        }
    };
    let routes = build_routes(config, tools)?;
    Ok((client, routes))
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
            let name = namespaced_tool_name(&config.id, &remote_name);
            if !names.insert(name.clone()) {
                return Err("MCP 工具名称发生冲突".to_owned());
            }
            let parameters_json = Value::Object((*tool.input_schema).clone()).to_string();
            if parameters_json.len() > MAX_SCHEMA_BYTES {
                return Err("MCP 工具 Schema 超过安全上限".to_owned());
            }
            let description = tool
                .description
                .map(|description| description.into_owned())
                .or(tool.title)
                .unwrap_or_else(|| format!("MCP tool from {}", config.display_name));
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
    servers: &HashMap<String, ConnectedServer>,
    routes: &RwLock<HashMap<String, ToolRoute>>,
    name: &str,
    arguments_json: &str,
    cancellation: Arc<AtomicBool>,
) -> Result<String, ToolExecutionError> {
    let route = routes
        .read()
        .ok()
        .and_then(|routes| routes.get(name).cloned())
        .ok_or_else(|| ToolExecutionError::new("请求的 MCP 工具不可用"))?;
    let server = servers
        .get(&route.server_id)
        .ok_or_else(|| ToolExecutionError::new("MCP Server 未连接"))?;
    let arguments = serde_json::from_str::<Value>(arguments_json)
        .ok()
        .and_then(Value::as_object)
        .cloned()
        .ok_or_else(|| ToolExecutionError::new("MCP 工具参数不是 JSON object"))?;
    let call = server
        .client
        .call_tool(CallToolRequestParams::new(route.remote_name).with_arguments(arguments));
    tokio::pin!(call);
    tokio::select! {
        result = &mut call => {
            let result = result.map_err(|_| ToolExecutionError::new("MCP 工具调用失败"))?;
            serde_json::to_string(&result)
                .map_err(|_| ToolExecutionError::new("MCP 工具结果无法编码"))
        }
        _ = wait_for_cancellation(cancellation) => {
            Err(ToolExecutionError::new("MCP 工具调用已取消"))
        }
        _ = tokio::time::sleep(CALL_TIMEOUT) => {
            Err(ToolExecutionError::new("MCP 工具调用超时"))
        }
    }
}

async fn wait_for_cancellation(cancellation: Arc<AtomicBool>) {
    while !cancellation.load(Ordering::Acquire) {
        tokio::time::sleep(CANCELLATION_POLL).await;
    }
}

fn disconnect_server(
    runtime: &tokio::runtime::Runtime,
    server_id: &str,
    servers: &mut HashMap<String, ConnectedServer>,
    routes: &RwLock<HashMap<String, ToolRoute>>,
) {
    remove_routes(routes, server_id);
    if let Some(mut server) = servers.remove(server_id) {
        let _ = runtime.block_on(server.client.close_with_timeout(SHUTDOWN_TIMEOUT));
    }
}

fn replace_routes(
    routes: &RwLock<HashMap<String, ToolRoute>>,
    server_id: &str,
    replacements: Vec<ToolRoute>,
) {
    let Ok(mut routes) = routes.write() else {
        return;
    };
    routes.retain(|_, route| route.server_id != server_id);
    for route in replacements {
        routes.insert(route.definition.name.clone(), route);
    }
}

fn remove_routes(routes: &RwLock<HashMap<String, ToolRoute>>, server_id: &str) {
    if let Ok(mut routes) = routes.write() {
        routes.retain(|_, route| route.server_id != server_id);
    }
}

fn emit(events: &EventSender<McpEvent>, event: McpEvent) {
    let _ = events.send_blocking(event);
}

fn namespaced_tool_name(server_id: &str, tool_name: &str) -> String {
    let server = sanitize_name(server_id, 16);
    let tool = sanitize_name(tool_name, 24);
    let mut hasher = DefaultHasher::new();
    server_id.hash(&mut hasher);
    tool_name.hash(&mut hasher);
    format!("mcp_{server}_{tool}_{:016x}", hasher.finish())
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

    #[test]
    fn namespaced_tool_names_are_bounded_valid_and_collision_resistant() {
        let first = namespaced_tool_name("server alpha", "search/files");
        let second = namespaced_tool_name("server alpha", "search_files");
        assert_ne!(first, second);
        assert!(first.len() <= 64);
        assert!(first
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-')));
    }

    #[test]
    fn storage_records_convert_to_typed_configs() {
        let record = McpServerRecord {
            id: "server".to_owned(),
            display_name: "Server".to_owned(),
            transport: McpTransport::Stdio,
            config_json: r#"{"command":"server.exe","args":["--stdio"]}"#.to_owned(),
            enabled: true,
            capabilities_json: None,
            schema_hash: None,
            last_error: None,
            created_at: 1,
            updated_at: 1,
        };
        assert_eq!(
            McpServerConfig::try_from(&record).expect("typed config").transport,
            McpTransportConfig::Stdio {
                command: "server.exe".to_owned(),
                args: vec!["--stdio".to_owned()],
            }
        );
    }

    #[test]
    fn idle_runtime_starts_and_stops_without_external_processes() {
        let runtime = McpRuntime::start().expect("start runtime");
        assert!(runtime.handle().tool_definitions().is_empty());
        drop(runtime);
    }
}
