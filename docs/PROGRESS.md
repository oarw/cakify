# Cakify 进度记录

> 本文件是项目状态的单一事实来源。
> 最后更新：2026-08-18（Asia/Shanghai）
> 当前阶段：M2/M3 - 聊天垂直切片
> 当前状态：M2_CHAT_SLICE_IN_PROGRESS

## 1. 当前快照

- 工作目录：`C:\Users\admin\Desktop\code\cakify`
- 分支：`main`，跟踪 `origin/main`。
- M1 开始前源码基线 HEAD：`a1f10429a7f48b5a7ca5968976676d6e2594554d`。
- M0 产品源码提交：`07643ab45f1eaabfa6e44d5a57116496ad1c25d2`（`feat: bootstrap GPUI product workspace`）。
- M0 最终 runtime 验证提交：`a1f10429a7f48b5a7ca5968976676d6e2594554d`。
- GitHub remote：`https://github.com/oarw/cakify.git`。
- 仓库可见性：`PRIVATE`；SecretStore Product validate `32127969715` 完成后确认 queued/in_progress 均为 0，已恢复并复核。
- 最近成功 Actions：Product validate `32127969715`，目标 commit `054aaf6b0ea939d41f455921ced714e4461ed5fa`；fmt/check/全量 tests、storage/repository/provider/secret contracts、Clippy、release build 与 artifact upload 全部通过。
- 本轮 Actions：runtime smoke 首轮 `32037554962` 的内存汇总证据无效；第二轮 `32038434473` 的性能/生命周期证据有效但截图暴露任务栏遮挡；第三轮 `32093988986` 全部硬门和 artifact 独立核验通过。
- Runtime smoke 首轮 `32037554962` 的窗口、单进程与 WM_CLOSE 退出通过，但汇总层把真实进程内存错误写成 0，因此内存证据无效。
- Runtime smoke 第二轮 `32038434473` 取得三轮非零内存、单进程、标题和正常退出证据；但截图显示窗口底部约 27 px 被任务栏遮挡，因此当时没有作为最终 M0 验收。
- Runtime smoke 第三轮 `32093988986` 三轮窗口都完整位于工作区，空闲整树 Working Set `35.477-37.121 MiB`，默认子进程 0，正常退出且无残留；M0 已关闭。
- 根 `.github/workflows/product-validate.yml`：已创建且只有 `workflow_dispatch`，push 不会自动触发。
- 根 `.github/workflows/windows-runtime-smoke.yml`：只有 `workflow_dispatch`；完整可见、内存、进程树和退出硬门已在最终 run 通过。
- 产品源码：根 Cargo workspace 已建立，包含 desktop、core、platform-windows、provider 和 storage；聊天垂直切片源码正在接入，尚未经过 Actions 编译验证。
- M1 源码：SQLite foundation、conversation/message/part/run repository、crash recovery、Provider profile CRUD 和 SecretStore 生命周期均已通过 Product validate；Windows CredMan、DPAPI current-user adapter、密文文件和 synthetic contracts 已闭合。live backup/restore 暂缓，先完成用户明确要求的聊天垂直切片。
- M1 依赖：固定 `rusqlite 0.40.2`、`sha2 0.10.9`，并把锁文件中已有的 `url 2.5.8` 设为 storage 直接依赖做 endpoint 结构化解析；最终依赖树未发现 GPL AI 业务 crate、向量库或密钥库越界包。
- 产品构建状态：本机没有编译/测试；Actions 已验证构建并实际生成 M0 与 M1 release EXE。M0 三次窗口 ready 为 `145.450/129.692/118.279 ms`，空闲 Working Set 为 `37.121/35.480/35.477 MiB`，均完整可见、单进程并正常退出；IME 保持 M2 独立物理机门。
- 产品计划：Markdown 架构/安全/路线图/来源和离线 HTML 已写入。
- 本次公开前审计与 visibility 闭环：已完成，见 `docs/PUBLIC-ACTIONS-AUDIT.md`；仓库已恢复 PRIVATE。
- 组件决定：M0 不引入 `gpui-component`；直接使用 GPUI primitives，见 ADR 0002。
- 历史 benchmark：完整移入 `archi/framework-benchmark-2026-08/`。
- 许可证：尚未选择，仓库仍没有 `LICENSE`。

## 2.1 当前聊天垂直切片（未验证）

- Core 已从 fake draft 状态回执升级为带对话历史的 `ChatProvider` 边界，支持文本 delta 合并、usage/finish、取消、失败、工具调用 delta 和审批事件。
- 新增 `crates/cakify-provider`：OpenAI-compatible SSE adapter、HTTPS/loopback endpoint 校验、禁用重定向、SecretStore 按请求读取 API Key、脱敏 HTTP 错误和 parser 契约测试源码。
- GPUI desktop 已接入官方 GPUI editor 示例改造的多行 composer、Enter 发送/Shift+Enter 换行、消息时间线、流式 assistant 更新、停止/重试、CommonMark 基础 block/code rendering。
- Provider 面板可以保存 endpoint/model/API Key；API Key 走 Windows Credential Manager，profile 走 SQLite。MCP 面板可以维护 stdio/HTTP server 草稿，工具审批行可以显示允许/拒绝状态。
- 当前明确未声称完成：本轮源码尚未在 Actions 运行；中文/日文物理 IME selection/clipboard 尚未验收；MCP `rmcp` 连接/进程 Job Object 尚未接入；消息尚未接 storage actor 做持久化；MCP 草稿尚未持久化。

## 2. 当前产品决定

- UI：GPUI + Rust。
- Core：同进程 Rust；UI 与核心通过 bounded typed command/event 通信。
- 数据：SQLite + `rusqlite` bundled；storage actor、WAL、migration。
- 密钥：Windows Credential Manager 主路径；DPAPI current-user 后备。
- Agent：小型 reducer/effect loop、显式 tool event、可取消。
- MCP：官方 `rmcp`，stdio + Streamable HTTP。
- 子进程：Windows Job Object 管理；默认启动子进程数为 0。
- 构建/测试/基准/打包/发布：GitHub Actions；本机只编辑和静态解析。
- 检索：内置搜索端点返回 HTTP 404 时，不原地反复重试；主动切换可用 MCP 检索工具，必要时使用官方 API 或官方仓库 CLI，并坚持官方/一手来源。
- 回退：只有 GPUI 的 IME、accessibility、维护或性能硬门持续失败才评估 Avalonia。

