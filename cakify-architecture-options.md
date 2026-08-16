# Cakify：Windows 高性能 AI Chat 客户端架构调研与实施建议

> 文档版本：2026-08-16  
> 目标平台：Windows 10/11，首发 x64  
> 目标产品：比 Electron 套壳更快启动、更低常驻内存，同时达到 Cherry Studio 与 RikkaHub 的基础聊天能力，并吸收 Pi Agent 的轻量 Agent 理念  
> 研发约束：本机只编辑源码；编译、测试、基准、打包和发布尽量全部交给 GitHub Actions

---

## 1. 一页结论

首选方案是：

**Rust 核心 + Tauri 2 桌面壳 + Svelte 5 UI + Windows WebView2。**

这条路线在“丰富聊天 UI、工具调用、MCP、Windows 交付、开发效率、启动速度和内存”之间最均衡。关键不是只把 Electron 换成 Tauri，而是把产品拆成可替换的桌面壳与稳定的 Rust 核心：

- UI 只负责渲染、输入与交互，不直接持有 API Key，不直接启动工具进程。
- 对话、供应商适配、Agent 循环、MCP、权限策略、SQLite 与密钥管理全部放进 Rust。
- Tauri 只是第一种宿主。如果实测无法达到性能门槛，可以保留核心，替换为 WinUI 3 或 Slint 壳。
- 第一版不捆绑本地模型、Node/Python 运行时、OCR/PDF 引擎和浏览器内核。

需要准确理解“Tauri 更轻”：

- 它不随应用重复打包 Chromium 与 Node.js，安装包通常会明显小于 Electron。
- Windows 上仍使用 WebView2。WebView2 本身是多进程模型，任务管理器里会看到浏览器进程组。
- 因此目标应是“相对 Electron 显著变轻”，而不是承诺“纯原生、几十 MB 常驻”。

建议在第一周做三个可比较的空壳基准：

1. Tauri 2 + Svelte：真实聊天布局、虚拟列表和流式 Markdown。
2. WinUI 3：同样的数据量和窗口结构。
3. Slint：只验证最关键的消息列表、输入框、中文输入法和文本选择。

最终按 CI 中采集的启动时间、进程树工作集、滚动帧率和实现成本决策，不凭框架宣传材料决策。

---

## 2. 产品边界与成功标准

### 2.1 首版必须解决的问题

- 一个入口管理多个云端模型供应商和 OpenAI-compatible 服务。
- 长对话仍然快速，流式输出不抖动，代码块与 Markdown 可读。
- 工具调用过程透明、可取消、可审批、可追踪。
- MCP 服务可以按会话启用，默认不自动扫描或启动全部服务。
- 用户数据默认本地保存；密钥不进入 WebView、日志或普通数据库字段。
- 安装后可直接使用，不要求用户再安装 Rust、Node、Python 或模型运行环境。

### 2.2 首版明确不做

- 不内置模型下载器。
- 不捆绑 Ollama、Node、Python、浏览器自动化运行时或大型解析器。
- 不做复杂多 Agent 编排。
- 不允许模型默认执行任意 Shell、任意文件写入或无限后台任务。
- 不在启动时连接所有供应商、探测全部模型或拉起全部 MCP 服务。
- 不以“兼容 Cherry/Rikka 的源码”为目标，只学习交互与功能；两者均为 AGPL-3.0，避免复制受许可约束的实现代码。

### 2.3 建议量化门槛

以下是项目目标，不是当前已验证成绩：

- Tauri 路线冷启动 P50：不高于 1.5 秒。
- 热启动 P50：不高于 0.7 秒。
- 空闲进程树工作集：不高于 180 MB。
- 10,000 条消息会话滚动：目标 60 FPS，无持续掉帧。
- UI 主线程长任务：不高于 50 ms。
- 流式首段到达后：30–60 ms 合并刷新一次，避免逐 token 重排。
- 取消工具后：不残留子进程，不继续写入会话。
- 安装包与增量更新尺寸必须在每次发布中记录并比较。

建议设置两条架构退出条件：

- 若优化后的 Tauri 空壳在统一 CI 机器上，冷启动 P50 仍超过 2 秒或空闲工作集长期超过 220 MB，则进入 WinUI 3 壳验证。
- 若 Slint 在中文输入法、文本选择、无障碍、Markdown 与长列表上需要大量自研，立即停止把它作为首发主路线。

---

## 3. 四套候选方案

评分采用“首版落地视角”的主观决策模型，满分 100；它只用于排序，必须由真实基准校正。

权重：

- 启动与内存：25
- 聊天 UI 完成度：20
- Provider、Agent 与 MCP 实现效率：20
- GitHub Actions 可验证性：15
- Windows 集成：10
- 生态、维护与许可风险：10

