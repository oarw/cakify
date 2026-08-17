# Cakify 技术架构

> 状态：产品架构基线 v1
> 日期：2026-08-17
> 目标平台：Windows 10 22H2 / Windows 11 x64；ARM64 在 x64 稳定后加入

## 1. 架构决定

Cakify 使用一个原生桌面进程，不沿用 benchmark 的 Rust HTTP sidecar：

~~~text
┌────────────────────────── cakify.exe ──────────────────────────┐
│                                                                │
│  GPUI 主线程                                                   │
│  ├─ Window / Focus / Input / IME                               │
│  ├─ AppViewModel（只保存呈现状态）                              │
│  └─ Virtual Message List / Markdown Blocks                     │
│              │ bounded AppCommand                              │
│              ▼                                                 │
│  Core service                                                  │
│  ├─ Conversation service     ├─ Provider adapters              │
│  ├─ Agent reducer/effects    ├─ MCP clients                    │
│  ├─ Tool permission engine   └─ Cancellation / retry           │
│       │                 │                    │                  │
│       ▼                 ▼                    ▼                  │
│  SQLite actor       SecretStore         Tool process runner    │
│  (one owner)        (CredMan/DPAPI)      (Windows Job Object)  │
│                                                                │
└────────────────────────────────────────────────────────────────┘
                   │
                   └─ 用户显式启用的 MCP/工具子进程
~~~

核心原则：

- UI 只负责交互和呈现，不直接访问 SQLite、网络、Credential Manager 或子进程。
- Core 是 UI 框架无关的 Rust 库，可用 fake provider 做确定性测试。
- 默认没有 Node、Python、WebView 或后台 sidecar。
- 所有跨线程通信均为有界、可取消的强类型消息；不让 token 流无限挤占 UI 队列。
- 持久状态、运行时状态和渲染状态分开建模，避免把 GPUI Entity 当数据库。

## 2. 产品 workspace

M0 建议创建以下结构：

~~~text
apps/
  cakify-desktop/             # GPUI composition root 与 Windows 可执行文件
crates/
  cakify-core/                # command/event、用例、会话与设置服务
  cakify-storage/             # SQLite schema、migration、repositories、backup
  cakify-provider/            # Provider trait、OpenAI-compatible adapter
  cakify-agent/               # reducer、tool registry、permission policy
  cakify-mcp/                 # rmcp transport、capability 与 server lifecycle
  cakify-platform-windows/    # CredMan、DPAPI、Job Object、known folders
  cakify-test-support/        # fake provider、fixture、deterministic clock/id
docs/
  ...
~~~

先不拆出 `domain`、`ui-components`、`telemetry` 等小 crate。只有当依赖方向或编译边界真正需要时再拆，避免空抽象。

依赖方向固定为：

- `cakify-desktop -> core/provider/agent/mcp/platform-windows`
- `provider/agent/mcp -> core` 中的领域类型与端口
- `storage -> core` 中的存储 trait
- `platform-windows -> core` 中的 SecretStore/ProcessSupervisor trait
- `core` 不依赖 GPUI、Win32、SQLite 或具体 HTTP 客户端

循环依赖出现时优先移动接口到拥有业务语义的一侧，不新建“common”垃圾桶。

## 3. 线程与异步模型

### GPUI 主线程

主线程只执行：

- 输入、焦点、窗口和 accessibility event。
- 轻量 reducer，把 `AppEvent` 合并到 view model。
- 虚拟列表测量与可见行渲染。
- 已解析 Markdown block 的绘制。

禁止在 UI 线程执行 SQLite、JSON Schema 验证、大段 Markdown parse、文件 hash、网络等待或进程 wait。

### Core service

Core 在独立服务线程运行 Tokio runtime。M0 先使用 current-thread runtime；只有并发压测证明需要时才增加 worker 数。网络、MCP 和超时在此调度，CPU/阻塞任务显式进入专用 worker。

接口形态：

~~~rust
pub struct CoreHandle {
    // 内部持有 bounded sender；不暴露 runtime。
}

pub enum AppCommand {
    OpenConversation { id: ConversationId },
    SendMessage { conversation_id: ConversationId, draft: DraftMessage },
    CancelRun { run_id: RunId },
    DecideTool { call_id: ToolCallId, decision: ToolDecision },
    SaveProvider { draft: ProviderDraft, secret: Option<SecretInput> },
}

pub enum AppEvent {
    ConversationLoaded { snapshot: ConversationSnapshot },
    RunChanged { run_id: RunId, revision: u64, change: RunChange },
    ToolApprovalRequested { request: ToolApproval },
    ProviderChanged { provider: ProviderSummary },
    OperationFailed { request_id: RequestId, error: UserFacingError },
}
~~~

