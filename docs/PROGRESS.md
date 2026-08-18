# Cakify 进度记录

> 本文件是项目状态的单一事实来源。
> 最后更新：2026-08-18（Asia/Shanghai）
> 当前阶段：M0 - 产品工作区与技术 spike
> 当前状态：M0_RUNTIME_SMOKE_WINDOW_GATE_FIX_IN_PROGRESS

## 1. 当前快照

- 工作目录：`C:\Users\admin\Desktop\code\cakify`
- 分支：`main`，跟踪 `origin/main`。
- 本轮开始时 HEAD：`36742654d67b276ce964ecaea1b6a5d1a2c4c58f`。
- M0 产品源码提交：`07643ab45f1eaabfa6e44d5a57116496ad1c25d2`（`feat: bootstrap GPUI product workspace`）。
- 已验证产品修复提交：`a2d19ceb5647ce050a5012ed2b8fdc1d7f7db4ab`。
- GitHub remote：`https://github.com/oarw/cakify.git`。
- 仓库可见性：`PRIVATE`；第二轮 runtime smoke 完成且确认无 queued/in_progress 后已恢复并复核。
- 最近成功 Actions：Windows runtime smoke `32038434473`，目标 commit `2c1125e10ff135656c078b69ffecf636d64fd728`。
- 本轮 Actions：runtime smoke 首轮 `32037554962` 的内存汇总证据无效；第二轮 `32038434473` 的性能/生命周期证据有效，但截图暴露任务栏遮挡，第三轮最终验收待当前修复提交后运行。
- Runtime smoke 首轮 `32037554962` 的窗口、单进程与 WM_CLOSE 退出通过，但汇总层把真实进程内存错误写成 0，因此内存证据无效。
- Runtime smoke 第二轮 `32038434473` 取得三轮非零内存、单进程、标题和正常退出证据；截图仍显示窗口底部约 27 px 被任务栏遮挡，暂不关闭 M0，正在增加窗口工作区硬门并缩短默认高度。
- 根 `.github/workflows/product-validate.yml`：已创建且只有 `workflow_dispatch`，push 不会自动触发。
- 根 `.github/workflows/windows-runtime-smoke.yml`：只有 `workflow_dispatch`；第二轮已取得有效性能数据，正在加入完整可见硬门后做最终重跑。
- 产品源码：根 Cargo workspace 已建立，包含 desktop、core、platform-windows 三个首批成员。
- 产品构建状态：本机没有编译/测试；Actions 已验证构建并实际运行 release EXE。第二轮三次窗口 ready 为 `160.087/122.374/128.065 ms`，空闲 Working Set 为 `37.289/35.668/35.684 MiB`，均为单进程并正常退出；窗口完整可见仍待修复后重跑，IME 保持独立物理机门。
- 产品计划：Markdown 架构/安全/路线图/来源和离线 HTML 已写入。
- 本次公开前审计与 visibility 闭环：已完成，见 `docs/PUBLIC-ACTIONS-AUDIT.md`；仓库已恢复 PRIVATE。
- 组件决定：M0 不引入 `gpui-component`；直接使用 GPUI primitives，见 ADR 0002。
- 历史 benchmark：完整移入 `archi/framework-benchmark-2026-08/`。
- 许可证：尚未选择，仓库仍没有 `LICENSE`。

## 2. 当前产品决定

- UI：GPUI + Rust。
- Core：同进程 Rust；UI 与核心通过 bounded typed command/event 通信。
- 数据：SQLite + `rusqlite` bundled；storage actor、WAL、migration。
- 密钥：Windows Credential Manager 主路径；DPAPI current-user 后备。
- Agent：小型 reducer/effect loop、显式 tool event、可取消。
- MCP：官方 `rmcp`，stdio + Streamable HTTP。
- 子进程：Windows Job Object 管理；默认启动子进程数为 0。
- 构建/测试/基准/打包/发布：GitHub Actions；本机只编辑和静态解析。
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

## 6. 尚未完成

- [ ] 在 Actions 闭合 GPUI 空窗口启动、退出、默认子进程、基础内存和完整可见门；第二轮仅剩任务栏遮挡未通过验收。
- [ ] 实现 SQLite initial schema/migration/storage actor。
- [ ] 实现 Credential Manager/DPAPI SecretStore。
- [ ] 实现 fake provider 聊天纵向切片。
- [ ] 实现真实 OpenAI-compatible provider。
- [ ] 实现 Agent/tool approval/Job Object。
- [ ] 实现 MCP stdio/Streamable HTTP。
- [ ] 实现轻量产品完善与发布流程。

## 7. 精确下一步

下一位执行者先闭合 M0，不再做框架泛泛选型：