### 3.1 方案 A：Rust + Tauri 2 + Svelte 5

**建议评分：89/100；首选。**

组成：

- Tauri 2：窗口、菜单、托盘、IPC、安装包与更新。
- Svelte 5 + TypeScript：聊天 UI、设置、工具执行面板。
- Rust + Tokio：网络流、Agent 状态机、MCP 进程、SQLite、策略与密钥。
- WebView2：Windows Web 渲染层。
- SQLite：历史、分支、附件索引、工具执行记录与设置。

优势：

- 不随应用打包 Chromium/Node，安装包与启动成本通常优于 Electron。
- HTML/CSS 对 Markdown、代码高亮、公式、Mermaid、响应式布局和主题非常成熟。
- Tauri 2 有 Windows 安装、GitHub Actions、WebDriver 测试、安全 capability 等官方文档。
- Rust 很适合异步流、子进程生命周期、协议适配、取消与资源边界。
- 未来替换 UI 壳时，Rust domain/core 可以继续复用。

风险：

- WebView2 仍会创建多进程；不能把单个进程的内存当作应用总内存。
- 大量逐 token DOM 更新、未虚拟化列表、一次性加载 Shiki/KaTeX/Mermaid，会迅速吃掉优势。
- Tauri capability 只约束 Tauri 命令，不等于完整工具权限系统；核心层仍需独立策略引擎。
- WebView2 Runtime 缺失时必须决定安装策略。

建议交付形态：

- 默认：NSIS x64 安装包。
- WebView2：优先 Evergreen bootstrapper；企业离线环境另做离线安装包。
- 第一阶段只发 x64；稳定后再考虑 ARM64。
- 自动更新与签名在核心功能稳定后启用，避免过早复杂化。

适用判断：

- 需要尽快达到 Cherry/Rikka 的丰富 UI 与基础能力。
- 团队愿意把性能问题当作工程预算，而不是期待换壳自动解决一切。

### 3.2 方案 B：C# + WinUI 3 + Windows App SDK

**建议评分：85/100；Windows 原生备选。**

组成：

- WinUI 3 / Windows App SDK：窗口和原生控件。
- .NET：Provider、Agent、MCP 与持久化。
- CommunityToolkit 或自研组件：设置、命令、虚拟列表。
- SQLite + Windows Credential Manager / DPAPI。

优势：

- Windows 原生视觉、输入、无障碍、窗口管理和系统集成最自然。
- 主 UI 不依赖 WebView2 时，空壳启动与内存上限更有机会领先。
- Windows API、通知、托盘、快捷键、文件关联和 MSIX 生态顺手。
- C# 在 Windows GUI 开发与调试上成熟。

风险：

- 聊天消息不是普通业务表单：Markdown、代码块、复制选择、公式、Mermaid、复杂附件、增量流式渲染都需要大量组件工程。
- 若为 Markdown 区域重新嵌入 WebView2，会削弱“纯原生”的收益并增加两套 UI 协调成本。
- GitHub-hosted Windows runner 可编译测试，但 GUI E2E、安装器和可视化回归通常比 Web 技术栈难维护。
- 打包模式、Windows App SDK Runtime、MSIX/非打包部署和签名需要提前选定。

建议变体：

- 首选 WinUI 3。
- 若 WinUI 3 的发布或控件稳定性阻碍首版，可以用 WPF + Fluent 风格作为务实后备。
- 不建议在没有性能数据前做“WinUI 外壳 + 大面积 WebView”的复杂混合。

适用判断：

- Windows 原生体验与企业部署优先级极高。
- 团队能接受为聊天渲染和工具面板投入更多 UI 工程。

### 3.3 方案 C：Rust + Slint

**建议评分：74/100；性能实验路线。**

组成：

- Slint：原生编译式 UI。
- Rust 核心：与方案 A 共用 domain、provider、agent、MCP、storage。
- 自研 Markdown 分块渲染、消息虚拟化和输入体验。

优势：

- 不依赖浏览器渲染层，理论上最有机会取得最低启动时间与内存。
- Rust 单语言核心，跨平台潜力好。
- 壳层可以很薄，适合对启动和设备资源极度敏感的产品。

风险：

- 富文本聊天 UI 的隐性成本最高。
- 需要认真验证中文输入法、文本选择、剪贴板、屏幕阅读器、双向文本、高 DPI、多屏和长列表。
- Markdown、代码高亮、公式、Mermaid、HTML 片段、PDF 预览等生态远不如 Web。
- 可能得到漂亮的空壳基准，却因功能补齐导致交付周期失控。

适用判断：

