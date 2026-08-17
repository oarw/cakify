# Cakify 跨供应商交接文档

> 用途：新的 AI 模型、供应商或工程师开始前必须完整阅读。
> 最后更新：2026-08-17（Asia/Shanghai）
> 交接状态：M0 产品源码已 bootstrap，下一步先跑产品 validate Actions 闭环。

## 1. 五分钟上下文

Cakify 要做一个 Windows-first 的原生 AI Chat 客户端：启动快、常驻内存低，不依赖 Electron/WebView，不让 Node/Python 默认常驻。产品功能整体向 Cherry Studio 与 RikkaHub 的轻量聊天能力看齐，包括多 Provider/模型、完整会话、附件、提示词、工具审批、MCP、搜索和导入导出。

用户明确不要：知识库/RAG、embedding/向量库、远程操控、云同步后台、内置大模型下载、重量插件 runtime、多 Agent 自主编排。

已确定栈：

- GPUI + Rust UI
- 同进程 Rust Core
- SQLite
- Windows Credential Manager；DPAPI current-user 后备
- 官方 Rust MCP SDK `rmcp`
- Windows Job Object 管理工具/MCP 子进程

第一轮比较 GPUI、Avalonia、Flutter、Tauri 已结束。GPUI benchmark 中位 ready `113.745 ms`、idle Working Set `42.016 MiB`，进入主线；Avalonia 只作为硬门失败回退。不要重新跑四框架或同时维护多套 UI。

## 2. 必读文件

按顺序阅读：

1. `AGENTS.md`
2. `docs/PROGRESS.md`
3. 本文件
4. `docs/PRODUCT-PLAN.md`
5. `docs/ROADMAP.md` 当前 milestone
6. 涉及架构/数据时再读 `docs/ARCHITECTURE.md`、`docs/SECURITY-AND-DATA.md`
7. 依赖与许可证问题读 `docs/RESEARCH-SOURCES.md`

离线总览为 `docs/PRODUCT-PLAN.html`，不使用 `.canvas.tsx`。

## 3. 当前真实状态

- 路径：`C:\Users\admin\Desktop\code\cakify`
- 分支：`main`，跟踪 `origin/main`
- 本轮开始 HEAD：`36742654d67b276ce964ecaea1b6a5d1a2c4c58f`
- M0 产品源码提交：`07643ab45f1eaabfa6e44d5a57116496ad1c25d2`
- Remote：`https://github.com/oarw/cakify.git`
- Visibility：`PRIVATE`
- 根产品 Cargo workspace 已建立，首批成员为 desktop、core、platform-windows。
- GPUI 空窗口和 fake Core bridge 已写入源码，但没有在本机或 Actions 编译。
- 旧 benchmark 完整归档在 `archi/framework-benchmark-2026-08/`。
- 根 `.github/workflows/validate.yml` 只有 `workflow_dispatch`；push 不会自动运行。
- 最近实际成功 run：Validate `32017467536`、Benchmark `32017470781`，都在 commit `40209896dca0009b747efc51ac885bed32b81f25`。
- 本轮创建产品源码、ADR 和手动 workflow；未运行 Actions、未切换 visibility、未本地编译/测试。
- 首个产品 `Cargo.lock`、release EXE 和产品依赖树尚不存在；不能把 M0 源码写成已通过。
- 仓库没有 LICENSE。

接手后先执行只读检查：`git status --short --branch`、`git rev-parse HEAD`、`git remote -v`、`gh repo view ... --json visibility,isPrivate`、`gh run list`。实际状态优先于本文。

## 4. 不可违反的执行约束

- 始终使用简体中文。
- 本机原则上只编辑源码；不安装大批环境，不执行项目编译、测试、benchmark、打包或发布。
- 完成源码/文档后自动 commit/push；不要等待用户再提醒推送。
- 2026 年 8 月私库 Actions 分钟已耗尽，PRIVATE 时不得运行 workflow。
- 本月每次 Actions 都必须：安全复核 -> 获得本次明确授权 -> public -> 运行/核对 -> 确认无 queued/in_progress -> private。
- 不自动修改仓库可见性，旧授权不能复用。
- 不提交/打印真实 API Key、OAuth token、签名证书、私人 endpoint 或用户数据。
- Actions 未实际运行不得写“通过”。
- 每次实质进展更新 `PROGRESS.md`；停止/换供应商更新本文件交接槽位。

