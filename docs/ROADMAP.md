# Cakify 产品路线图

> 日期：2026-08-17
> 估算口径：AI 实施工作日，不含 GitHub Actions 排队、临时公开授权等待和物理机人工验证时间。

## 1. 产品范围硬约束

Cakify 的整体功能向 **Cherry Studio 与 RikkaHub 的轻量 AI Chat 能力**看齐，而不是只做一个模型演示壳。

必须逐步达到：

- 多 Provider、多模型与 OpenAI-compatible endpoint。
- 会话创建、重命名、归档、删除、搜索与分支。
- 流式聊天、停止、重试、编辑重发、重新生成、复制与 Markdown/代码块。
- system prompt、常用提示词、模型参数与会话级模型切换。
- 图片/文件附件与 Provider 能力提示。
- function/tool calling、清晰时间线、逐次审批、取消与错误恢复。
- MCP 本地/远程 server 管理、启停、会话绑定与权限。
- 主题、快捷键、导入/导出、数据管理与基本更新能力。
- Zed 已验证好用的轻量能力：消息排队、上下文压缩、线程导航、工具权限摘要，按里程碑加入。

明确不做：

- 知识库、RAG、向量数据库、embedding pipeline、目录后台索引。
- 远程操控电脑、远程桌面、跨设备代理或无人值守自动化。
- 云端账号/同步、团队空间、服务端代理和遥测后台。
- 内置本地模型下载/推理管理；首版只允许用户连接已有 endpoint。
- 常驻 Node/Python/Bun、插件商店自动执行第三方安装脚本。
- 多 Agent 编排、自动 spawn 子 Agent、浏览器自动化和通用 shell 自主循环。

新增功能如果隐含后台索引、远程控制、常驻 runtime 或云服务，默认判定为超范围，必须先修改本节并由用户明确确认。

## 2. 总工期判断

- 可聊天的纵向切片：约 10–15 个 AI 工作日。
- 达到轻量 Cherry Studio/RikkaHub 基础功能的 alpha：约 22–35 个 AI 工作日。
- 加上物理机验证、签名、安装器与回归修复，日历时间预计 5–8 周。

AI 可以完成全部源码、测试、CI、文档和修复。仍需要用户参与的只有：每次 2026 年 8 月临时公开运行 Actions 的授权、真实 Provider key 的本机体验、物理 Windows IME/无障碍判断，以及未来签名证书/发行渠道决定。

## 3. M0：产品工作区与技术 spike

估算：1–2 AI 工作日。

交付：

- 创建产品 Cargo workspace 与首批 desktop/core/platform-windows crate；其余边界在首次实现时加入。
- 固定 Rust toolchain、GPUI commit 和最小 Windows feature。
- 建立 `AppCommand`/`AppEvent`、确定性 ID/fake core loop 与空壳 GPUI window。
- 创建只允许 `workflow_dispatch` 的产品 validate workflow。
- 对 `gpui-component` 先做 revision/依赖兼容门；不通过时直接形成拒绝 ADR，不把未运行的 UI 测试写成通过。
- 写首批 ADR：GPUI pin、UI component 采用/拒绝、线程模型、依赖许可。

验收门：

- Actions 能在 `windows-2025` 产生 release exe 和 `Cargo.lock`。
- 空窗口启动/退出稳定；无默认 sidecar/Node/Python/WebView。
- Core 的 fake command/event round trip 有确定性测试。
- `gpui-component` 形成明确 adopt/partial/reject 结论，不能悬而不决进入 M1。
- 依赖树没有 GPL-only Zed Agent/AI crate。

当前状态与剩余任务：

1. [x] 创建根 `rust-toolchain.toml`、`Cargo.toml` 和 workspace dependency policy。
2. [x] 创建 `apps/cakify-desktop`、`crates/cakify-core`、`crates/cakify-platform-windows` 首批骨架。
3. [x] 实现 bounded command/event bridge、revision 和 fake core loop 测试源码。
4. [x] 创建 Windows 数据目录边界；真实 known-folder FFI 与 SecretStore 在 M1 实现。
5. [x] 添加只允许 `workflow_dispatch` 的 validate workflow 和首批 ADR。
6. [x] `gpui-component` revision 兼容门失败，M0 决定直接使用 GPUI primitives。
7. [x] Product validate `32034202488` 生成 release EXE、`Cargo.lock` 和依赖树；fmt/check/tests/Clippy/release build 全部通过。
8. [x] 核对最终 artifact、锁文件一致性、依赖边界、哈希与 secret；提交锁文件并恢复仓库 PRIVATE。
9. [ ] 增加 Windows runtime smoke，核对默认进程树、窗口生命周期、Working Set 和退出无残留后关闭 M0。