- 性能是压倒一切的卖点。
- 可以牺牲首版富文本功能，或拥有长期 UI 基础设施预算。

### 3.4 方案 D：Go + Wails 2

**建议评分：83/100；Go 团队的替代选项。**

组成：

- Wails 2：Windows 壳与前后端桥接。
- Svelte/React/Vue：Web UI。
- Go：Provider、Agent、MCP、SQLite 与系统集成。
- WebView2：Windows 渲染层。

优势：

- Go 编译、并发与部署体验直接，团队上手成本可能低于 Rust。
- UI 仍使用成熟 Web 生态。
- 单个 Go 后端可实现供应商、MCP 与工具编排。
- GitHub Actions 构建相对清晰。

风险：

- 与 Tauri 一样无法绕开 WebView2 进程组。
- 截至本调研日期，Wails 主 README 将 v2 作为稳定路线、v3 仍需谨慎确认发布状态；首版不要押注不稳定主线。
- Tauri 2 在 capability、安全边界、发布文档和 Rust MCP 生态上的组合更贴合本项目。
- 若核心最终需要复杂的取消树、精细资源控制和 Rust SDK，Go 路线复用度较低。

适用判断：

- 团队 Go 熟练度显著高于 Rust。
- 愿意接受与 Tauri 类似的 WebView2 性能边界。

### 3.5 暂不推荐作为首发主路线

- Flutter Desktop：自绘 UI 跨平台优秀，但 Windows 桌面体积、文本与富聊天适配并不天然优于首选路线。
- Qt WebEngine / CEF：通常重新捆绑 Chromium，直接违背“避免 Electron 级浏览器负担”的核心目标。
- .NET MAUI：适合多端产品，但本项目当前是 Windows-first，原生桌面完成度和调试链条不如直接选 WinUI。
- 全部逻辑放前端：密钥、工具权限、进程控制和数据一致性风险太高。

---

## 4. 推荐架构：可换壳的 Rust 核心

### 4.1 仓库结构

~~~text
apps/
  desktop/                 # Tauri 壳、窗口、系统菜单、托盘
packages/
  ui/                      # Svelte UI、设计 token、组件与浏览器 mock
crates/
  domain/                  # 消息、会话、工具、模型等纯领域类型
  core/                    # 用例、状态机、事件总线、取消和任务编排
  providers/               # OpenAI/Anthropic/Gemini/兼容端点
  agent/                   # 轻 Agent 循环、上下文与错误恢复
  mcp/                     # MCP client、transport、server lifecycle
  policy/                  # 工具风险、权限、路径与网络策略
  storage/                 # SQLite、迁移、导入导出
  secrets/                 # DPAPI/Credential Manager 封装
tests/
  fixtures/                # 固定 SSE、工具调用、MCP 与错误样本
  contracts/               # provider 与 IPC 契约测试
.github/
  workflows/               # CI、E2E、基准、发布、安全
~~~

### 4.2 分层原则

~~~text
Svelte UI
   │  typed commands + event stream
   ▼
Desktop host / Tauri IPC
   │
   ▼
Application core
   ├── Conversation service
   ├── Provider router
   ├── Agent loop
   ├── Tool policy
   ├── MCP manager
   └── Storage / secrets
~~~

边界要求：

- domain 不依赖 Tauri、Svelte、数据库或具体供应商。
- core 不知道 UI 组件，只发布稳定事件。
- provider 只负责请求转换、流解析、错误归一化与能力声明。
- tool executor 不直接决定是否允许执行；先经过 policy。
- storage 用迁移控制 schema，UI 不直接写 SQLite。
- 所有长任务都接受 cancellation token，并有可观察状态。

### 4.3 核心事件模型

建议统一事件，而不是让每个供应商直接驱动 UI：

~~~text
RunStarted
MessageDelta
ReasoningDelta
UsageUpdated
ToolCallProposed
ToolApprovalRequired
ToolStarted
ToolOutputDelta
ToolCompleted
ToolFailed
RunCompleted
RunCancelled
RunFailed
~~~

UI 只消费事件并更新本地投影。这样可以：

- 统一 OpenAI、Anthropic、Gemini 和兼容端点的差异。
- 记录并重放问题会话。
- 在单元测试里用固定事件序列验证 UI。
- 避免网络层状态直接渗透到界面。

---

## 5. 聊天数据模型

### 5.1 内容块，而不是单个 Markdown 字符串

消息内容建议建模为有序块：

- Text：普通 Markdown 文本。
- Reasoning：可折叠推理摘要或供应商返回的 reasoning 内容。
- Image：图片引用和元信息。
- File：附件引用、MIME、大小与解析状态。
- ToolCall：工具名、参数、审批状态和执行 ID。
- ToolResult：结果、截断信息、错误和耗时。
- Error：可恢复错误或失败说明。
- Usage：token、缓存命中与估算成本。

