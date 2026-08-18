# Cakify 产品总计划

> 版本：v1
> 日期：2026-08-18（Asia/Shanghai）
> 决策状态：GPUI 产品路线已确认，M0 已闭合，当前进入 M1 数据与秘密基础

## 1. 一句话定义

Cakify 是一个 Windows-first、原生渲染、启动快、常驻内存低的多 Provider AI Chat 客户端：功能完整度向 Cherry Studio 与 RikkaHub 的轻量聊天部分看齐，工具体验吸收 Zed 与 Pi 风格，但不做知识库/RAG、远程操控、云同步或重量级常驻运行时。

## 2. 已确定的产品与技术决定

- UI：GPUI + Rust，原生 GPU 渲染，不使用 WebView2。
- Core：Rust，同进程运行；Provider、会话、Agent、MCP 都通过强类型边界组织。
- 数据：SQLite + `rusqlite` bundled，单 storage actor、WAL、版本化 migration。
- 密钥：Windows Credential Manager 为主，DPAPI current-user 为结构化 secret 后备。
- 网络：Rust async HTTP，OpenAI-compatible first；真实 adapter 与 UI 解耦。
- 工具：小状态机、显式事件、默认审批、全链路取消。
- MCP：官方 Rust SDK `rmcp`，stdio + Streamable HTTP。
- 子进程：Windows Job Object 管理整棵进程树。
- 默认进程模型：只有 `cakify.exe`；不常驻 Node、Python、Bun、WebView 或 benchmark sidecar。
- 构建：本机只编辑；编译、测试、基准、打包、发布在 GitHub Actions。

第一轮 Actions benchmark 的 GPUI 原型三轮中位数为 ready `113.745 ms`、整树 idle Working Set `42.016 MiB`，明显优于其余三候选。原始工程与报告已移到 [`archi/framework-benchmark-2026-08/`](../archi/framework-benchmark-2026-08/README.md)。

## 3. 为什么 Zed 很重要，但不能直接拿代码

Zed 已经在 GPUI 上实现成熟 Agent 对话，验证了这些产品形态可行：

- 多线程会话、消息编辑与排队。
- 真正编辑器级输入、焦点、selection 与 context mention。
- 增量消息、工具时间线、审批和错误恢复。
- 模型选择、上下文压缩、MCP、线程导出与通知。

这显著降低“GPUI 能不能做成熟 AI Chat”的产品风险。但 Zed 的 `agent`、`agent_ui`、`language_model`、`context_server` 等相关 crate 是 GPL-3.0-or-later，Cakify 当前没有决定 GPL。因此：

- 使用 Apache-2.0 的 GPUI API。
- 把 Zed 公开 UI 行为与状态划分当需求参考。
- 独立实现 Cakify 的领域模型、协议、数据库和界面。
- 不复制 GPL 源码、测试、结构化片段或把 GPL crate 链入产品。

Apache-2.0 的 `gpui-component` 提供 textarea、虚拟列表、Markdown 和主题，但 M0 revision 兼容门未通过；ADR 0002 已决定当前直接使用 GPUI primitives。只有未来满足固定 revision、IME、流式 Markdown、体积和依赖审计条件时才重新评估。

## 4. 功能基线

### Alpha 必须具备

- 多 Provider/profile、模型列表、模型收藏与自定义兼容 endpoint。
- 会话 CRUD、重命名、固定/归档/删除、搜索和分支。
- 流式输出、停止、继续/重试、编辑重发、重新生成、复制。
- Markdown、代码高亮/复制、表格、引用、链接、长文本。
- system prompt、常用提示词、会话级模型和常用参数。
- 图片/文件附件、capability 检查和体积限制。
- tool/function calling 时间线、审批、拒绝、取消、失败恢复。
- MCP server CRUD、stdio/Streamable HTTP、启停与会话级绑定。
- 明暗主题、快捷键、草稿恢复、导入/导出和数据清理。

### 后续轻量增强

- 消息队列与“立即发送”。
- 长上下文 token 估算与显式压缩。
- 通知、会话内导航、最近删除。
- SQLite FTS5 关键词搜索。
- Provider 参数 profile 与更细工具权限。

### 明确排除