## 4. M1：数据与秘密基础

估算：2–4 AI 工作日。

交付：

- SQLite storage actor、initial schema、migration runner。
- Conversation/message/part/run repository。
- Windows Credential Manager SecretStore；DPAPI round-trip adapter。
- Provider profile CRUD，SQLite 只存 credential reference。
- crash recovery、backup/restore 和 synthetic secret 测试。

验收门：

- 新建/编辑/删除 Provider 后 secret 生命周期正确。
- 数据库、日志、导出 fixture 内没有 secret plaintext。
- migration 从空库和上一 schema 均可重复验证。
- app 异常结束后 running run 变为 interrupted，不丢已 checkpoint 文本。
- live backup 恢复后 `integrity_check` 与领域计数一致。

## 5. M2：GPUI 完整聊天纵向切片

估算：4–6 AI 工作日。

交付：

- 会话栏、聊天页、模型选择、设置页与明暗主题。
- 真正多行 composer：中文 IME、selection、clipboard、undo/redo、草稿恢复。
- 虚拟消息列表与增量 Markdown/code block。
- fake provider 流式输出、停止、失败、重试、编辑重发。
- 会话 CRUD、自动标题占位策略、消息复制。

验收门：

- 微软拼音 composition 时 Enter 不误发送；候选窗位置、焦点恢复和高 DPI 正确。
- 10,000 消息打开和滚动不创建 10,000 个常驻完整 view。
- stream delta 合并，无每-token 全线程重绘。
- 参考硬件 release 构建启动 P50 <= 400 ms、P95 <= 800 ms；空闲整树 Working Set <= 80 MiB。阈值是产品目标，首次运行前不能写成已达标。
- UI 自动截图覆盖 empty/loading/streaming/error/dark；物理机 IME 仍需人工门。

## 6. M3：真实 Provider 与基础聊天完成度

估算：3–5 AI 工作日。

交付：

- OpenAI-compatible provider、模型列表与自定义 endpoint。
- 流式 text/tool-call normalization、usage、错误分类、限流反馈。
- system prompt、temperature 等被支持参数、会话级模型切换。
- 图片/文件附件 capability gate。
- 重新生成、编辑并分支、会话 JSON/Markdown 导出。
- 常用提示词的本地轻量管理，不引入知识库。

验收门：

- fake server 覆盖碎片化 SSE、UTF-8 边界、malformed event、429、断线和取消。
- 真实 Provider key 只在本机手工 smoke 使用，不进 CI。
- 看到部分输出后的错误不会静默重复请求。
- endpoint 重定向不泄露 Authorization header。
- 导出默认不含 secret、内部绝对路径和日志。

达到 M3 即形成可日常聊天的 alpha vertical slice。

## 7. M4：轻量 Agent 与工具审批

估算：4–6 AI 工作日。

交付：

- 纯 reducer Agent loop 和持久 run/tool-call 状态。
- tool registry、schema validation、超时、输出截断。
- tool timeline：proposed/waiting/running/completed/failed/cancelled。
- deny once、allow once、按工具持久规则。
- 极小内建工具集；高风险工具默认不提供。
- Windows Job Object process supervision。

验收门：

- 未批准的 tool effect 绝不执行。
- cancel 到 child/grandchild 全部退出的 P95 <= 2 s。
- 应用崩溃/重启不自动重放有副作用 tool call。
- schema/permission race、重复 approval 与迟到 output 有确定性测试。
- 工具输出超过上限时截断并明确标识，不使 UI/数据库失控。

## 8. M5：MCP

估算：4–7 AI 工作日。

交付：

- `rmcp` stdio 与 Streamable HTTP。
- server CRUD、启停、状态、最近错误、capability snapshot。
- 会话级 server/tool 选择与统一权限。
- progress/cancel、断线重连、schema hash 与授权失效。
- MCP secret references 和受限环境变量注入。

验收门：