不要把工具调用和附件序列化进一大段 Markdown。结构化内容更适合重试、迁移、导出、比较和后续协议升级。

### 5.2 会话必须是树

核心字段：

- conversation_id
- message_id
- parent_id
- role
- parts
- provider/model snapshot
- created_at / updated_at
- run_id
- status

编辑旧消息、重新生成和“从这里分叉”都创建新分支，不原地破坏旧链。当前 UI 只展示一条 active path。

### 5.3 附件策略

- 二进制写入应用数据目录，不存进 SQLite BLOB。
- SQLite 只保存内容哈希、相对路径、MIME、大小、原名和引用关系。
- 导入时做 MIME 与大小校验。
- 删除会话时先减少引用计数，再由后台低优先级任务清理孤儿。
- 默认不自动解析大型 PDF/Office；解析器作为延迟加载或可选插件。

---

## 6. 达到 Cherry Studio / RikkaHub 基础能力的范围

### 6.1 P0：首个可用版本

供应商与模型：

- OpenAI-compatible
- Anthropic
- Gemini
- 自定义 Base URL
- 自定义 Header、请求体覆盖和代理
- 模型能力声明：视觉、工具、reasoning、上下文长度
- 会话内快速切换模型

聊天：

- 流式输出、停止、重试、重新生成
- 编辑旧消息与分支
- 搜索、置顶、重命名、归档
- Markdown、表格、代码块、复制、公式
- 图片与文本附件
- 系统提示词和 Assistant 配置
- JSONL / Markdown 导入导出

工具与轻 Agent：

- 原生 function calling
- 内置安全工具的统一注册表
- MCP stdio
- MCP Streamable HTTP
- 工具审批、取消、超时、重试与日志
- 运行时间线：模型请求、工具参数、结果、耗时和错误

桌面体验：

- 亮/暗主题
- 键盘快捷键
- 托盘可选
- SQLite 本地历史
- Windows 安装包
- 崩溃恢复与未发送草稿

### 6.2 P1：基础产品稳定后

- 多模型并排比较
- 联网搜索
- Ollama / LM Studio 连接器，但不捆绑运行时
- 知识库 / RAG
- 可解释的长期记忆
- WebDAV 或可选云同步
- OCR、TTS、图片生成
- PDF / Office 解析
- 提示词与 Assistant 模板

### 6.3 P2：延后

- 插件市场
- 本地模型下载器
- 多 Agent / 子 Agent
- Canvas 类交互工作台
- 手机端
- 自动化工作流市场

---

## 7. UI/UX 建议

### 7.1 学习对象与取舍

学习 ChatGPT：

- 对话本身是第一视觉层级。
- 输入区固定、模型与工具状态清楚但不过度占空间。
- 停止、重试、编辑和分支动作靠近对应消息。
- 工具执行过程按时间线展开。

学习 Cherry Studio：

- 多供应商与模型管理。
- Assistant / 预设。
- 丰富附件、知识和工具入口。
- 设置项搜索和分组。

学习 RikkaHub：

- 更紧凑的移动式信息密度。
- Provider 配置灵活。
- 工具调用和多模型能力。
- 不把所有高级功能永久铺在主界面。

应避免：

- 首屏塞满 Provider、知识库、插件、MCP 和高级参数。
- 每条消息一直展示整排按钮。
- token 每到一个就触发完整 Markdown 重渲染。
- 把供应商设置与会话临时参数混在一起。
- 工具运行只显示旋转图标，不展示在做什么。

### 7.2 建议布局

~~~text
┌──────────────┬──────────────────────────────────┬───────────────┐
│ 会话 / 搜索  │ 当前会话                         │ 上下文检查器  │
│ Assistant    │ 消息树的当前路径                 │ 模型与参数    │
│              │                                  │ 工具时间线    │
│              │ 固定输入区                       │ 附件与引用    │
└──────────────┴──────────────────────────────────┴───────────────┘
~~~

- 左栏可折叠，主栏优先。
- 右侧检查器只在需要时打开。
- 小窗口隐藏检查器，但不删除功能入口。
- 工具审批直接嵌入时间线，不使用阻断整个应用的系统弹窗。
- 高级模型参数放进逐步展开的面板。

### 7.3 长对话渲染

- 消息列表必须虚拟化。
- 每条消息按内容块局部更新。
- 流式文本先以轻量纯文本/基础 Markdown 增量展示，完成后再做完整高亮。
- Shiki、KaTeX、Mermaid、PDF 与 OCR 按首次使用延迟加载。
- 代码高亮放进 worker 或低优先级任务。
- 不让整个 conversation store 的变化触发所有消息重绘。