具体 channel crate 在 M0 spike 后确定，但必须满足：

- command capacity 初始为 256，event capacity 初始为 1024。
- 队列满时返回可观察错误或合并低价值事件，绝不无限增长。
- 文本 delta 在 core 侧按 16–33 ms 或最小字节阈值合并。
- 每个 run 有 `CancellationToken`；取消对 provider、MCP、工具进程与数据库收尾传播。
- event 带单调 revision，UI 忽略过期更新。

### SQLite actor

单独线程独占 `rusqlite::Connection`。所有写操作通过 typed request 进入，读取可在验证后增加只读连接池；MVP 先保持单 owner，减少锁与迁移复杂度。

写入策略：

- 用户消息先事务持久化，再开始 provider request。
- 流式 assistant 文本按约 250 ms 或 4 KiB 批量 checkpoint。
- 完成、失败、取消都以事务写入最终 run 状态。
- 启动恢复把遗留 `running/tool_running` 标记为 `interrupted`，保留已收到文本。

## 4. 领域模型

核心实体：

- `ProviderProfile`：provider kind、endpoint、display name、credential reference、默认模型和能力缓存。
- `Conversation`：标题、provider/model snapshot、system instruction、归档状态。
- `Message`：role、顺序、父消息、创建时间和编辑来源。
- `MessagePart`：text、reasoning summary、image、file、tool call、tool result、citation、error。
- `Run`：一次模型/Agent 执行，保存开始/结束、usage、finish reason、错误分类和取消来源。
- `ToolCall`：tool identity、参数、风险摘要、审批、状态、截断后的输出与耗时。
- `McpServer`：transport 配置、启用状态、capability snapshot 和 secret references。

所有外部标识使用 UUIDv7；SQLite 初期保存 canonical text，便于迁移与排障。时间统一保存 UTC Unix milliseconds，UI 层负责本地化。

消息不能只存一段 Markdown 字符串。`MessagePart` 允许稳定表达流式文本、附件和工具时间线，也避免未来用正则从模型文本反推结构。

## 5. Provider 边界

首个真实 adapter 是 OpenAI-compatible，随后按实际需求加入 Anthropic 与 Gemini 原生协议。兼容端点不假定完全兼容：能力由 profile 与探测结果显式记录。

~~~rust
pub trait ChatProvider: Send + Sync {
    fn capabilities(&self) -> ProviderCapabilities;
    async fn models(&self, ctx: RequestContext) -> Result<Vec<ModelInfo>, ProviderError>;
    async fn stream(
        &self,
        request: NormalizedChatRequest,
        sink: StreamSink,
        cancel: CancellationToken,
    ) -> Result<CompletionSummary, ProviderError>;
}
~~~

统一事件至少覆盖：text delta、tool-call delta、usage、finish、provider warning、retry-after 与错误。Provider 原始 payload 默认不落盘；调试模式也必须脱敏并由用户显式开启。

错误分类至少包括：

- 配置/密钥缺失
- 认证/权限
- 限流与可重试上游错误
- 网络/DNS/TLS
- 请求不兼容或上下文过长
- 内容策略
- 协议解析错误
- 用户取消

重试只针对幂等、未产生工具副作用的阶段。看到部分 assistant 文本后，不自动重新提交整个请求；UI 提供显式继续/重试。

## 6. 轻量 Agent

Agent 采用“纯 reducer + effects”的小循环，而不是绑定某个 Node/Python Agent runtime：

~~~text
Idle
  -> Preparing
  -> Requesting
  -> Streaming
  -> ToolProposed
  -> AwaitingApproval
  -> ToolRunning
  -> Requesting (继续模型回合)
  -> Completed | Failed | Cancelled | Interrupted
~~~

每次状态变化都生成领域事件并可持久化。Effect runner 只执行 reducer 明确产生的 effect；UI 不能直接调用工具。

工具注册表提供稳定命名、JSON Schema、风险等级、超时、输出上限与权限摘要。首版内建工具保持很小：

- 只读文件读取/目录列表，且必须由用户选择工作目录。
- 可选 HTTP fetch，显示目标域名与将发送的上下文。
- 不在 MVP 默认启用任意 shell、删除文件或写入系统目录。
- MCP 工具与内建工具使用同一审批和审计模型。

权限默认 `confirm`。决定支持 deny once、allow once、按具体工具持久允许、按服务器禁用；路径/命令模式的“总是允许”放到后续安全里程碑。

## 7. MCP

使用官方 Rust SDK `rmcp`，首版实现：