- 与至少两个受控 fixture server 完成版本协商、tools/list/call/cancel。
- 不支持的 spec/transport 显示明确错误，不降级为不安全兼容。
- server 关闭、会话取消和 app exit 都无残留进程。
- remote server 默认 HTTPS；发送上下文前能识别目标 server。
- MCP tool 与内建 tool 使用同一审批/审计语义。

## 9. M6：轻量产品完成度

估算：4–7 AI 工作日。

交付：

- 会话本地全文搜索（SQLite FTS5），不做 embedding/RAG。
- 收藏/固定/批量归档、最近删除与数据清理。
- 常用提示词、快捷键、通知、消息排队、长上下文压缩。
- 设置导入/导出、附件管理、诊断包（脱敏）。
- Provider 模型收藏、模型参数 profile。
- 首次启动、空状态、升级 migration 与错误恢复细化。

验收门：

- 50k 消息的关键词搜索满足交互目标，后台不常驻索引线程。
- 压缩产生显式 summary part，原消息不被静默删除。
- message queue 可编辑/删除/立即发送，状态在崩溃后可解释。
- 全部设置有默认值、验证、撤销/错误反馈。
- 基础功能对照 Cherry Studio/RikkaHub 清单完成，不以知识库模块数量衡量完成度。

## 10. M7：发布硬化

估算：3–6 AI 工作日。

交付：

- portable ZIP、版本元数据、SBOM、许可证清单与校验和。
- 安装/更新方案决策；签名条件具备后加入 MSIX/安装器。
- crash recovery、日志轮转、数据迁移与卸载策略。
- 冷/热启动、内存、长列表、流式、进程清理基准。
- Windows 10/11、100/150/200% DPI、双屏、睡眠恢复与 UI Automation 检查。

验收门：

- release artifact 可在干净 Windows 用户环境启动，不依赖预装 Node/Python/WebView runtime。
- 安装、升级、卸载和保留/删除数据语义符合文档。
- artifact、SBOM、SHA-256、commit 和 workflow run 可追溯。
- 无高危依赖公告或未知许可证。
- 所有性能门在明确机器/runner 上记录原始样本，不能只报告最好值。

## 11. GitHub Actions 路线

计划恢复产品 workflow：

- `product-validate.yml`：fmt、clippy `-D warnings`、unit/integration、migration、dependency/license policy。
- `windows-smoke.yml`：release build、窗口探针、fake provider、SQLite、CredMan/DPAPI、Job Object cleanup、截图。
- `benchmark.yml`：启动、整树内存、10k 列表、streaming、cancel cleanup；只手动运行。
- `package.yml`：portable ZIP/SBOM/checksum；只手动运行。
- `release.yml`：tag、签名、release environment；到 M7 才创建。

2026 年 8 月约束仍有效：仓库 PRIVATE 时不触发 Actions。即使源码自动 commit/push，workflow 也保持 `workflow_dispatch`。用户已持续授权本月后续受控闭环；每次运行前仍做公开安全复核，无新增实质风险时自动完成 public -> 当前任务所需 workflow -> 核对 -> 无活动任务 -> private，不再逐次询问。进入 9 月先检查额度和规则，再决定是否恢复 push/PR validate。

## 12. 回退与停止条件

### GPUI -> Avalonia 回退

只有架构文档定义的 IME、accessibility、上游维护或性能硬门失败时启动。不得因为一次编译错误或文档较少同时维护两套 UI。

### 功能延期

任一功能出现以下特征则移出 alpha：

- 需要常驻重量 runtime。
- 需要云端服务或用户账户。
- 隐含 RAG/embedding/目录索引。
- 实质属于远程操控或无人值守自动化。
- 无法在 UI 中清楚表达数据去向和权限。

### 发布停止

出现 secret 泄露、工具绕过审批、取消后残留进程、迁移可能破坏数据、许可证不明或签名供应链异常时，停止发包，先修复根因。

## 13. 进度更新方式

每完成一个任务：

1. 更新 `docs/PROGRESS.md` 的当前 milestone、完成项、阻塞和精确下一步。
2. 实质架构决定写 ADR 或更新 `docs/ARCHITECTURE.md`，不只留在聊天里。
3. Actions 记录 run URL/ID、commit SHA、artifact 名和结论；未运行写“未运行”。
4. 停止或换模型前更新 `docs/HANDOFF.md`。
5. 完成的源码与文档自动 commit/push；8 月 Actions 按持续授权自动完成受控临时公开闭环，超出授权边界时再请求确认。