---

## 8. 轻 Agent：学习 Pi 的理念，不复制它的全部运行形态

Pi 值得借鉴的是：

- 一个小而清晰的有状态 Agent 核心。
- 工具调用是显式事件，不是隐藏副作用。
- 流式事件贯穿模型、工具与 UI。
- 上下文组装、工具注册和交互界面彼此解耦。
- 能力通过组合与扩展获得，而不是把所有功能硬编码进核心。

不建议把整个 Pi coding-agent 连同 Node/Bun 环境作为桌面 sidecar 捆绑进首版。这样会重新引入运行时体积、进程管理、升级与安全问题。

建议把理念映射到 Rust：

~~~text
用户输入
  → 构造上下文
  → 发起 Provider 流
  → 接收文本或 ToolCall
  → JSON Schema 校验
  → 风险评估
  → 必要时请求用户批准
  → 执行内置工具或 MCP
  → 写入 ToolResult
  → 继续模型循环
  → 完成 / 取消 / 超时 / 失败
~~~

### 8.1 明确的运行状态

~~~text
Idle
Preparing
CallingModel
AwaitingApproval
RunningTool
Continuing
Completed
Cancelled
Failed
~~~

每次运行都有 run_id；每个工具调用有 call_id；工具输出与最终回答都能追溯到对应 ID。

### 8.2 默认关闭的能力

- 任意 Shell。
- 任意路径文件写入。
- 后台无限循环。
- 子 Agent。
- 自动连接所有 MCP。
- 启动时扫描本机服务。
- 未经确认向陌生域名上传附件。

### 8.3 内置工具最小集合

首版可以只提供：

- read_attachment：读取用户已附加的文件。
- search_conversation：搜索本地会话。
- open_url：交给系统浏览器打开，并明确显示目标域名。
- copy_to_clipboard：写剪贴板，可低风险自动允许。

文件写入、Shell、浏览器控制等高风险能力在策略模型成熟后再加。

---

## 9. MCP 与工具系统

### 9.1 MCP transport

首版支持：

- stdio：由应用启动并管理子进程。
- Streamable HTTP：连接远程或本机服务。

Rust 可优先采用官方 MCP Rust SDK / rmcp，并在自己的 adapter 后面封装，避免协议 SDK 变化扩散到 core。

### 9.2 MCP 服务配置

建议字段：

- id / display_name
- transport
- command / args / cwd
- env 引用，不把明文秘密写进普通配置
- url / headers 引用
- enabled
- auto_start，默认 false
- trusted，默认 false
- tool allowlist / denylist
- startup timeout / call timeout

### 9.3 工具风险等级

- 低风险：只读本地会话、读取用户主动附加内容。
- 中风险：访问已显示域名、写剪贴板、创建应用沙箱内文件。
- 高风险：运行命令、写任意路径、发送敏感文件、控制浏览器、修改 Git 仓库。

审批界面至少显示：

- 工具来源：内置或具体 MCP 服务。
- 工具名与说明。
- 规范化参数。
- 涉及的路径、命令、域名。
- 本次允许、会话内允许或拒绝。

“永久允许”不应在首版提供，或必须进入独立设置页管理。

### 9.4 进程安全

- 启动子进程时记录 process tree。
- 取消或超时时终止整棵子进程树。
- 限制 stdout/stderr 单次与累计大小。
- 工具输出超限后落盘并只向模型提供摘要。
- 禁止把用户 API Key 自动继承给 MCP 子进程。
- 环境变量采用显式 allowlist。

---

## 10. Provider 适配层

统一接口应覆盖：

~~~text
capabilities()
list_models()
stream_chat(request, cancellation)
count_or_estimate_tokens()
normalize_error()
~~~

统一请求包含：

- model
- system / developer instructions
- message parts
- tools 与 JSON Schema
- temperature / top_p / max tokens
- reasoning 参数
- stop
- provider-specific extension map

统一错误类别：

- authentication
- permission
- rate_limit
- context_too_long
- invalid_request
- unsupported_capability
- network
- timeout
- cancelled
- provider_internal

不要把所有供应商强行压成最低公分母。通用字段进入核心，独有能力放在带命名空间的 extension，并由 capability 决定 UI 是否显示。

---

## 11. 安全与隐私

### 11.1 密钥

- API Key 不进入 WebView，不进 localStorage，不进前端日志。
- Rust 后端使用 Windows DPAPI 或 Credential Manager 保存。
- SQLite 只保存 secret reference。
- 导出默认不包含密钥。
- 崩溃日志和诊断包做字段级脱敏。

### 11.2 网络