## 3. 产品范围

功能完整度向 Cherry Studio 与 RikkaHub 的轻量聊天能力看齐：

- 多 Provider/模型、兼容 endpoint。
- 完整会话管理、搜索/分支/导入导出。
- 流式聊天、停止、重试、编辑重发、重新生成。
- Markdown/代码、附件、system prompt、常用提示词。
- tool/function calling、审批、取消、错误恢复。
- MCP server 管理、启停、权限和会话绑定。
- 主题、快捷键、草稿、数据与隐私管理。

明确不做：知识库/RAG、embedding/向量库、远程操控、云同步后台、内置大模型下载、重量插件 runtime、多 Agent 自动编排和默认任意 shell。

## 4. Zed/GPUI 调研结论

- Zed 已验证 GPUI 可以做成熟 AI 对话：输入、线程、消息队列、上下文、工具时间线、审批、MCP 与压缩。
- GPUI 为 Apache-2.0，可作为产品 UI 框架。
- Zed `agent`/`agent_ui` 等 AI 业务 crate 是 GPL-3.0-or-later；当前不得复制或依赖，只参考公开行为并独立实现。
- GPUI 官方输入接口覆盖 selection、marked/composition range、UTF-8/UTF-16 转换；真实中文 IME 仍必须验证。
- Apache-2.0 `gpui-component` 有 textarea、Markdown、virtual list 和主题，但当前 revision 未固定 Zed 依赖且与产品 pin 不一致；M0 已决定不引入，满足 ADR 0002 的重新评估条件后再讨论。

## 5. 已完成

- [x] 四候选 GPUI/Avalonia/Flutter/Tauri benchmark 与最终报告。
- [x] 选择 GPUI 主线、Avalonia 回退。
- [x] 核实 GPUI/Zed AI 源码与许可证边界。
- [x] 调研 GPUI input/testing、`gpui-component`、SQLite、CredMan/DPAPI、MCP/rmcp、Job Object 与 Windows 发布。
- [x] 写 `docs/PRODUCT-PLAN.md` 与离线 `docs/PRODUCT-PLAN.html`。
- [x] 写 `docs/ARCHITECTURE.md`。
- [x] 写 `docs/SECURITY-AND-DATA.md`。
- [x] 写 `docs/ROADMAP.md`。
- [x] 写 `docs/RESEARCH-SOURCES.md`。
- [x] 把 benchmark app/core/fixture/script/workflow/report 整体迁入 `archi/`。
- [x] 更新根 README、进度和交接入口。
- [x] 完成静态核验：Markdown 本地链接无缺失、HTML 章节标签配对、无外部脚本/字体、敏感值模式无命中、`git diff --check` 无错误。
- [x] 创建 Rust `1.97.1` 产品 workspace，固定 GPUI/Zed commit 与首批 crates.io 版本。
- [x] 创建 `cakify-desktop`、`cakify-core`、`cakify-platform-windows` 三个首批成员。
- [x] 实现有界 command/event bridge、单调 revision、fake core loop 和测试源码。
- [x] 实现 GPUI 原生空窗口，连接同进程 Core，不使用 benchmark HTTP sidecar。
- [x] 建立 Windows 本地数据目录边界；真实 known-folder/CredMan/DPAPI FFI 留在 M1。
- [x] 创建只允许手动触发的 product validate workflow，计划输出 release EXE、`Cargo.lock` 与依赖树。
- [x] 写 ADR 0001/0002，固定 runtime/thread/pin，并基于 revision 不兼容门拒绝 M0 引入 `gpui-component`。
- [x] 对新增源码做静态结构、链接、敏感模式、workflow trigger、括号和 staged diff 检查；没有把它们写成编译通过。
- [x] 完成本次公开前安全审计：Git 全历史、GitHub secrets/config、10 个 run 日志、20 个 artifact、cache 元数据、LFS、Release、Issue/PR、fork 与许可证均已检查。
- [x] 经用户明确授权执行本次临时公开，并只触发 Product validate；首轮准确记录为格式检查失败。
- [x] 从首轮 artifact 取回并核对首个产品 `Cargo.lock` 与依赖树；锁文件固定 GPUI/Zed revision，未发现候选框架越界依赖。
- [x] 修复 rustfmt 差异与 Core revision E0503 借用冲突，提交锁文件。
- [x] Product validate `32034202488` 通过 fmt/check/tests/Clippy/release build，并产出 release EXE、锁文件与依赖树。
- [x] 核对最终 artifact 的哈希、锁文件逐行一致性、固定 Zed revision、越界依赖和文本 secret；确认无活动任务后恢复 PRIVATE。
- [x] 将用户的 2026 年 8 月后续 Actions 受控闭环持续授权写入 AGENTS、Cursor 规则、计划、进度和交接文档。
- [x] 实现 Windows runtime smoke 脚本与独立手动 workflow：三轮主窗口探针、进程树采样、80 MiB idle 门、0 默认子进程门、WM_CLOSE 正常退出、残留检查、截图与结构化 artifact。
- [x] 对 runtime smoke 做本机静态核验：PowerShell AST 解析错误 0，workflow 只有手动触发与 `contents: read`，不在本机执行 EXE 或 Cargo。
- [x] Windows runtime smoke `32093988986` 通过三轮窗口完整可见、80 MiB idle、0 子进程、WM_CLOSE/无残留硬门；artifact JSONL、日志、哈希和截图已独立核对，仓库已恢复 PRIVATE。
- [x] Product validate `32097907337` 在含 runner 锁文件的 commit 上通过 SQLite actor、migration、schema/外键/checksum/重开/未来版本 contract、Clippy 与 release build；仓库已恢复 PRIVATE。
- [x] Product validate `32100910742` 通过 conversation/message/part/run repository、v3 migration、稳定分页、聚合事务、checkpoint revision、run 终态和一次性 crash recovery contract；仓库已恢复 PRIVATE。
- [x] Product validate `32119057930` 通过 Provider profile create/get/list/update/disable/delete、stale write、原子 model cache、endpoint/metadata/capability JSON、opaque credential reference 与删除级联 contract；release EXE 和 artifact 已独立核对，仓库已恢复 PRIVATE。
- [x] Product validate `32127969715` 通过 SecretStore lifecycle、Credential Manager put/get/update/delete、DPAPI current-user round-trip/tamper、workspace tests、Clippy 和 release build；artifact 已独立核对，仓库已恢复 PRIVATE。