- stdio server。
- Streamable HTTP server。
- initialize/capability/version negotiation。
- tools/list、tools/call、progress、cancel 和结构化错误。
- server 启用/禁用、重连、健康状态和最近错误。
- 每个会话选择启用哪些 server，不把全部工具无条件暴露给模型。

stdio server 由 `cakify-platform-windows` 启动并立即加入 Job Object。关闭会话、取消运行或退出应用时按 graceful shutdown -> timeout -> terminate job 的顺序收尾。禁止 child breakaway，环境变量使用 allowlist；秘密通过短生命周期环境变量或 stdin 注入，绝不写配置明文。

暂不实现旧式双端点 HTTP+SSE、MCP marketplace、自动安装任意包和未经确认的 OAuth 浏览器回调。

## 8. GPUI UI

### 第一条纵向切片

一个可用窗口包含：

- 左侧可折叠会话栏：新建、搜索入口、最近会话、重命名、归档/删除。
- 顶部紧凑工具栏：当前 Provider/模型、会话标题、更多菜单。
- 中央虚拟消息列表：user/assistant/system/tool block、Markdown、代码复制、错误和重试。
- 底部真实多行 composer：中文 IME、selection、clipboard、拖放附件、发送/停止。
- 设置页：Provider、模型、外观、MCP、数据与隐私。

视觉方向学习 ChatGPT 的内容节奏、Cherry Studio 的多 Provider 管理、RikkaHub 的轻量会话功能和 Zed 的工具时间线；不复制品牌样式或受限源码。界面保持安静、密集、工作导向，按钮优先使用 Lucide 图标并提供 tooltip。

### 输入与 IME

不能把 benchmark 的 placeholder composer 沿用到产品。输入实现必须覆盖：

- `EntityInputHandler` / `ElementInputHandler`。
- UTF-8 与 Windows UTF-16 offset 转换。
- marked/composition range、候选窗定位和 composition commit/cancel。
- grapheme-aware cursor、selection、undo/redo、剪贴板。
- Enter 发送、Shift+Enter 换行，并在 IME composition 时禁止误发送。
- 100k 字符草稿不冻结 UI。

`gpui-component` 的 textarea 可以加速实现，但必须先通过中文微软拼音、日文 IME、emoji/组合字符、DPI 和 accessibility spike；否则基于 GPUI 官方 input 示例独立实现最小 composer。

### 消息与 Markdown

- 只渲染 viewport 附近消息；行高变化必须保持 scroll anchor。
- 流式消息以 block 为单位增量 parse，解析在 UI 线程外执行。
- 代码块、高亮、表格和长 URL 必须有稳定尺寸与横向处理。
- token delta 合并后最多约每帧更新一次，不对每个 token 重建整棵线程。
- 复制可见文本、复制 Markdown、重新生成、编辑并重发、分支从数据模型层实现。

Zed 已证明 GPUI 能承载成熟对话，但其 `agent_ui` 为 GPL；Cakify 只把公开行为当验收参考。

## 9. 可观察性与错误恢复

日志采用结构化事件，默认 `info`，滚动文件有体积/数量上限。必须在格式化前完成字段级脱敏；以下内容禁止进入日志：API Key、Authorization header、Cookie、OAuth token、完整用户消息、工具 secret env、CredentialBlob。

关键指标只在本地保存，遥测默认关闭。第一阶段记录：

- startup phase timings
- UI event loop long task
- stream delta queue depth
- SQLite operation latency
- provider time-to-first-token
- cancel-to-process-exit
- current/peak process-tree memory（benchmark 构建）

崩溃恢复不自动重跑工具。重启后展示 interrupted 状态，让用户决定继续、复制结果或重试。

## 10. 依赖策略

调研时版本快照包括 `rusqlite 0.40.2`、`tokio 1.53.1`、`reqwest 0.13.4`、`rmcp 3.1.2`、`windows 0.62.2`、`zeroize 1.9.0`。这些不是未经验证的最终 pin。

M0 通过 Actions 产生首个产品 `Cargo.lock` 后才冻结版本。网络默认使用 rustls，SQLite 使用 bundled feature，Windows API 只打开所需 feature。每个新增依赖需说明为什么不能用标准库或现有依赖完成。

## 11. 架构硬门

以下任一项连续两个 milestone 无法达标，启动 Avalonia 回退评估：

- Windows 中文 IME composition/候选窗/焦点无法可靠工作。
- 核心控件无法提供最低可接受的 UI Automation/accessibility 信息。
- GPUI 上游升级频率导致锁定 revision 仍无法维护或修复高危问题。
- 真实 10k 会话、Markdown 流式渲染无法满足路线图性能门。

回退只替换 `cakify-desktop`，core/storage/provider/agent/mcp/platform traits 保持不变。这正是 UI 与 Core 边界必须在 M0 固化的原因。