- 所有模型和 MCP HTTP 请求由后端发起。
- 自定义代理、证书和 Base URL 需要单独建模。
- 工具审批显示最终解析后的主机名。
- 附件上传前显示供应商与目标域。
- 可选提供“仅允许已配置 Provider 域名”的严格模式。

### 11.3 Tauri 边界

- 每个窗口只获得所需 capability。
- 不向聊天内容开放通用文件系统或 Shell 插件。
- IPC 命令采用明确参数类型和长度限制。
- 前端传来的路径、URL、工具参数一律视为不可信。
- capability 之外，Rust policy 仍进行业务级授权。

### 11.4 本地数据

- 数据库开启 WAL，并设计可恢复迁移。
- 敏感工具输出允许单独标记“不写历史”。
- 一键导出与一键清空必须可用。
- 删除应覆盖数据库引用、附件和缓存索引。

---

## 12. 性能工程

### 12.1 启动路径

启动时只做：

- 创建窗口。
- 打开数据库并执行廉价 schema 检查。
- 加载当前主题、窗口状态和最近会话摘要。
- 渲染骨架与输入区。

延迟执行：

- 模型列表刷新。
- MCP 连接。
- 全文索引维护。
- 更新检查。
- Markdown 重型插件。
- 附件解析器。

### 12.2 内存

- 统计整个进程树，不只统计主进程。
- 只保留当前窗口附近的消息视图模型。
- 附件以文件和流处理，不复制成多份 Base64。
- 限制工具输出、日志和 SSE 原始帧的缓存。
- Provider 响应完成后释放临时缓冲。
- 一个应用只创建一个必要的 WebView2 environment。

### 12.3 流式渲染

- 网络层继续逐片接收。
- core 将 delta 合并为 30–60 ms 的 UI 批次。
- UI 在动画帧内更新当前内容块。
- 完成前不重复执行完整语法高亮。
- 用户滚离底部后停止强制自动滚动。

### 12.4 基准方法

每个框架使用同一组 fixture：

- 空会话。
- 200 条普通 Markdown。
- 10,000 条短消息。
- 50 个大型代码块。
- 10 分钟流式响应。
- 20 次工具调用，其中含取消、超时和大输出。

记录：

- 进程启动到窗口可交互。
- 进程树峰值与空闲工作集。
- 首次打开大对话耗时。
- 滚动帧时间。
- 流式期间 UI 长任务。
- 取消到所有子进程退出的时间。
- 安装包和解压后体积。

基准必须上传原始 JSON artifact，README 中只展示趋势，不只展示最好的一次。

---

## 13. GitHub Actions：本机零构建方案

### 13.1 现实边界

可以全部放进 Actions：

- Rust/TypeScript 编译
- 单元、集成、契约测试
- Windows WebDriver E2E
- 启动与内存基准
- 安装包构建
- 签名
- Release 与更新清单

不能完全替代：

- 日常交互式 Windows UI 调试。
- 主观评估中文输入法、文本选择、屏幕阅读器、多显示器。
- 真实用户机器上的驱动、企业策略和杀毒软件兼容性。

折中方式：

- UI 在浏览器 mock 模式开发，fixture 模拟后端事件。
- 每个 PR 由 Windows runner 生成安装包、截图、测试视频和基准 JSON。
- 里程碑版本下载 artifact，在一台干净 Windows 虚拟机或真实机器做人工验收。
- 本机不安装 Rust/Node 也可以编辑，但排错反馈会比本机构建慢。

### 13.2 工作流拆分

建议创建：

~~~text
.github/workflows/
  ci.yml
  e2e.yml
  bench.yml
  release.yml
  security.yml
~~~

ci.yml：

- pull_request、push 到 main。
- Ubuntu：cargo fmt、clippy、core/provider/policy 单测、前端 lint/typecheck。
- Windows 2025：Tauri compile、Windows-only 单测、安装包 smoke。
- 使用依赖缓存，但 cache key 必须包含 lockfile。

e2e.yml：

- Windows 2025。
- 安装匹配的 WebView2/Edge Driver。
- 启动 fixture server，不访问真实付费模型。
- 测试新建会话、流式、停止、重试、工具审批、取消和恢复。
- 失败时上传截图、视频、日志和数据库副本。

bench.yml：

- workflow_dispatch 和定时运行。
- 固定 runner 镜像与 fixture。
- 预热后测冷启动/热启动多次。
- PowerShell 收集进程树 working set。
- 上传 JSON；与基线比较时先采用告警，数据稳定后再设硬门槛。

release.yml：

- tag v* 触发。
- 首发只构建 x64 NSIS。
- 生成校验和、SBOM、安装包和更新清单。
- 签名秘密只进入 release environment。
- 发布前验证安装、启动、卸载和升级。

