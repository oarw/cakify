# Cakify 跨供应商交接文档

> 用途：新的 AI 模型、供应商或工程师开始前必须完整阅读。
> 最后更新：2026-08-19（Asia/Shanghai）
> 交接状态：`v0.1.0-pre.1` 已由统一 Release 流水线发布，安装版、便携版、独立 EXE 与校验文件可下载；继续推进消息持久化、会话 CRUD 与物理 IME。

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
- 本轮 UI/UX 与图标已验证源码 HEAD：`79997ef6febc72e6e445900ca6d2419c5f3ce4a9`；本次交接文档提交后以 `git rev-parse HEAD` 为准。本轮没有发布新版本，现有 prerelease 仍指向 `ae14994930c61eff61c33d51bee6974447e9192a`。
- M1 开始前源码基线 HEAD：`a1f10429a7f48b5a7ca5968976676d6e2594554d`
- M0 产品源码提交：`07643ab45f1eaabfa6e44d5a57116496ad1c25d2`
- Product validate/runtime 已验证源码提交：`79997ef6febc72e6e445900ca6d2419c5f3ce4a9`
- Release 已验证源码提交：`ae14994930c61eff61c33d51bee6974447e9192a`
- M0 runtime 最终验证提交：`a1f10429a7f48b5a7ca5968976676d6e2594554d`
- Remote：`https://github.com/oarw/cakify.git`
- Visibility：`PRIVATE`；UI/UX 验证闭环完成后已确认无 queued/in_progress 并恢复、复核。
- 最近 Product validate：`32257944799`，目标 HEAD `79997ef6febc72e6e445900ca6d2419c5f3ce4a9`，全量验证与 release artifact 已通过并核对。
- 根产品 Cargo workspace 已建立，成员为 desktop、core、platform-windows、provider、storage、mcp；聊天、工具与 MCP 切片已通过 Actions 与真实窗口 smoke。
- GPUI 空窗口和 fake Core bridge 已通过 Actions 的 fmt/check/tests/Clippy/release build 与最终 runtime smoke；三轮窗口完整可见，空闲整树 Working Set `35.477-37.121 MiB`，默认子进程 0，正常退出且无残留。
- 旧 benchmark 完整归档在 `archi/framework-benchmark-2026-08/`。
- 根 `.github/workflows/product-validate.yml` 只有 `workflow_dispatch`；push 不会自动运行。
- 最近实际成功 run：Windows runtime smoke `32259992090`；同一提交的 Product validate `32257944799` 也已通过。没有触发 Release。
- Runtime smoke 前两轮分别暴露无效内存聚合与任务栏遮挡；第三轮全部硬门通过。本机始终未编译/测试/运行产品。
- 产品 `Cargo.lock` 已提交；最新 Product validate artifact 含 11,015,680-byte release EXE、依赖树和三份 migration，详见第 12 节。
- 本轮 UI/UX 验证的 public -> Actions -> private 已闭环；仓库已恢复并复核为 PRIVATE。
- 已发布 prerelease：<https://github.com/oarw/cakify/releases/tag/v0.1.0-pre.1>。安装后三轮 ready `115.879-147.114 ms`、idle Working Set `36.867-39.223 MiB`，完整可见、单进程、正常退出，安装/卸载 exit code 均为 0。
- 仓库没有 LICENSE。
- `crates/cakify-mcp` 已固定 `rmcp 3.1.0`，实现 async actor、stdio/Streamable HTTP、工具发现/路由、并发与生命周期边界，stdio 使用 process-wrap Job Object/KillOnDrop；配置、路由、取消与生命周期 tests 已通过。
- Composer 已实现 selection、clipboard、鼠标拖选、多行导航和 marked-text/UTF-16 IME 接口；Provider 已通过真实 loopback HTTP、工具回填、有界 SSE 与错误脱敏契约。物理 IME、真实第三方 MCP 和真实用户 API Key 仍未验收。

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

1. 接消息持久化与会话 CRUD，覆盖流式增量、工具结果、失败/取消和重启恢复。
2. 建立微软拼音/日文 IME、候选窗、高 DPI 拖选、剪贴板与多行 composer 的 Windows 物理机门；实现长消息虚拟列表。
3. 将 Provider 改为可即时取消的 async transport；补 MCP 协议取消、状态重同步、工具变化通知、远程认证和真实 stdio 进程树 smoke。
4. 后续版本复用已经验证的统一 Release 流水线；不要重跑旧 commit，也不要手工创建 tag/上传资产。

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

