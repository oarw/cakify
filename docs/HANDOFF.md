# Cakify 跨供应商交接文档

> 用途：新的 AI 模型、供应商或工程师开始前必须完整阅读。
> 最后更新：2026-08-18（Asia/Shanghai）
> 交接状态：M0 Product validate 与 Windows runtime smoke 已闭合，当前进入 M1 数据与秘密基础。

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
- M1 开始前源码基线 HEAD：`a1f10429a7f48b5a7ca5968976676d6e2594554d`
- M0 产品源码提交：`07643ab45f1eaabfa6e44d5a57116496ad1c25d2`
- Product validate 已验证源码提交：`a2d19ceb5647ce050a5012ed2b8fdc1d7f7db4ab`
- M0 runtime 最终验证提交：`a1f10429a7f48b5a7ca5968976676d6e2594554d`
- Remote：`https://github.com/oarw/cakify.git`
- Visibility：`PRIVATE`
- 根产品 Cargo workspace 已建立，首批成员为 desktop、core、platform-windows。
- GPUI 空窗口和 fake Core bridge 已通过 Actions 的 fmt/check/tests/Clippy/release build 与最终 runtime smoke；三轮窗口完整可见，空闲整树 Working Set `35.477-37.121 MiB`，默认子进程 0，正常退出且无残留。
- 旧 benchmark 完整归档在 `archi/framework-benchmark-2026-08/`。
- 根 `.github/workflows/product-validate.yml` 只有 `workflow_dispatch`；push 不会自动运行。
- 最近实际成功 run：Windows runtime smoke `32093988986`，commit `a1f10429a7f48b5a7ca5968976676d6e2594554d`。
- Runtime smoke 前两轮分别暴露无效内存聚合与任务栏遮挡；第三轮全部硬门通过。本机始终未编译/测试/运行产品。
- 首个产品 `Cargo.lock` 已提交；最终 artifact 含 release EXE 和依赖树，详见第 12 节。
- Product validate 与 M0 runtime smoke 的公开前安全审计及 public -> Actions -> private 均已闭环，见 `docs/PUBLIC-ACTIONS-AUDIT.md`；仓库已恢复 PRIVATE。
- 仓库没有 LICENSE。

接手后先执行只读检查：`git status --short --branch`、`git rev-parse HEAD`、`git remote -v`、`gh repo view ... --json visibility,isPrivate`、`gh run list`。实际状态优先于本文。

## 4. 不可违反的执行约束

- 始终使用简体中文。
- 本机原则上只编辑源码；不安装大批环境，不执行项目编译、测试、benchmark、打包或发布。
- 完成源码/文档后自动 commit/push；不要等待用户再提醒推送。
- 2026 年 8 月私库 Actions 分钟已耗尽，PRIVATE 时不得运行 workflow。
- 用户已于 2026-08-17 持续授权本月后续受控闭环。每次 Actions 必须安全复核；无新增实质风险时自动执行：确认 private/无活动任务 -> public -> 只运行当前任务所需手动 workflow -> 核对 -> 确认无 queued/in_progress -> 立即 private，不再逐次询问。
- 自动授权不包括长期公开、Release/发包、无关 workflow 或审计发现新增风险后继续；这些情况请求用户确认。授权于 2026-08-31 23:59（Asia/Shanghai）或用户撤销时失效。
- 公开状态不得闲置或跨会话遗留；失败修复期间可维持同一闭环，但停止工作前必须恢复 private。
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

## 9. M0 结论与 M1 精确任务

源码任务 1–7 已在 `07643ab45f1eaabfa6e44d5a57116496ad1c25d2` 完成：workspace、三个首批 crate、Core 协议/fake loop、GPUI 空窗口、Windows 数据目录边界、手动 validate workflow 和 ADR 均已写入。格式、借用错误与锁文件在 `2fe81c3a4b2e1b744c9c0d003577da5482a7e24b`、`a2d19ceb5647ce050a5012ed2b8fdc1d7f7db4ab` 闭合。

`gpui-component` 的 M0 结论是拒绝当前依赖：调研 commit 的 lock 指向比产品 pin 落后 88 个提交的 GPUI，workspace 又未声明 Zed revision。没有继续运行 textarea/IME/体积测试，也没有把这些项目写成通过。详情见 `docs/decisions/0002-reject-gpui-component-for-m0.md`。

最终 Windows runtime smoke `32093988986` 已闭合 M0：三轮窗口矩形均为 `(24,55)-(1000,714)`，完整位于 runner 工作区；空闲整树 Working Set `37.121/35.480/35.477 MiB`，默认子进程 0，WM_CLOSE 后 exit 0 且无残留。artifact 与恢复 PRIVATE 事实见第 12 节。

接手后的顺序：

1. 建立 M1 SQLite storage actor、initial schema、migration runner 和连接配置硬门。
2. 实现 conversation/message/part/run repository、crash recovery 与领域约束测试。
3. 实现 Provider profile CRUD；SQLite 只存 opaque credential reference。
4. 实现 Credential Manager 主路径、DPAPI current-user 后备与 synthetic secret 测试。
5. 实现 live backup/restore 与 `integrity_check`；不要提前接真实 Provider。
6. 物理微软拼音/日文 IME、DPI、无障碍仍是 M2 独立人工门；M0 空壳没有真实输入框，不能写成 IME 通过。

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

窗口壳层已验证、其余仍是后续目标：