security.yml：

- cargo audit / cargo deny
- pnpm audit 或等价依赖审计
- CodeQL
- secret scanning / gitleaks
- Dependabot 或 Renovate
- 许可证清单与 SBOM

### 13.3 Actions 权限与供应链

- workflow 顶层默认 permissions: contents: read。
- Release job 单独申请 contents: write。
- 第三方 Action 固定到完整 commit SHA，不只使用 tag。
- fork PR 不获得 secrets。
- 不在 pull_request_target 中 checkout 并运行不可信 PR 代码。
- 不把 build artifact 当作可信输入跨越高权限 workflow，除非校验来源和哈希。
- 签名使用 GitHub Environment 审批与最小可见范围。
- 构建日志禁止输出请求头、环境变量和完整用户目录。

---

## 14. “先公开跑 Actions，再转私有”的安全方案

GitHub 对公开仓库的标准托管 runner 通常免费，但公开并不只是一个计费开关。公开期间：

- 完整源码与 Git 历史可被任何人下载。
- Actions 日志、artifact 的可见性需要逐项确认。
- 别人可以创建公开 fork；主仓库之后改回私有，既有公开 fork 不会因此自动消失。
- 曾经提交过的秘密即使从最新版本删除，也可能仍在历史、缓存、日志或 fork 中。
- 仓库改回私有后，公开下载、Pages 与社区协作方式会变化。

### 14.1 首选做法

如果源码最终可以开源：

1. 从第一天就按“永久公开”标准清理仓库。
2. 不提交真实 Key、签名证书、私人端点、客户数据和内部 URL。
3. 公开期间只跑不需要秘密的 CI、E2E fixture 与 unsigned build。
4. 签名发布放到私有仓库、受保护 environment 或后续有额度时执行。
5. 即使之后改私有，也假设公开过的所有提交永久可获得。

### 14.2 更安全的替代

如果源码不希望永久外泄：

- 不建议反复切换主私库可见性。
- 建立经过清理的公开镜像，只包含允许开源的核心与构建文件。
- 或建立独立公开的 cakify-releases 仓库，只承载安装包、校验和与更新清单；源码仍在私库构建，但这不能解决私库 Actions 分钟不足。
- 也可以使用自托管 Windows runner，但需要维护干净机器、安全更新与隔离，通常不适合首阶段。

### 14.3 切换公开前检查清单

- 扫描整个 Git 历史中的 secrets。
- 检查 LFS 对象、Release、artifact、Actions cache。
- 删除或替换私人 issue、讨论、wiki 和项目板内容。
- 检查分支保护与 workflow 权限。
- 所有真实凭据轮换，而不是只删除文件。
- 确认许可证与第三方素材允许公开。
- 接受“公开期间产生的 fork 无法收回”这一事实。

本调研不会自动修改仓库可见性，也不会启动计费或发布动作。

---

## 15. 分阶段实施

### 阶段 0：架构竞速，约 1 周

- 建立共享 fixture 和性能采集脚本。
- Tauri、WinUI、Slint 各做最小可比较壳。
- 验证中文输入、10,000 消息、流式输出和任务取消。
- 用 Actions 产出数据，确认首选壳。

退出条件：

- 有可重复的冷/热启动和内存数据。
- 明确 UI 富文本成本。
- 决定主路线和备用路线。

### 阶段 1：聊天核心，约 2–3 周

- domain、SQLite migration、消息树。
- OpenAI-compatible 与 Anthropic。
- 流式、停止、重试、编辑和分支。
- Markdown、代码块、图片/文本附件。
- 浏览器 mock 与 Windows 安装 artifact。

退出条件：

- 不使用真实 Key 的 fixture E2E 全绿。
- 大会话达到初步性能门槛。

### 阶段 2：轻 Agent 与 MCP，约 2–3 周

- 统一 tool registry。
- Agent 状态机和事件流。
- MCP stdio / Streamable HTTP。
- 审批、取消、超时、输出限制和进程树清理。
- 工具执行时间线。

退出条件：

- 恶意参数、超大输出、取消和崩溃均有测试。
- 默认配置下不存在任意 Shell/任意文件写入。

### 阶段 3：产品化，约 2 周

- Gemini、自定义供应商与代理。
- 搜索、归档、导入导出。
- 主题、快捷键、托盘。
- DPAPI/Credential Manager。
- 安装、升级、卸载 smoke。

### 阶段 4：发布与观察，约 1–2 周

- 签名、SBOM、Release、更新清单。
- 崩溃诊断与脱敏。
- 性能基线变成门禁。
- 小范围测试后再排 P1。

---

## 16. 最终建议