## 5. 产品范围防漂移

判断一个需求是否在范围内：

- 如果是 Cherry Studio/RikkaHub 常见的本地多模型聊天、会话、提示词、附件、工具、MCP、搜索、导入导出，通常在范围内。
- 如果需要后台抓取文档、embedding、向量检索，属于明确排除的 RAG。
- 如果能控制远端/本机桌面或无人值守执行，属于明确排除的远程操控。
- 如果需要常驻 Node/Python、云服务、账号同步或自动安装第三方 runtime，先停止并请求用户扩大范围。
- 本地关键词搜索使用 SQLite FTS5，可以做；语义搜索/向量库不做。

不要把“轻量”解释为省略聊天基本功能；它指不引入与聊天无关的重量子系统。

## 6. Zed 参考与许可证边界

用户指出 Zed 已有成熟 AI 对话，这一判断正确。Zed 证明 GPUI 可以承载输入、消息队列、上下文、工具时间线、审批、MCP 与压缩。

但必须区分：

- `crates/gpui`：Apache-2.0，可用。
- `crates/agent`、`agent_ui`、`language_model`、`context_server` 等：GPL-3.0-or-later。

当前项目许可证未定，不复制或依赖 GPL AI 业务代码。可以观察公开产品行为、阅读 API 用法、形成独立设计。Apache-2.0 的 `gpui-component` 可作为交互/API 参考，但当前 revision 对 GPUI Git 依赖未固定且与产品 pin 不一致；ADR 0002 已决定 M0 不引入。

## 7. 架构要点

- 一个 `cakify.exe`；不沿用 benchmark HTTP sidecar。
- GPUI 主线程只做窗口、输入、view state、虚拟列表与绘制。
- Core service 独立线程运行 async I/O，接收 bounded `AppCommand`，发出带 revision 的 `AppEvent`。
- stream delta 按 16–33 ms/字节阈值合并，不能每 token 重绘整线程。
- SQLite actor 独占 writer connection；用户消息先落盘再请求模型。
- secret 只在 provider/core 短暂读取，UI 只看 Configured/Missing/Error。
- Agent 是 reducer + effects；工具默认 confirm，取消贯穿 Provider/MCP/process。
- MCP stdio/HTTP 共用工具权限；stdio child 进入 Job Object。
- 崩溃恢复将未完成 run 标为 interrupted，不自动重跑工具。

建议 workspace 见 `docs/ARCHITECTURE.md`。不要一开始创建无实际边界价值的 `common/utils/shared` crate。

## 8. 数据与安全要点

- 活动库：`%LOCALAPPDATA%\Cakify\data\cakify.db`。
- `rusqlite` bundled、WAL、foreign keys、migration、busy timeout。
- API Key/OAuth refresh token：Credential Manager generic credential。
- DPAPI：仅 current-user、禁止 `CRYPTPROTECT_LOCAL_MACHINE`、禁止 UI prompt。
- SQLite 只有 credential reference，不能出现明文 key/token 列。
- live backup 用 SQLite Backup API/`VACUUM INTO`，不能裸复制 `.db`。
- Markdown 不执行 script/远程 embed；代码块不会自动变成“运行”。
- 日志在格式化前脱敏，默认不记录会话正文、headers 或原始 Provider payload。
- 工具/MCP 设置 timeout、输出上限、cwd/env allowlist 和进程树清理。

## 9. M0 精确任务

源码任务 1–7 已在 `07643ab45f1eaabfa6e44d5a57116496ad1c25d2` 完成：workspace、三个首批 crate、Core 协议/fake loop、GPUI 空窗口、Windows 数据目录边界、手动 validate workflow 和 ADR 均已写入。