1. 提交并推送 620 px 默认高度与窗口/工作区矩形硬门；确认远端 commit 与本地一致。
2. 按 8 月持续授权自动执行增量安全复核 -> public -> 只运行 `Windows runtime smoke` -> 核对 artifact 与完整截图 -> 无活动任务 -> private。
3. 若 smoke 失败，按真实日志修复并只重跑同一 workflow；未通过前不关闭 M0。
4. 物理微软拼音、日文 IME、DPI 与 UI Automation 保持独立人工门，不把无输入框的 M0 空壳写成 IME 通过。
5. runtime smoke 闭合后进入 M1：先建 SQLite storage actor/schema/migration，再实现 Credential Manager/DPAPI SecretStore。

详细验收见 `docs/ROADMAP.md` 的 M0。

## 8. 性能与质量门

以下是全产品目标；M0 第二轮已取得窗口壳层的部分证据，但当前窗口尺寸/硬门修复提交仍须最终重跑：

- 冷启动到可交互 P50 <= 400 ms、P95 <= 800 ms。
- 默认 idle 整树 Working Set <= 80 MiB。
- 默认子进程数 0。
- 10,000 消息使用真实虚拟列表。
- 流式更新按 16–33 ms 合并。
- 工具取消到整棵进程树退出 P95 <= 2 s。
- SQLite、日志、导出、artifact 中明文 secret 命中数 0。

第二轮产品窗口实测为：窗口句柄 ready `122.374-160.087 ms`、空闲整树 Working Set `35.668-37.289 MiB`、默认子进程数 0。ready 只代表 M0 窗口创建，不等于完整聊天输入可交互；最终源码仍以第三轮 artifact 为准。

本机未运行项目编译、测试、GUI、benchmark 或 package。任何门必须在后续 Actions/物理机实际运行后再标记通过。

## 9. 当前阻塞与风险

- `PRIVATE_ACTIONS_QUOTA`：2026 年 8 月私库 Actions 分钟已耗尽。
- `PUBLIC_CYCLE_AUTOMATION`：用户已持续授权 8 月后续受控闭环；每次仍须安全复核，公开不得闲置，新增风险或扩大范围时停止并请求确认。
- `LICENSE_PENDING`：根仓库无 LICENSE，最终开源/闭源策略未定。
- `GPUI_PRE_1_0`：必须固定 commit，升级单独处理。
- `M0_RUNTIME_SMOKE_WINDOW_OCCLUDED`：第二轮非零内存、标题、单进程和退出证据有效，但窗口底部被任务栏遮挡；必须用新增工作区硬门重跑后才能关闭 M0。
- `DIRECT_GPUI_UI_WORK`：M0 已拒绝当前 `gpui-component` 依赖，聊天输入、Markdown 和组件需要直接实现与维护。
- `IME_ACCESSIBILITY_GAP`：真实微软拼音、日文 IME、DPI、多显示器、UI Automation 尚未验证。
- `M0_FINAL_SOURCE_UNRUN`：第二轮已经是产品窗口性能，不再沿用 benchmark 代替；但 620 px 高度与完整可见硬门尚未经过 Actions，最终 M0 结论待第三轮。
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

### 公开前审计记录

- 审计目标 HEAD：`b87789ce6c145cb8b1507ba077d8112d744dcdac`。
- 18 个可达 commit、141 个历史路径，高置信 secret 与敏感文件名 0 命中。
- Actions/Dependabot/Codespaces secrets、variables、environments 均为 0。
- 10 个历史 run 的约 1,719,506 字符日志，高置信 secret 0 命中。
- 20 个 artifact 实际解包为 221 个文件、410,466,234 bytes，高置信 secret 与敏感文件名 0 命中；本地临时目录已删除。
- 两份旧 Flutter cache 只核对了 key、来源 workflow 和创建日志，未逐文件扫描；当前 workflow 不使用 cache。
- LFS、Release、Issue、PR、tag、fork 均为 0；仓库无 `LICENSE`。
- 结论：安全审计未发现阻止本次临时公开的问题；本次授权、运行、产物核对和恢复 PRIVATE 已闭环。

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

## 12. 更新规则

每次有实质进展后：

1. 更新当前快照、完成项、阻塞与精确下一步。
2. Actions 必须记录 run URL/ID、commit SHA、artifact 和结论；未运行不得写通过。
3. 架构决定写文档/ADR，不只留在聊天。
4. 停止或更换模型/供应商前更新 `HANDOFF.md`。
5. 源码/文档完成后自动 commit/push；8 月 Actions 按持续授权自动闭环，新增风险、范围扩大、长期公开或进入新月份时重新确认规则。