## 6. 尚未完成

- [x] 实现 SQLite initial schema/migration/storage actor。
- [x] 实现 Credential Manager/DPAPI SecretStore；Product validate `32127969715` 的 lifecycle、CredMan、DPAPI round-trip/tamper、Clippy、release build 全部通过。
- [ ] 实现 fake provider 聊天纵向切片。
- [ ] 实现真实 OpenAI-compatible provider。
- [ ] 实现 Agent/tool approval/Job Object。
- [ ] 实现 MCP stdio/Streamable HTTP。
- [ ] 实现轻量产品完善与发布流程。

## 7. 精确下一步

下一位执行者直接实施 M1，不再做框架或 M0 runtime 泛泛选型：

1. 实现 SQLite live backup/restore，使用 Backup API 或 `VACUUM INTO`，禁止裸复制活动 `.db`。
2. 在 Windows Actions 验证恢复后的 `integrity_check`、领域计数、WAL/备份边界和无测试 secret 导出。
3. M1 数据/密钥闭合后进入 M2 GPUI composer、虚拟消息列表和 fake stream。

详细验收见 `docs/ROADMAP.md` 的 M1。

## 8. 性能与质量门

以下是全产品目标；M0 已取得窗口壳层的启动、内存、进程树与退出证据，聊天交互和后续数据层仍按 milestone 分别验收：

- 冷启动到可交互 P50 <= 400 ms、P95 <= 800 ms。
- 默认 idle 整树 Working Set <= 80 MiB。
- 默认子进程数 0。
- 10,000 消息使用真实虚拟列表。
- 流式更新按 16–33 ms 合并。
- 工具取消到整棵进程树退出 P95 <= 2 s。
- SQLite、日志、导出、artifact 中明文 secret 命中数 0。

最终 M0 产品窗口实测为：窗口句柄 ready `118.279-145.450 ms`、空闲整树 Working Set `35.477-37.121 MiB`、默认子进程数 0。ready 只代表 M0 窗口创建，不等于完整聊天输入可交互；完整冷启动目标将在 M2 真实 composer/消息列表后重测。

本机未运行项目编译、测试、GUI、benchmark 或 package。任何门只按对应 Actions/物理机实际证据标记通过；未覆盖的 M1–M7 门仍保持未完成。

## 9. 当前阻塞与风险

- `PRIVATE_ACTIONS_QUOTA`：2026 年 8 月私库 Actions 分钟已耗尽。
- `PUBLIC_CYCLE_AUTOMATION`：用户已持续授权 8 月后续受控闭环；每次仍须安全复核，公开不得闲置，新增风险或扩大范围时停止并请求确认。
- `LICENSE_PENDING`：根仓库无 LICENSE，最终开源/闭源策略未定。
- `GPUI_PRE_1_0`：必须固定 commit，升级单独处理。
- `M1_BACKUP_PENDING`：SQLite、repository/crash recovery、Provider profile 和 Windows SecretStore 已通过对应 Product validate；live backup/restore 尚未实现，真实聊天 UI/Provider 仍未开始。
- `M1_ARTIFACT_DOWNLOAD_PENDING`：storage foundation 与 repository 最终 artifacts 分别已由 runner 成功上传 5/6 个文件，ID/大小/digest 与上传日志一致；本机到两个 Azure Blob 主机持续连接超时，因此最终 ZIP 内容独立解包检查仍待网络恢复后补做。workflow 结论和仓库恢复不受影响，不能把尚未下载写成已核对。
- `DIRECT_GPUI_UI_WORK`：M0 已拒绝当前 `gpui-component` 依赖，聊天输入、Markdown 和组件需要直接实现与维护。
- `IME_ACCESSIBILITY_GAP`：真实微软拼音、日文 IME、DPI、多显示器、UI Automation 尚未验证。
- `M0_METRICS_SCOPE`：最终 M0 指标只覆盖窗口壳层，不冒充真实 composer、长消息列表或 Provider 启动性能；M2/M3 必须重测。
- `SIGNING_PENDING`：签名证书、MSIX/安装器和更新通道未决定，安排在 M7。

## 10. Actions 事实记录