`gpui-component` 的 M0 结论是拒绝当前依赖：调研 commit 的 lock 指向比产品 pin 落后 88 个提交的 GPUI，workspace 又未声明 Zed revision。没有继续运行 textarea/IME/体积测试，也没有把这些项目写成通过。详情见 `docs/decisions/0002-reject-gpui-component-for-m0.md`。

接手后的顺序：

1. 静态复核仓库公开风险，并取得用户针对这一次 visibility 切换的明确授权。
2. 临时设为 public，只运行 `Validate product workspace`，记录 run URL/ID、SHA、artifact 和每个 step 结论。
3. 下载并核对 `product-validation-<run_id>`，把生成的 `Cargo.lock` 提交；编译失败则按日志修复，不得宣称 M0 通过。
4. 确认没有 queued/in_progress 后恢复 private，并再次复核可见性。
5. 核对 release EXE 的窗口启动/退出和默认进程树；物理机 IME/无障碍保持独立人工门。
6. M0 验证完成后进入 M1，不要提前接真实 Provider。

## 10. 后续顺序

- M1：SQLite + Credential Manager/DPAPI。
- M2：真实 GPUI composer/IME、虚拟消息、fake stream。
- M3：OpenAI-compatible、附件、编辑/分支、导出。
- M4：Agent/tool approval/Job Object。
- M5：MCP。
- M6：轻量产品完成度，包括 FTS5、提示词、队列、压缩。
- M7：portable、SBOM、签名/安装器和发布硬化。

不能为了 UI 展示速度跳过 M1 的 secret/data 边界，也不能为了“Agent”先做任意 shell。

## 11. 性能门

当前目标而非已验证事实：

- 冷启动 P50 <= 400 ms、P95 <= 800 ms。
- 默认 idle 整树 Working Set <= 80 MiB。
- 默认子进程数 0。
- 10,000 消息真实虚拟化。
- 输入主线程无稳定 > 50 ms 阻塞。
- 工具取消到进程树退出 P95 <= 2 s。

Benchmark 壳的 `113.745 ms / 42.016 MiB` 只是基线，不代表产品已达门。

## 12. Actions 记录

- Validate：<https://github.com/oarw/cakify/actions/runs/32017467536>，success，artifact `cargo-lock-32017467536`。
- Benchmark：<https://github.com/oarw/cakify/actions/runs/32017470781>，success，artifacts `benchmark-{gpui,avalonia,flutter,tauri}-32017470781`。
- 两者 commit：`40209896dca0009b747efc51ac885bed32b81f25`。
- 历史运行完成后已确认无 queued/in_progress，并恢复 PRIVATE。
- 本轮无新 run；不得把文档静态检查写成 Actions 通过。

## 13. 交接槽位

- 当前分支：`main`
- 本轮开始 HEAD：`36742654d67b276ce964ecaea1b6a5d1a2c4c58f`
- M0 产品源码提交：`07643ab45f1eaabfa6e44d5a57116496ad1c25d2`
- Remote：`https://github.com/oarw/cakify.git`
- Visibility：`PRIVATE`
- 当前 milestone：M0 源码已 bootstrap，等待产品 validate Actions 与首个 `Cargo.lock`
- 当前正在做：GPUI 空窗口、Core bridge、Windows 数据目录边界和手动 workflow 已写入；编译状态未知
- 最近成功 Actions：Validate `32017467536`；Benchmark `32017470781`
- 本轮 Actions：未运行
- 精确下一动作：按第 9 节取得新的 visibility 授权并运行 `Validate product workspace`
- 需要用户决定：未来每次本月 Actions visibility 授权；项目许可证；M7 签名/发行渠道
- 已知风险：M0 尚未编译、无 `Cargo.lock`、GPUI pre-1.0、直接 UI 组件工作量、真实 IME/accessibility、产品性能未跑
- 禁止误操作：不要重跑四候选；不要恢复归档 workflow；不要引入当前 `gpui-component`；不要复制 Zed GPL Agent UI；不要开始 RAG/远控

交接时用实际 `git rev-parse HEAD` 更新或解释 HEAD；不要让文档中的工作前基线冒充最新提交。