- 知识库、RAG、embedding、向量数据库、目录后台抓取。
- 远程操控、远程桌面、无人值守电脑代理。
- 云端账号、同步、团队空间、服务端中转。
- 内置下载/管理本地大模型。
- 插件商店自动安装 Node/Python 包。
- 多 Agent 自动编排、默认任意 shell 与无限自治循环。

这不是“先不做，之后默认补上”的普通 backlog，而是轻量产品边界。任何改变都需要重新评估内存、启动、安全和维护成本。

## 5. 系统蓝图

~~~text
用户
  │
  ▼
GPUI View / Input / Virtual List
  │ AppCommand               ▲ AppEvent（delta 合并）
  ▼                          │
Core Service ─────────────────
  ├─ Conversation / Settings
  ├─ Provider adapters ────── HTTPS APIs
  ├─ Agent reducer + effects
  ├─ Tool permission engine
  └─ MCP clients ──────────── stdio / Streamable HTTP
          │
          ├─ SQLite actor
          ├─ Windows SecretStore
          └─ Job Object process supervisor
~~~

详细模块、线程、协议和状态机见 [`ARCHITECTURE.md`](ARCHITECTURE.md)。数据位置、Credential Manager/DPAPI、工具安全和备份规则见 [`SECURITY-AND-DATA.md`](SECURITY-AND-DATA.md)。

## 6. UI/UX 方向

布局保持工作型桌面客户端的安静与效率：

- 会话栏窄而可折叠，不做营销式首页。
- 主聊天列给内容足够宽度，长代码/表格有明确横向处理。
- Provider/模型选择靠近会话标题，常用设置一到两次操作可达。
- composer 是稳定的多行工作区，不随动态内容跳动。
- 工具调用作为消息流中的结构化时间线，不用弹窗淹没用户。
- 风险审批在调用旁显示来源、参数、范围和副作用。
- 图标使用 Lucide，陌生图标有 tooltip；命令按钮不用冗余圆角文字胶囊。
- 亮色、暗色和高对比状态从 token 设计，不为每个页面手写颜色。

借鉴重点：

- ChatGPT：聊天节奏、composer、消息操作、内容可读性。
- Cherry Studio：Provider/模型配置、会话管理、提示词、MCP 与桌面工作流。
- RikkaHub：轻量多 Provider 聊天、模型与会话组织。
- Zed：GPUI 实践、消息队列、上下文、工具审批与 Agent 状态反馈。
- Pi：小而清晰的 Agent loop、工具事件和取消，不绑定其运行时。

## 7. 性能预算

预算必须按 release、整棵进程树、固定 fixture 和明确机器记录，当前数值是验收目标，不是已通过结果：

- 冷启动到可交互：P50 <= 400 ms，P95 <= 800 ms。
- 无会话活动的 idle Working Set：<= 80 MiB。
- 默认启动子进程数：0。
- 10,000 消息会话：虚拟化打开，不能实例化 10,000 个完整 view。
- 输入事件：主线程不出现可重复的 > 50 ms 阻塞。
- 流式更新：16–33 ms 合并，不按单 token 全树重绘。
- 工具取消到完整进程树退出：P95 <= 2 s。
- API Key：SQLite、日志、artifact 与导出中的明文命中数为 0。

如果真实产品因 Markdown/cache/SQLite 把内存推到 80 MiB 以上，必须用 profile 解释成本再调整门，不能只改数字。

## 8. 数据原则

- `%LOCALAPPDATA%\Cakify` 保存数据库、附件、缓存与滚动日志。
- 对话正文默认本地明文，依赖 Windows 用户 ACL；Secret 独立保护。
- SQLite 不含 `api_key`、`token` 或可还原 secret 的字段。
- 消息使用 `MessagePart` 表达 text、attachment、tool call/result、citation/error。
- 流式文本定期 checkpoint；崩溃后显示 interrupted，不自动重跑工具。
- live backup 使用 SQLite backup API/`VACUUM INTO`，不裸复制活动 `.db`。
- 本地搜索只用 FTS5 关键词，不加入 embedding/RAG。

## 9. 安全原则

- 工具默认 confirm；模型与 MCP 输出均不可信。
- Secret 只在 core/provider 中短暂展开，UI 只看到配置状态。
- 自定义 HTTP endpoint、外链、路径、附件和 JSON Schema 均结构化验证。
- MCP/工具子进程受 Job Object、超时、输出上限、cwd 与 env allowlist 约束。
- 取消、退出和崩溃恢复不能重复有副作用操作。
- 日志先脱敏再格式化；默认不写会话正文或上游原始 payload。
- 导出默认排除密钥、内部绝对路径和详细工具输出。
- 不把权限弹窗描述成 sandbox；真正运行不可信代码需要后续独立隔离方案。