- Product validate：<https://github.com/oarw/cakify/actions/runs/32257944799>，success，commit `79997ef6febc72e6e445900ca6d2419c5f3ce4a9`，job `96084032514`，artifact `product-validation-32257944799`（ID `9367654858`，digest `sha256:d1a294897137076e7486e23bd8985d0cbdaf5ced365a00a60e9321b637ac59b6`）。fmt/check/全量与专项 tests/Clippy/release build 全绿；EXE 11,015,680 bytes，SHA-256 `57DECF7E704DE0311EEB7CBA36B9253E4AF4CEB12F37CE38D1DBEC8452AED190`，`NotSigned`，锁文件规范化匹配仓库，EXE 关联图标已提取查看。
- Windows runtime smoke：<https://github.com/oarw/cakify/actions/runs/32259992090>，success，同一 commit，job `96090679510`，artifact `windows-runtime-smoke-32259992090`（ID `9368112302`，digest `sha256:2f555408e24881dad67ceb58336a2c497ae50f39c0996c44cfe249eb4e76d196`）。三轮 ready `113.600-149.608 ms`，idle Working Set `39.879-44.285 MiB`，完整可见、单进程、正常退出且无残留；截图中 UI 无遮挡。
- 本轮四个中间失败 run 为 `32256003025`/`32256461763`/`32256869809`/`32257241135`，分别对应 rustfmt、解析闭合、runner 格式与 GPUI ElementId 类型问题，后续均在同一修复闭环关闭。恢复前 queued/in_progress 为 0，当前仓库 `PRIVATE`，本轮没有 Release。
- Release：<https://github.com/oarw/cakify/actions/runs/32249902570>，success，commit `ae14994930c61eff61c33d51bee6974447e9192a`，jobs `96058366384`/`96062786058`，artifact `release-candidate-32249902570`（ID `9364501538`，digest `sha256:f3f2824997a8b152fa8566793b75353cb3712456eb1ec8b6ed25dfee6a9ff4e1`）。安装版、便携 ZIP、独立 EXE 与 SHA256SUMS 已发布并独立核验；仓库已恢复 PRIVATE。
- Product validate：<https://github.com/oarw/cakify/actions/runs/32229464063>，success，commit `a1233f18e31022042236d056faa4376e33639ea7`，job `95996012638`，artifact `product-validation-32229464063`（ID `9357175604`，digest `sha256:0ce96aba1d9f96b7d6bff4685c4ee52ae090e8ac35749c08c7cac3a1db3668b5`）。fmt/check/tests/专项 contracts/Clippy/release 全绿；EXE SHA-256 `E5CA7F9A4B15F207958BF3FE79ADDF8B866C67B50E8F5B51632F0638B09FB965`。
- Windows runtime smoke：<https://github.com/oarw/cakify/actions/runs/32231259895>，success，同一 commit，job `96001355368`，artifact `windows-runtime-smoke-32231259895`（ID `9357552182`，digest `sha256:e0c15eb062fe2c463b3bd182f7fbde30031ce6ac6e2e0fc02b7a5b9f025ae192`）。三轮 ready `129.433-157.840 ms`，idle Working Set `36.734-39.293 MiB`，完整可见、单进程、正常退出、无残留；截图已独立核对。