- [历史 benchmark Validate #32017467536](https://github.com/oarw/cakify/actions/runs/32017467536)：`success`，commit `40209896dca0009b747efc51ac885bed32b81f25`，artifact `cargo-lock-32017467536`。GitHub 曾因短暂复用 workflow 路径而改变其显示名；它不是产品 M0 run。
- [Benchmark candidates #32017470781](https://github.com/oarw/cakify/actions/runs/32017470781)：`success`，同一 commit，artifacts `benchmark-{gpui,avalonia,flutter,tauri}-32017470781`。
- [Product validate #32032509531](https://github.com/oarw/cakify/actions/runs/32032509531)：`failure`，commit `9b6e71e07514c6f447de084a527d9a571b8368bd`，artifact `product-validation-32032509531`（ID `9289483786`，含 `Cargo.lock`、`dependency-tree.txt`，无 EXE）。依赖/许可证边界通过，`cargo fmt --check` 失败，后续步骤被跳过。
- [Product validate #32033412479](https://github.com/oarw/cakify/actions/runs/32033412479)：`failure`，commit `2fe81c3a4b2e1b744c9c0d003577da5482a7e24b`，artifact `product-validation-32033412479`（ID `9289873982`，含锁文件与依赖树，无 EXE）。格式通过；workspace check 在 Core 发现 6 个同源 E0503 借用错误，后续步骤被跳过。
- [Product validate #32034202488](https://github.com/oarw/cakify/actions/runs/32034202488)：`success`，commit `a2d19ceb5647ce050a5012ed2b8fdc1d7f7db4ab`，job `95400694626`，artifact `product-validation-32034202488`（ID `9290400569`，archive digest `sha256:e9c4f5f0db1488d8f946acfcb2766d2d0ccd4f313fa4f7a476747639f9a8a7b5`）。fmt、workspace check、tests、Clippy、release build 与上传全部通过。
- 最终 EXE：5,722,112 bytes，SHA-256 `4EB5AF9970EAFFC35850C599CD2A91685D6C1CC9FCB11B45526CA5B8D7DBF8DF`，`NotSigned`（M7 前预期状态）。artifact `Cargo.lock` 与仓库逐行一致，依赖树 SHA-256 `4F424A9412718C56907ECE687A2342E55F491C9C6F7E4B3BFE3712E3276E729A`，越界框架包与文本 secret 均 0 命中。
- 恢复 private 前再次确认最终 run 为 completed/success，queued 与 in_progress 均为空；随后恢复 PRIVATE 并复核可见性。
- [Windows runtime smoke #32037554962](https://github.com/oarw/cakify/actions/runs/32037554962)：GitHub `success`，commit `95b17b3e41d5b658a55d169615c32fb29dfcc51c`，job `95410897261`，artifact `windows-runtime-smoke-32037554962`（ID `9291266584`，digest `sha256:37933cd52977dcbd1a57b3412b53dd325845b98a2e35310ccb802cd793b20f15`）。三轮窗口 ready `157.141/131.985/133.119 ms`，均为 1 个进程，WM_CLOSE 后约 `33.577/35.500/35.465 ms` 以 code 0 退出；但顶层内存汇总错误为 0，不能作为 80 MiB 门通过证据。
- [Windows runtime smoke #32038434473](https://github.com/oarw/cakify/actions/runs/32038434473)：GitHub `success`，commit `2c1125e10ff135656c078b69ffecf636d64fd728`，job `95413313494`，artifact `windows-runtime-smoke-32038434473`（ID `9291484529`，digest `sha256:dbf7314d6987caae8833d3387a16c665c901563b57dfb29e4b0ca2fab09c2128`）。三轮窗口标题均为 `Cakify`，ready `160.087/122.374/128.065 ms`，空闲 Working Set `37.289/35.668/35.684 MiB`，峰值最高 `38.449 MiB`，均为单进程、0 子进程、WM_CLOSE 后 `30.913/26.962/26.434 ms` 以 code 0 退出；JSONL 独立复算一致，日志为空。截图显示 960x680 内容窗口底部约 27 px 被任务栏遮挡，因此该 run 只接受性能/生命周期证据，不作为最终 M0 完整可见验收。
- [Windows runtime smoke #32093988986](https://github.com/oarw/cakify/actions/runs/32093988986)：GitHub `success`，commit `a1f10429a7f48b5a7ca5968976676d6e2594554d`，job `95581655025`，artifact `windows-runtime-smoke-32093988986`（ID `9309416529`，digest `sha256:8a09c0785d1cc77257c798f26247a5bec16ae8e63b95ab129faa04a18430a6c3`）。三轮窗口矩形均为 `(24,55)-(1000,714)`，完整位于 `(0,0)-(1024,720)` 工作区；ready `145.450/129.692/118.279 ms`，空闲 Working Set `37.121/35.480/35.477 MiB`，峰值最高 `38.320 MiB`，均为单进程、0 子进程、WM_CLOSE 后 `35.356/30.831/31.409 ms` 以 code 0 退出且无残留。JSONL 独立复算逐项一致，6 份日志为空白，artifact 文本 secret 命中 0，截图无遮挡。
- 最终 runtime artifact：EXE 5,722,624 bytes，SHA-256 `CE54D290BD0F0A19F1CDDE0322C4A7C2D09838D62CCE4B5DDDAD276EA035EA78`；result JSON SHA-256 `9E9FEEB09AB3266E9098020B48E23F5DB55BDFA951811D0C85307E3F98FA5930`；截图 SHA-256 `34062732DE298CDED4B8BF9D58D0650C6D7F44B2B67C9C607C82509A2B202E12`。恢复前 queued/in_progress 均为空，随后已复核仓库为 PRIVATE。
- [Product validate #32097396883](https://github.com/oarw/cakify/actions/runs/32097396883)：`failure`，commit `900bcde26847fc9910d50823469262bb4295ee9c`，job `95591302608`，artifact `product-validation-32097396883`（ID `9310434562`，digest `sha256:f1c82871e39b1e5ac87188fa1c9608211a52826d5f8b3ae470a7bb75ca2add34`）。依赖树/许可证边界成功，格式检查失败，后续 check/test/storage contract/Clippy/release 均被跳过；不能记为 M1 通过。artifact `Cargo.lock` SHA-256 `731531574FD1B25AA23F8B0476BF60365D2529B894F50FE5A0AC020B34441E30`，dependency tree SHA-256 `8F85318D7203E7DE9B5BC223EF741F979C4EC5A1A3831CB3B000AF552F4C2684`；两份 migration 与目标提交内容一致，仅 runner checkout 使用 CRLF。
- [Product validate #32097907337](https://github.com/oarw/cakify/actions/runs/32097907337)：`success`，commit `785241720db087ce38121b095ea5f192063ab2b4`，job `95592703383`，artifact `product-validation-32097907337`（ID `9310763337`，digest `sha256:66b893168eadc5ead939c71c4059ca65f15cc6c9f2b2c38c2e3f49a2274ab118`，5 files，2,385,820 bytes）。fmt、workspace check、全量 tests、5 项 storage contract、Clippy、release build 与上传全部成功；release 优化构建用时 3m51s。artifact ZIP 因本机连接 `productionresultssa8.blob.core.windows.net:443` 持续超时尚未解包，不能写成内容已独立核对。
- [Product validate #32100633458](https://github.com/oarw/cakify/actions/runs/32100633458)：`failure`，commit `2f4b8688fc71ae727781baa7ac9306db48f9e2aa`，job `95600298727`，artifact `product-validation-32100633458`（ID `9311450847`，digest `sha256:b15cb49d2ab6970c9de214e8a215c0856a8c00fd8bfc0663223da0401e8de9a2`）。依赖树/许可证边界成功，格式检查失败；check/tests/storage/repository contract/Clippy/release 均跳过，不能写成 repository 通过。
- [Product validate #32100910742](https://github.com/oarw/cakify/actions/runs/32100910742)：`success`，commit `621097cdc08a9ac5129eef2200c2b8c7628504e2`，job `95601074839`，artifact `product-validation-32100910742`（ID `9311722769`，digest `sha256:51d059c6089178c9afb56c858d594d744892071da2a1b28cd6edf24e96f144af`，6 files，2,386,139 bytes）。fmt/check/全量 tests、storage contract 5/5、repository contract 4/4、Clippy、release build 与上传全部成功；release 优化构建用时 3m43s。artifact ZIP 因本机连接 Azure Blob 超时尚未解包，不能写成内容已独立核对。
- [Product validate #32118633092](https://github.com/oarw/cakify/actions/runs/32118633092)：`failure`，commit `0cca0725f23ff73118cb03bdb45de311d7634800`，job `95653757929`，artifact `product-validation-32118633092`（ID `9317717492`，digest `sha256:8e3feac94207dafbbcd120e6c4e301ccb30366403969e6d5a78bbd44bec9aebc`，47,139 bytes）。依赖树/许可证边界通过，格式检查失败；workspace check、tests、三个 contract、Clippy 与 release build 均跳过，不能记为 Provider 通过。
- [Product validate #32119057930](https://github.com/oarw/cakify/actions/runs/32119057930)：`success`，commit `9673349691062c80349f358d0ec8fc0a61364180`，job `95655067997`，artifact `product-validation-32119057930`（ID `9318178539`，digest `sha256:e1e5a95bdf6d5b726ed98c14481a1c6dbee765189cc9f947d8f0ae524492fa9b`，6 files，2,386,150 bytes）。fmt/check/全量 tests、storage contract 5/5、repository contract 4/4、provider profile contract 4/4、Clippy、release build 与上传全部成功；release 优化构建用时 4m14s。
- [Product validate #32127609188](https://github.com/oarw/cakify/actions/runs/32127609188)：`failure`，commit `c6e109b5bbc741e37486913fda1ed94e4829d8f0`，job `95681546487`，artifact `product-validation-32127609188`（ID `9321000183`，digest `sha256:4f89b37f5f4b44bda62d80eee926732446fe7e4b2adf500dfa903dcf1681c07d`，47,174 bytes）。依赖树/许可证边界通过，rustfmt 在三个新 Rust 文件失败；workspace check、tests、secret contracts、Clippy 和 release build 全部跳过，不能记为 SecretStore 通过。
- [Product validate #32127969715](https://github.com/oarw/cakify/actions/runs/32127969715)：`success`，commit `054aaf6b0ea939d41f455921ced714e4461ed5fa`，job `95682647629`，artifact `product-validation-32127969715`（ID `9321446137`，digest `sha256:1b4f6f03c4a6d0883f5c11d94b87a061d2d38db1e660d8401433d5d6fb6c795d`，6 files，2,386,299 bytes）。fmt、workspace check、全量 tests、storage/repository/provider/secret contracts、Clippy、release build 与上传全部成功；release 优化构建用时 4m03s。
- SecretStore artifact 已实际下载解包：`Cargo.lock` 归一化后与仓库一致；三份 migration、dependency tree 和 EXE 共 6 个文件，越界 GPL AI crate 0，artifact 文本高置信 secret 0；release EXE 为 5,722,624 bytes，SHA-256 `9C63E9A44A8C7AC78D03FDCDAC4B3F9922E9A2388A9122B97F75B226982F3E0D`，`NotSigned`（M7 前预期状态）。
- SecretStore 临时公开前复核覆盖 37 个可达 commit、524 个 objects、160 个历史路径，高置信 secret 与敏感文件名 0；GitHub secrets/variables/environments、Release、Issue、PR、fork、LFS 均为 0。本轮只运行 Product validate；确认 queued/in_progress 为 0 后恢复 PRIVATE 并复核 `isPrivate=true`。
- 最终 Provider artifact 已实际下载解包：`Cargo.lock` 与三份 migration 在 CRLF -> LF 归一化后和仓库逐字一致；dependency tree 的 GPL AI 业务 crate 命中 0，文本 secret 命中 0。EXE 为 5,722,624 bytes，SHA-256 `D3F27A091CD16EC63726534B0FB6F5442D77FFC481806010FDEF523C00453892`，`NotSigned`（M7 前预期状态）；依赖树 SHA-256 `F6A1ECD32544E40B5427B805BEF2CE782BBE2EA871C982F9B80A33ED029C384A`。
- Provider 临时公开前复核覆盖 34 个可达 commit、487 个 objects，高置信 secret 0 命中；Actions secrets/variables/environments、Release、Issue、PR、fork、LFS 文件均为 0，仓库仍无 LICENSE。本轮只运行 Product validate；恢复前 queued/in_progress 均为 0，随后已复核仓库为 PRIVATE。

### 公开前审计记录

- 最终 runtime 循环审计目标 HEAD：`a1f10429a7f48b5a7ca5968976676d6e2594554d`。
- 25 个可达 commit、145 个历史路径，高置信 secret、敏感文件名与 LFS pointer 0 命中。
- Actions/Dependabot/Codespaces secrets、variables、environments 均为 0。
- 原完整审计覆盖 10 个历史 run；三次 runtime 日志共 250,394 字符，高置信 secret 0 命中。
- 20 个 artifact 实际解包为 221 个文件、410,466,234 bytes，高置信 secret 与敏感文件名 0 命中；本地临时目录已删除。
- 两份旧 Flutter cache 只核对了 key、来源 workflow 和创建日志，未逐文件扫描；当前 workflow 不使用 cache。
- LFS、Release、Issue、PR、tag、fork 均为 0；仓库无 `LICENSE`。
- 结论：安全审计未发现阻止最终 runtime 临时公开的问题；运行、产物核对和恢复 PRIVATE 已闭环，详见 `docs/PUBLIC-ACTIONS-AUDIT.md`。

## 11. 进度日志

### 2026-08-16 至 2026-08-17：框架筛选

- 创建统一 benchmark core、fixture、四候选原型与 Windows Actions。
- 经用户明确授权完成临时 PUBLIC -> Actions -> PRIVATE 闭环。
- 最终 GPUI 三轮中位 ready `113.745 ms`、idle Working Set `42.016 MiB`；确定 GPUI 主线、Avalonia 回退。

### 2026-08-17：产品规划与归档

- 用户确认最终栈：GPUI UI + Rust Core + SQLite + Windows Credential Manager/DPAPI。
- 调研 Zed 成熟 AI 对话；确认 GPUI Apache-2.0 与 Zed AI 业务代码 GPL-3.0-or-later 的边界。
- 调研 `gpui-component`、GPUI IME 接口、SQLite WAL/backup、CredMan/DPAPI、MCP/rmcp、Job Object 和 Windows 打包。
- 用户进一步明确：整体功能向 Cherry Studio/RikkaHub 看齐，但不做知识库/RAG、远程操控等臃肿能力。
- 写入完整产品计划、架构、安全、路线图、来源和离线 HTML。
- 把全部 benchmark 时代源码、脚本、workflow 和报告迁入 `archi/framework-benchmark-2026-08/`，根目录准备进入产品 M0。
- 本轮未运行 Actions、未切换可见性、未在本机编译或测试。
- 完成本地静态核验；没有把静态检查写成 Actions 通过。

### 2026-08-17：M0 产品源码 bootstrap

- 创建 Rust `1.97.1` workspace 与首批 desktop/core/platform-windows crate，固定 GPUI 到已验证 Zed revision。
- 实现同进程 fake Core、有界 command/event、revision、公开协议测试源码和 GPUI 原生空窗口。
- 添加仅 `workflow_dispatch` 的产品 validate workflow，包含 fmt/check/test/clippy、release build、依赖树和 GPL 业务 crate 防线。
- 静态核对 `gpui-component` 调研 revision；其锁定 GPUI 比产品 pin 落后 88 个提交，M0 决定不引入并写 ADR。
- 修正归档文档迁移后的相对链接；Markdown/HTML 本地链接与敏感值模式检查无异常。
- 源码提交为 `07643ab45f1eaabfa6e44d5a57116496ad1c25d2`；本轮仍未运行 Actions、未生成 `Cargo.lock`、未切换 visibility。

### 2026-08-17：Product validate 公开前审计

- 按 2026 年 8 月规则完成 Git 历史、GitHub secrets/config、Actions 日志/artifact/cache、LFS、Release、Issue/PR、fork 与许可证检查。
- 未发现 secret 或用户数据；artifact 临时下载已清理，仓库保持 PRIVATE，无 queued/in_progress run。
- 记录旧 Flutter cache 未逐文件扫描、无 LICENSE 与公开副本不可收回等残余风险。
- 公开前审计阶段尚未切换 visibility 或 dispatch workflow，当时等待本次明确授权；后续闭环结果见下一节。

### 2026-08-17：Product validate 首轮

- 用户明确确认本次临时 PUBLIC -> 只运行 Product validate -> 核对产物 -> PRIVATE。
- 将仓库临时设为 PUBLIC，只触发 Product validate `32032509531`，目标 commit `9b6e71e07514c6f447de084a527d9a571b8368bd`。
- 首轮依赖树与许可证边界通过，但格式检查失败；check/test/clippy/release build 未执行，结论准确记录为 `failure`。
- artifact `product-validation-32032509531` 已取回：`Cargo.lock` SHA-256 `F6FF23586B01F6569C32CE3359F517E01F3C9E7591ED25798D52D4B2D7FC99C6`，依赖树 SHA-256 `4F424A9412718C56907ECE687A2342E55F491C9C6F7E4B3BFE3712E3276E729A`。
- 已按 runner 的 rustfmt 精确差异修复 4 个源码文件；等待提交、推送并重跑同一 workflow。
- 格式修复与锁文件已提交为 `2fe81c3a4b2e1b744c9c0d003577da5482a7e24b`；第二轮格式通过并进入 workspace check。
- 第二轮发现事件构造与 `send_event` 同时借用 revision 的 6 个 E0503 错误；由于 `send_event` 会统一覆盖 revision，已将构造值改为占位 `0`，等待重跑。
- 借用修复提交为 `a2d19ceb5647ce050a5012ed2b8fdc1d7f7db4ab`；第三轮 Product validate `32034202488` 全部通过并生成 release EXE。
- 核对最终 artifact 后，在 queued/in_progress 均为空时恢复仓库 PRIVATE；本轮只运行了 Product validate。
- 用户要求后续无需逐次提醒，自动 push 并自动完成 8 月受控 public -> 所需 Actions -> private 闭环；已写入持久规则，超出授权边界仍需确认。

### 2026-08-17：M0 Windows runtime smoke 源码

- 新增 `scripts/windows/runtime-smoke.ps1`：连续三轮启动 release EXE，以主窗口句柄判定 ready，采样整棵进程树并执行 80 MiB idle Working Set 与 0 默认子进程门。
- 每轮通过 `CloseMainWindow()` 发送 WM_CLOSE，要求 10 秒内以 exit code 0 退出，再按 executable path 检查残留；失败路径强制清理但保留失败结论。
- 新增仅手动触发、`contents: read` 的 `Windows runtime smoke` workflow，使用 locked release build，上传 JSON/JSONL、Markdown 摘要、日志、截图与 EXE。
- 本机只完成 PowerShell AST、workflow trigger/permission、路径和 diff 静态检查；未启动应用、未编译、未运行 smoke，等待 Actions 实证。
- 首轮 Actions 实际打开窗口三次并正常退出；artifact 明细每轮进程 Working Set 为 `39,505,920` bytes，但顶层聚合错误为 0，定位为 ordered dictionary 与 `Measure-Object -Property` 不兼容。
- 改用显式数值累加，移除对 GPUI 无效的 `WaitForInputIdle`；依据固定 GPUI revision 的 `TitlebarOptions` 给窗口设置 `Cakify` 标题，并将标题纳入第二轮 ready 门。
- 首轮截图真实捕获 Cakify 窗口，但 1120x720 默认尺寸超出 runner 的 1024x768 桌面并被左右/底部裁切；默认窗口调整为 960x680，等待第二轮截图复核完整框架。
- 第二轮修复后的指标与 JSONL 独立复算一致，三轮约 `35.7-37.3 MiB`、单进程、正常关闭；但 960x680 内容高度加系统标题栏后仍伸入任务栏区域约 27 px，拒绝仅凭绿色 workflow 关闭 M0。
- 默认内容高度进一步调整为 620 px，并在 smoke 中通过 `GetWindowRect` 与显示器 `WorkingArea` 对三轮完整可见性做硬门；下一轮 artifact 将记录窗口/工作区矩形并继续保留桌面截图。

### 2026-08-18：M0 完整可见硬门

- 中断恢复后核对实际状态：仓库为 PRIVATE、无活动 run、HEAD 与 `origin/main` 一致，三处修复未被部分暂存或提交。
- 对窗口高度、Win32 矩形门和进度记录完成 staged diff、PowerShell AST、workflow trigger/permission、whitespace 与敏感值静态检查；本机仍未运行 Cargo、EXE 或项目测试。
- 提交 `a1f10429a7f48b5a7ca5968976676d6e2594554d` 推送后完成最终公开前审计，只触发 Windows runtime smoke `32093988986`。
- 最终三轮窗口完整可见、空闲整树 Working Set `35.477-37.121 MiB`、默认子进程 0、正常退出且无残留；artifact JSON/JSONL、日志、哈希与截图已独立核验。
- 在 queued/in_progress 均为空时恢复仓库 PRIVATE；M0 正式关闭，当前进入 M1 SQLite/storage foundation。

### 2026-08-18：M1 SQLite/storage foundation 源码

- 依据架构与安全文档建立独立 `cakify-storage`，保持 Core 不依赖 SQLite；actor 使用容量 64 的 typed 同步队列，独占 writer connection。
- 固定官方当前 `rusqlite 0.40.2`，关闭默认 features、只启用 bundled SQLite；使用现有 `sha2 0.10.9` 对不可变 migration SQL 计算 SHA-256。
- 初始 schema 包含 migration history 与计划中的 12 张领域表，全部使用 STRICT、foreign key、JSON/状态/check 约束；只有 `provider_profiles.credential_ref` 与 credential 引用相关，不建立 FTS/embedding/vector 表。
- 新增空库初始化、v1 -> v2、重复打开、外键、checksum 篡改、未来 schema 拒绝和连接 PRAGMA contract 测试源码；Product validate 增加显式 storage contract step 并上传 migration SQL。
- 本机只做源码/SQL/workflow 静态审查；`Cargo.lock`、rustfmt、编译、测试、Clippy 和真实 SQLite 执行均待 Actions。

### 2026-08-18：M1 Product validate 首轮

- 对 commit `900bcde26847fc9910d50823469262bb4295ee9c` 完成公开前复核后，临时设为 PUBLIC 并只触发 Product validate `32097396883`。
- 依赖树与许可证边界成功；`cargo fmt --check` 给出 4 个源文件的纯格式差异，编译、测试、storage contract、Clippy 与 release build 未执行。
- 已取回 artifact 中 runner 生成的锁文件和 migration SQL；核对 `rusqlite 0.40.2` checksum、直接依赖边界、migration 规范化内容与文本 secret 均无异常。
- 已按 CI 精确差异修复格式并落盘锁文件；当时仓库仅因同一 workflow 修复保持临时 PUBLIC，随后第二轮成功并已恢复 PRIVATE。
- 用户补充检索规则：内置搜索端点仍会返回 HTTP 404；已写入 `AGENTS.md` 和当前产品决定，后续主动改用 MCP 工具或官方一手 API/仓库 CLI。
- 修复提交 `785241720db087ce38121b095ea5f192063ab2b4` 的第二轮 Product validate `32097907337` 全部通过：workspace tests 中 storage contract 5/5，通过专门重复执行 5/5；Clippy、release build 和 artifact upload 均成功。
- 上传日志确认 artifact 为 5 个文件、2,385,820 bytes，ID `9310763337`、digest `sha256:66b893168eadc5ead939c71c4059ca65f15cc6c9f2b2c38c2e3f49a2274ab118`；本机到 Azure Blob 连续三种客户端连接超时，完整解包核验明确留为待办。
- 确认 queued/in_progress 为 0 后立即恢复 PRIVATE，并复核 run completed/success 与 `isPrivate=true`；M1 转入领域 repository 与 crash recovery。

### 2026-08-18：M1 repository 与 crash recovery 源码

- 在 actor 内新增 typed conversation/list/page、message+parts 聚合事务、thread 装载、soft delete/purge、run 创建/单调更新和文本 checkpoint 命令；Core 不接触 SQLite connection。
- 新增 schema migration v3 `message_part_revisions`，以 revision 条件更新流式文本，拒绝乱序或同 revision 冲突增量；启动时在同一 writer transaction 将遗留 active run 原子标记为 `interrupted`，保留已有 part 文本并返回一次性恢复报告。
- 新增 provider snapshot JSON 的递归 credential-bearing key 防线；只允许 opaque/non-secret snapshot，解析或敏感键均在入库前拒绝。
- 新增 repository contract 源码：稳定 cursor 分页/软删除、消息与 parts 原子回滚、checkpoint 幂等与 stale 拒绝、run 单调状态/终态保护、重启恢复一次性与级联 purge。
- Product validate workflow 增加显式 `Run repository contract tests`；本机只做源码、SQL、workflow、依赖与敏感模式静态检查，尚未编译或执行测试。
- 对 commit `2f4b8688fc71ae727781baa7ac9306db48f9e2aa` 临时公开后只触发 Product validate `32100633458`；首轮在 rustfmt 门失败，后续步骤未运行，已按 runner 的完整差异修复 actor/lib/repository/test 纯格式。
- 格式修复提交 `621097cdc08a9ac5129eef2200c2b8c7628504e2` 的第二轮 Product validate `32100910742` 全部通过：repository contract 4/4 覆盖稳定分页/软删除、消息聚合回滚与顺序、checkpoint+一次性恢复、run 单调/终态；storage contract 5/5、Clippy 与 release build 同样通过。
- 上传日志确认 artifact 为 6 个文件、2,386,139 bytes，ID `9311722769`、digest `sha256:51d059c6089178c9afb56c858d594d744892071da2a1b28cd6edf24e96f144af`；新旧 artifact 均因本机到 Azure Blob 网段连接超时而未解包，待网络恢复补查。
- 确认 queued/in_progress 为 0 后立即恢复 PRIVATE，并复核 run completed/success 与 `isPrivate=true`；M1 转入 Provider profile 与 SecretStore 生命周期。

### 2026-08-18：M1 Provider profile 源码

- 新增 Provider profile create/get/list/update/disable/enable/delete typed actor API；更新使用 expected/next timestamp 做乐观并发保护，删除返回 opaque credential reference，供后续 SecretStore 在数据库提交后清理。
- profile create/update 可与 model cache 在同一 SQLite immediate transaction 提交；另提供独立原子 model cache replace，读取按 model ID 稳定排序，profile 列表按启用状态、显示名与 ID 稳定排序。
- endpoint 通过锁文件已有的 `url 2.5.8` 解析，限制为无内嵌认证、query 或 fragment 的绝对 HTTP(S) URL；metadata/capabilities 必须为有大小上限的 JSON object，并递归拒绝 credential-bearing key。
- `credential_ref` 只接受 `Cakify/provider/<opaque>/api-key` target 形式；SQLite schema 仍无 API key/token/blob 列，也没有向 storage API 增加 secret plaintext 参数。
- 新增四组 Provider contract 源码，覆盖 CRUD/禁用、stale write、敏感配置拒绝且不落盘、profile+cache 回滚、重复 model/reference、删除返回 reference 与外键级联；Product validate 新增显式 contract step。
- 本机仅编辑源码并完成 `git diff --check` 等静态检查；首轮 Product validate `32118633092` 在 rustfmt 门失败，编译与测试均未执行，已按 runner 精确差异修复。
- 修复提交 `9673349691062c80349f358d0ec8fc0a61364180` 的第二轮 Product validate `32119057930` 全部通过：workspace check、全量 tests、storage 5/5、repository 4/4、provider 4/4、Clippy、release build 和 artifact upload 均成功。
- 最终 artifact 已实际下载并逐项核对：6 个文件、2,386,150 bytes；锁文件与 migrations 归一化后和仓库一致，dependency tree 越界 crate 与文本 secret 均为 0；release EXE 为 5,722,624 bytes、SHA-256 `D3F27A091CD16EC63726534B0FB6F5442D77FFC481806010FDEF523C00453892`、未签名。
- 确认 queued/in_progress 为 0 后立即恢复 PRIVATE，并复核 run completed/success 与 `isPrivate=true`；M1 转入 SecretStore 两阶段生命周期与 Windows CredMan/DPAPI。

### 2026-08-18：M1 SecretStore 源码

- 在 Core 新增受限 `SecretId`、不实现明文调试/序列化/克隆的 `SecretInput`/`SecretValue`、同步 `SecretStore` port，以及 profile reference 提交和删除的补偿生命周期。
- 生命周期写入前读取旧值：首次 reference 提交失败删除新 secret；同 ID 更新提交失败恢复旧 secret，避免数据库旧引用指向空凭据；删除流程先提交 reference 删除，再清理 secret 并保留可重试错误。
- Windows 层实现 `CRED_TYPE_GENERIC` + `CRED_PERSIST_LOCAL_MACHINE` 的 Credential Manager adapter，限制 generic blob 为 2,560 bytes；读取后清零系统 blob，再用 `CredFree` 释放。
- 实现 DPAPI current-user adapter：只设置 `CRYPTPROTECT_UI_FORBIDDEN`，不设置 machine scope；secret ID 派生 entropy，密文带版本头并写入哈希文件名，通过同目录临时文件、`sync_all` 与 `MoveFileExW` 原子替换。
- 新增 Windows synthetic contract，覆盖 CredMan put/get/update/delete/idempotent cleanup，以及 DPAPI round-trip、磁盘不含测试明文、tamper failure 和删除；Product validate 增加两个显式 secret contract step。
- 首轮 Product validate `32127609188` 的依赖/许可证门通过但 rustfmt 失败；按 runner 完整差异修复后，第二轮 `32127969715` 实际执行并通过 workspace 编译、全量测试、CredMan/DPAPI synthetic contract、Clippy 和 release build。

### 2026-08-18：聊天垂直切片源码（待 Actions）

- 用户明确将下一步优先级改为聊天输入框、消息列表、真实模型请求、流式输出、Markdown、工具/MCP UI；live backup/restore 暂缓，不删除 M1 数据边界。
- 新增 Core provider trait、bounded run worker、24 ms/1 KiB delta 合并、取消和工具审批事件；失败/取消会撤销未完成的用户 turn，避免重试重复上下文。
- 新增 OpenAI-compatible blocking SSE adapter，先限制在请求线程内阻塞，避免 UI 线程网络等待；远程 HTTPS、loopback HTTP、无重定向和 Header 敏感标记作为代码契约。
- GPUI 界面从 M0 placeholder 改为多行输入、消息列表、assistant Markdown/code block、Provider 设置、停止/重试、MCP server 草稿与工具审批 UI。
- 本机没有 Rust/Cargo，未执行编译、测试、格式化或运行；下一步必须推送后仅运行 Product validate，按 CI 日志修复，再运行 Windows runtime smoke 做真实窗口/输入可见性核对。

## 12. 更新规则

每次有实质进展后：

1. 更新当前快照、完成项、阻塞与精确下一步。
2. Actions 必须记录 run URL/ID、commit SHA、artifact 和结论；未运行不得写通过。
3. 架构决定写文档/ADR，不只留在聊天。
4. 停止或更换模型/供应商前更新 `HANDOFF.md`。
5. 源码/文档完成后自动 commit/push；8 月 Actions 按持续授权自动闭环，新增风险、范围扩大、长期公开或进入新月份时重新确认规则。