- 冷启动 P50 <= 400 ms、P95 <= 800 ms。
- 默认 idle 整树 Working Set <= 80 MiB。
- 默认子进程数 0。
- 10,000 消息真实虚拟化。
- 输入主线程无稳定 > 50 ms 阻塞。
- 工具取消到进程树退出 P95 <= 2 s。

最终 M0 产品窗口的句柄 ready 为 `118.279-145.450 ms`，空闲整树 Working Set 为 `35.477-37.121 MiB`，默认子进程 0。它只证明窗口壳层，不代表真实 composer、长列表或 Provider 已达门；M2/M3 必须重测。

## 12. Actions 记录

- Validate：<https://github.com/oarw/cakify/actions/runs/32017467536>，success，artifact `cargo-lock-32017467536`。
- Benchmark：<https://github.com/oarw/cakify/actions/runs/32017470781>，success，artifacts `benchmark-{gpui,avalonia,flutter,tauri}-32017470781`。
- 两者 commit：`40209896dca0009b747efc51ac885bed32b81f25`。
- Product validate：<https://github.com/oarw/cakify/actions/runs/32032509531>，failure，commit `9b6e71e07514c6f447de084a527d9a571b8368bd`，artifact ID `9289483786`；格式失败。
- Product validate：<https://github.com/oarw/cakify/actions/runs/32033412479>，failure，commit `2fe81c3a4b2e1b744c9c0d003577da5482a7e24b`，artifact ID `9289873982`；workspace check 发现 6 个同源 E0503。
- Product validate：<https://github.com/oarw/cakify/actions/runs/32034202488>，success，commit `a2d19ceb5647ce050a5012ed2b8fdc1d7f7db4ab`，job `95400694626`，artifact ID `9290400569`；fmt/check/tests/Clippy/release/upload 全部通过。
- 最终 artifact `product-validation-32034202488` digest `sha256:e9c4f5f0db1488d8f946acfcb2766d2d0ccd4f313fa4f7a476747639f9a8a7b5`；EXE 5,722,112 bytes，SHA-256 `4EB5AF9970EAFFC35850C599CD2A91685D6C1CC9FCB11B45526CA5B8D7DBF8DF`，未签名。
- 最终锁文件与仓库逐行一致，越界框架包与 artifact 文本 secret 0 命中。恢复 private 前已确认 queued/in_progress 为空；当前仓库 PRIVATE。
- Windows runtime smoke：<https://github.com/oarw/cakify/actions/runs/32037554962>，success 但内存顶层聚合无效，不能作为最终证据。
- Windows runtime smoke：<https://github.com/oarw/cakify/actions/runs/32038434473>，success，性能/生命周期证据有效，但截图显示任务栏遮挡，未接受为最终 M0 验收。
- Windows runtime smoke：<https://github.com/oarw/cakify/actions/runs/32093988986>，success，commit `a1f10429a7f48b5a7ca5968976676d6e2594554d`，job `95581655025`，artifact `windows-runtime-smoke-32093988986`（ID `9309416529`，digest `sha256:8a09c0785d1cc77257c798f26247a5bec16ae8e63b95ab129faa04a18430a6c3`）。三轮完整可见、空闲 Working Set `35.477-37.121 MiB`、0 子进程、正常退出且无残留；JSONL/日志/截图独立核验通过。
- 最终 runtime artifact EXE SHA-256 `CE54D290BD0F0A19F1CDDE0322C4A7C2D09838D62CCE4B5DDDAD276EA035EA78`，result JSON SHA-256 `9E9FEEB09AB3266E9098020B48E23F5DB55BDFA951811D0C85307E3F98FA5930`，截图 SHA-256 `34062732DE298CDED4B8BF9D58D0650C6D7F44B2B67C9C607C82509A2B202E12`。恢复前 queued/in_progress 为空，当前仓库 PRIVATE。

## 13. 交接槽位

- 当前分支：`main`
- M1 开始前源码基线 HEAD：`a1f10429a7f48b5a7ca5968976676d6e2594554d`
- M0 产品源码提交：`07643ab45f1eaabfa6e44d5a57116496ad1c25d2`
- Product validate 已验证提交：`a2d19ceb5647ce050a5012ed2b8fdc1d7f7db4ab`
- M0 runtime 最终验证提交：`a1f10429a7f48b5a7ca5968976676d6e2594554d`
- Remote：`https://github.com/oarw/cakify.git`
- Visibility：`PRIVATE`
- 当前 milestone：M1 数据与秘密基础；M0 已闭合
- 当前正在做：SQLite storage actor、initial schema 与 migration runner
- 最近成功 Actions：Windows runtime smoke `32093988986`
- 本轮 Actions：三次 runtime smoke；前两次证据分别因聚合错误/截图遮挡被拒绝，第三次最终通过
- 精确下一动作：实现 SQLite actor/schema/migration，再做 conversation/message/part/run repository
- 需要用户决定：项目许可证；M7 签名/发行渠道。8 月受控 visibility 闭环已有持续授权，不再逐次询问
- 已知风险：M1 schema/migration 尚未验证、GPUI pre-1.0、直接 UI 组件工作量、真实 IME/accessibility、M0 指标只覆盖窗口壳层、EXE 未签名
- 禁止误操作：不要重跑四候选；不要恢复归档 workflow；不要引入当前 `gpui-component`；不要复制 Zed GPL Agent UI；不要开始 RAG/远控

交接时用实际 `git rev-parse HEAD` 更新或解释 HEAD；不要让文档中的工作前基线冒充最新提交。