- Validate：<https://github.com/oarw/cakify/actions/runs/32017467536>，success，artifact `cargo-lock-32017467536`。
- Benchmark：<https://github.com/oarw/cakify/actions/runs/32017470781>，success，artifacts `benchmark-{gpui,avalonia,flutter,tauri}-32017470781`。
- 两者 commit：`40209896dca0009b747efc51ac885bed32b81f25`。
- Product validate：<https://github.com/oarw/cakify/actions/runs/32032509531>，failure，commit `9b6e71e07514c6f447de084a527d9a571b8368bd`，artifact ID `9289483786`；格式失败。
- Product validate：<https://github.com/oarw/cakify/actions/runs/32033412479>，failure，commit `2fe81c3a4b2e1b744c9c0d003577da5482a7e24b`，artifact ID `9289873982`；workspace check 发现 6 个同源 E0503。
- Product validate：<https://github.com/oarw/cakify/actions/runs/32034202488>，success，commit `a2d19ceb5647ce050a5012ed2b8fdc1d7f7db4ab`，job `95400694626`，artifact ID `9290400569`；fmt/check/tests/Clippy/release/upload 全部通过。
- 最终 artifact `product-validation-32034202488` digest `sha256:e9c4f5f0db1488d8f946acfcb2766d2d0ccd4f313fa4f7a476747639f9a8a7b5`；EXE 5,722,112 bytes，SHA-256 `4EB5AF9970EAFFC35850C599CD2A91685D6C1CC9FCB11B45526CA5B8D7DBF8DF`，未签名。
- 最终锁文件与仓库逐行一致，越界框架包与 artifact 文本 secret 0 命中。恢复 private 前已确认 queued/in_progress 为空；当前仓库 PRIVATE。
- Product validate：<https://github.com/oarw/cakify/actions/runs/32127969715>，success，commit `054aaf6b0ea939d41f455921ced714e4461ed5fa`，job `95682647629`，artifact `product-validation-32127969715`（ID `9321446137`，digest `sha256:1b4f6f03c4a6d0883f5c11d94b87a061d2d38db1e660d8401433d5d6fb6c795d`）。fmt/check/全量 tests、storage/repository/provider/secret contracts、Clippy、release build 与上传全部成功；6 files、2,386,299 bytes，release EXE 5,722,624 bytes，SHA-256 `9C63E9A44A8C7AC78D03FDCDAC4B3F9922E9A2388A9122B97F75B226982F3E0D`，`NotSigned`。锁文件归一化匹配仓库，越界 GPL AI crate 与文本 secret 0 命中；恢复前 queued/in_progress 为空，随后复核仓库 PRIVATE。
- Product validate：<https://github.com/oarw/cakify/actions/runs/32153002500>，success，commit `cf822f00f9958111973dc7e93903a1515f9726db`，job `95763320261`，artifact `product-validation-32153002500`（ID `9331090742`，digest `sha256:157d7597834867e13e08b9026c90a5335aa0e7c2dd03d6dcf9f6926f1ab98413`）。全门通过；release EXE 9,249,280 bytes，SHA-256 `8EC4399E668667142752B8E6380AFA06D034841B9ED0072FFC69FB30D58EDFE1`，`NotSigned`；artifact 内容独立核对无异常。
- Windows runtime smoke：<https://github.com/oarw/cakify/actions/runs/32154636851>，success，同一 commit，job `95768769663`，artifact `windows-runtime-smoke-32154636851`（ID `9331473360`，digest `sha256:0084053b94a7468c9fb6ba25c4ee0b2875312f0eefbb46c7feb0b87f353e3413`）。三轮真实聊天窗口完整可见，idle Working Set `36.164-38.863 MiB`、0 子进程、正常退出且无残留；截图已独立核对。
- Windows runtime smoke：<https://github.com/oarw/cakify/actions/runs/32037554962>，success 但内存顶层聚合无效，不能作为最终证据。
- Windows runtime smoke：<https://github.com/oarw/cakify/actions/runs/32038434473>，success，性能/生命周期证据有效，但截图显示任务栏遮挡，未接受为最终 M0 验收。
- Windows runtime smoke：<https://github.com/oarw/cakify/actions/runs/32093988986>，success，commit `a1f10429a7f48b5a7ca5968976676d6e2594554d`，job `95581655025`，artifact `windows-runtime-smoke-32093988986`（ID `9309416529`，digest `sha256:8a09c0785d1cc77257c798f26247a5bec16ae8e63b95ab129faa04a18430a6c3`）。三轮完整可见、空闲 Working Set `35.477-37.121 MiB`、0 子进程、正常退出且无残留；JSONL/日志/截图独立核验通过。
- 最终 runtime artifact EXE SHA-256 `CE54D290BD0F0A19F1CDDE0322C4A7C2D09838D62CCE4B5DDDAD276EA035EA78`，result JSON SHA-256 `9E9FEEB09AB3266E9098020B48E23F5DB55BDFA951811D0C85307E3F98FA5930`，截图 SHA-256 `34062732DE298CDED4B8BF9D58D0650C6D7F44B2B67C9C607C82509A2B202E12`。恢复前 queued/in_progress 为空，当前仓库 PRIVATE。

## 13. 交接槽位

- 当前分支：`main`
- M1 开始前源码基线 HEAD：`a1f10429a7f48b5a7ca5968976676d6e2594554d`
- M0 产品源码提交：`07643ab45f1eaabfa6e44d5a57116496ad1c25d2`
- Product validate/runtime 已验证提交：`79997ef6febc72e6e445900ca6d2419c5f3ce4a9`
- Release 已验证提交：`ae14994930c61eff61c33d51bee6974447e9192a`
- M0 runtime 最终验证提交：`a1f10429a7f48b5a7ca5968976676d6e2594554d`
- Remote：`https://github.com/oarw/cakify.git`
- Visibility：`PRIVATE`
- 当前 milestone：M2/M3 聊天垂直切片；M0 与 M1 SecretStore 已闭合
- 当前正在做：聊天工作台 UI/UX 与 Cakify 应用图标已完成并验证；下一项是消息持久化与会话 CRUD
- 最近成功 Actions：Product validate `32257944799`、Windows runtime smoke `32259992090`
- 本轮 Actions：全量验证、Release EXE、三轮原生窗口 smoke、artifact、截图与 EXE 关联图标均已核对；没有发布新版本，仓库已恢复 PRIVATE
- 精确下一动作：接消息持久化与会话 CRUD，让发送、流式增量、工具结果、失败/取消和重启恢复落入现有 storage actor；随后补物理 IME/长列表和 Provider/MCP 取消硬化
- 需要用户决定：项目许可证；M7 签名/发行渠道。8 月受控 visibility 闭环已有持续授权，不再逐次询问
- 已知风险：blocking Provider 在无网络数据期间不能即时取消；MCP 尚缺协议取消/状态重同步/工具变化通知/远程认证和真实第三方互操作；未使用真实用户 Key 做在线 smoke；消息持久化/物理 IME 未闭合；EXE 与安装器未签名
- 安全状态：仓库 `PRIVATE`，无 queued/in_progress Actions；`v0.1.0-pre.1` prerelease 已发布且资产可由授权账号下载
- 禁止误操作：不要重跑四候选；不要恢复归档 workflow；不要引入当前 `gpui-component`；不要复制 Zed GPL Agent UI；不要开始 RAG/远控

交接时用实际 `git rev-parse HEAD` 更新或解释 HEAD；不要让文档中的工作前基线冒充最新提交。