## 10. 交付路线

路线图分八个 milestone：

- M0 产品 workspace、Core bridge 与 GPUI runtime smoke：已完成。
- M1 SQLite 与 Windows SecretStore：2–4 AI 日。
- M2 GPUI 完整聊天纵向切片：4–6 AI 日。
- M3 真实 Provider 与基础聊天完成度：3–5 AI 日。
- M4 轻量 Agent 与工具审批：4–6 AI 日。
- M5 MCP：4–7 AI 日。
- M6 轻量产品完成度：4–7 AI 日。
- M7 发布硬化：3–6 AI 日。

总量约 22–35 个 AI 工作日；可日常聊天的 M0–M3 约 10–15 个 AI 工作日。完整任务、验收门、回退和 Actions 设计见 [`ROADMAP.md`](ROADMAP.md)。

## 11. GitHub Actions 与本地边界

本机不安装成套 Rust/Node/Python/Flutter/Visual Studio 环境，不在本机编译、测试或跑 GUI。可以在本机做源码编辑、JSON/YAML/文本静态解析和 Git 操作。

产品计划使用手动 workflow：

- Validate：fmt、clippy、unit/integration、migration、依赖/许可证。
- Windows smoke：release exe、窗口、fake stream、Credential/DPAPI、Job Object、截图。
- Benchmark：启动、整树内存、10k、stream、cancel。
- Package：portable ZIP、SBOM、checksum；MSIX/签名后置。

2026 年 8 月私库 Actions 分钟已耗尽。源码完成后可以自动 commit/push，但 Actions 不能随 push 自动触发。用户已于 2026-08-17 持续授权本月后续的受控临时公开闭环：每次仍须安全复核；无新增实质风险时由执行者自动临时 public、只运行当前任务所需的手动 workflow、核对 runs/artifacts、确认无 queued/in_progress，再立即恢复 private。长期公开、Release/发包、无关 workflow 或新增风险不在授权内；进入 9 月先核实额度和规则。

## 12. 决策与风险

已接受：

- GPUI pre-1.0 带来 API 变化；通过完整 SHA pin、独立升级 PR 与 UI/Core 边界控制。
- SQLite `WAL + synchronous=NORMAL` 优先性能，突然断电可能丢最近事务；通过 checkpoint/crash test 验证，必要时切 FULL。
- Credential Manager/DPAPI 保护静态 secret，但不抵抗已控制同用户会话的恶意程序。

仍需验证：

- GPUI/`gpui-component` 的真实微软拼音、日文 IME、候选窗、DPI 和 UI Automation。
- 增量 Markdown + 虚拟可变高度列表的滚动锚定。
- 产品依赖加入后的真实启动/内存预算。
- Windows-hosted runner 对 Credential Manager 与可视 UI smoke 的稳定性。
- 最终许可证、签名证书与发行渠道。

Avalonia 是唯一正式回退。只有 IME、accessibility、GPUI 维护或性能硬门持续失败才启动，不再同时维护 Flutter/Tauri/C++ 版本。

## 13. 下一次直接开始的位置

M0 workspace、三个首批 crate、Core bridge、GPUI 空窗口、依赖 pin、`gpui-component` 拒绝决策、Product validate 和 Windows runtime smoke 均已闭合。最终 M0 run `32093988986` 的三轮窗口 ready 为 `118.279-145.450 ms`、空闲整树 Working Set 为 `35.477-37.121 MiB`、默认子进程 0；下一位执行者不再重复这些工作，按顺序继续：

1. 进入 M1，先实现 SQLite storage actor、initial schema 和 migration runner。
2. 实现 conversation/message/part/run repository、crash recovery 与 live backup/restore。
3. 实现 Provider profile CRUD，SQLite 只保存 opaque credential reference。
4. 再实现 Windows Credential Manager 与 DPAPI current-user SecretStore。
5. 保留真实微软拼音/日文 IME、DPI 和 UI Automation 为 M2 独立人工门，不用 M0 空壳替代。

数据与密钥边界闭合后再叠真实聊天 UI；不从“大而全首页”或 RAG 开始。