如果现在开始实现：

1. 采用 Rust + Tauri 2 + Svelte 5。
2. 先建立 domain/core/provider/policy，不先堆 UI 页面。
3. UI 只对接稳定 IPC 与事件协议。
4. 首版只做云端 Provider、SQLite、基础附件、function calling 和 MCP。
5. 不捆绑本地模型、Node/Python、OCR/PDF 引擎。
6. 从第一天建立 Windows Actions 的启动、内存和进程清理基准。
7. 保留 DesktopHost 边界；Tauri 未达门槛时换 WinUI 3，不推倒核心。
8. 把仓库公开视为不可逆的信息披露，而不是临时借用免费分钟。

一句话判断：

**Tauri 2 是最可能把产品做出来且明显轻于 Electron 的路线；WinUI 3 是性能与 Windows 原生体验的保险；Slint 是值得基准验证、但不适合直接押注完整首版的激进方案。**

---

## 17. 官方资料

### Tauri 与 WebView2

- [Tauri 2：Get Started](https://v2.tauri.app/start/)
- [Tauri：Process Model](https://v2.tauri.app/concept/process-model/)
- [Tauri：Windows Installer](https://v2.tauri.app/distribute/windows-installer/)
- [Tauri：WebDriver Testing](https://v2.tauri.app/develop/tests/webdriver/)
- [Tauri：WebDriver CI](https://v2.tauri.app/develop/tests/webdriver/ci/)
- [Tauri：GitHub Pipelines](https://v2.tauri.app/distribute/pipelines/github/)
- [Tauri：Windows Code Signing](https://v2.tauri.app/distribute/sign/windows/)
- [Tauri：Capabilities](https://v2.tauri.app/security/capabilities/)
- [Microsoft：Distribute a WebView2 app](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution)
- [Microsoft：WebView2 process model](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/process-model)
- [Microsoft：WebView2 user data folder](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/user-data-folder)

### Windows 原生与 Slint/Wails

- [Microsoft：WinUI 3](https://learn.microsoft.com/en-us/windows/apps/winui/winui3/)
- [Microsoft：Windows App SDK](https://learn.microsoft.com/en-us/windows/apps/windows-app-sdk/)
- [Microsoft：Package and deploy Windows apps](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/)
- [Slint](https://slint.dev/)
- [Slint：Desktop platforms](https://docs.slint.dev/latest/docs/slint/guide/platforms/desktop/)
- [Wails README](https://github.com/wailsapp/wails#readme)

### 参考产品与 Agent

- [Cherry Studio：Key Features](https://github.com/CherryHQ/cherry-studio#-key-features)
- [Cherry Studio：AGPL-3.0 License](https://github.com/CherryHQ/cherry-studio/blob/main/LICENSE)
- [RikkaHub：Features](https://github.com/rikkahub/rikkahub#-features)
- [RikkaHub：AGPL-3.0 License](https://github.com/rikkahub/rikkahub/blob/master/LICENSE)
- [Pi mono repository](https://github.com/badlogic/pi-mono)
- [Pi coding-agent](https://github.com/badlogic/pi-mono/tree/main/packages/coding-agent)
- [Pi agent core](https://github.com/badlogic/pi-mono/tree/main/packages/agent)

### MCP、安全与 GitHub Actions

- [Model Context Protocol specification 2026-07-28](https://modelcontextprotocol.io/specification/2026-07-28)
- [Official MCP Rust SDK](https://github.com/modelcontextprotocol/rust-sdk)
- [Microsoft：Windows DPAPI](https://learn.microsoft.com/en-us/windows/win32/api/dpapi/)
- [GitHub Actions usage limits and billing](https://docs.github.com/en/actions/learn-github-actions/usage-limits-billing-and-administration)
- [GitHub-hosted runner images](https://github.com/actions/runner-images#available-images)
- [GitHub：Changing repository visibility](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/managing-repository-settings/setting-repository-visibility)
- [GitHub Actions security hardening](https://docs.github.com/en/actions/security-for-github-actions/security-guides/security-hardening-for-github-actions)
- [GitHub Codespaces overview](https://docs.github.com/en/codespaces/overview)

---

## 18. 决策记录模板

在阶段 0 完成后，把实测结果写入下列记录：

~~~text
Decision:
Date:
Candidates:
Chosen:

Measured on:
- Runner image:
- Commit:
- Fixture version:

Results:
- Cold start P50/P95:
- Warm start P50/P95:
- Idle process-tree working set:
- 10k-message frame time:
- Installer size:
- E2E reliability:

Trade-offs accepted:
Exit conditions:
Review date:
~~~

这样后续即使替换桌面壳，也能清楚知道当时为什么做出选择。
