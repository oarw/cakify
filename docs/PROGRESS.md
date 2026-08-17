# Cakify 进度记录

> 本文件是项目状态的单一事实来源。
> 最后更新：2026-08-17（Asia/Shanghai）
> 当前阶段：M0 - 产品工作区与技术 spike
> 当前状态：M0_PRODUCT_VALIDATE_FIX_IN_PROGRESS

## 1. 当前快照

- 工作目录：`C:\Users\admin\Desktop\code\cakify`
- 分支：`main`，跟踪 `origin/main`。
- 本轮开始时 HEAD：`36742654d67b276ce964ecaea1b6a5d1a2c4c58f`。
- M0 产品源码提交：`07643ab45f1eaabfa6e44d5a57116496ad1c25d2`（`feat: bootstrap GPUI product workspace`）。
- GitHub remote：`https://github.com/oarw/cakify.git`。
- 仓库可见性：本次经用户明确授权临时设为 `PUBLIC`；闭环结束前必须恢复 `PRIVATE`。
- 最近成功 Actions：Validate `32017467536`、Benchmark `32017470781`，均针对 benchmark commit `40209896dca0009b747efc51ac885bed32b81f25`。
- 本轮 Actions：Product validate `32032509531` 已运行并因 `cargo fmt --check` 失败；依赖/许可证边界步骤通过，后续 check/test/clippy/release build 被跳过。
- 根 `.github/workflows/product-validate.yml`：已创建且只有 `workflow_dispatch`，push 不会自动触发。
- 产品源码：根 Cargo workspace 已建立，包含 desktop、core、platform-windows 三个首批成员。
- 产品构建状态：本机没有编译/测试；首轮 artifact 已生成并取回 `Cargo.lock` 与依赖树，但尚未取得 release EXE，正在按 CI 差异修复格式并重跑。
- 产品计划：Markdown 架构/安全/路线图/来源和离线 HTML 已写入。
- 本次公开前审计：已完成并写入 `docs/PUBLIC-ACTIONS-AUDIT.md`；目标 HEAD `b87789ce6c145cb8b1507ba077d8112d744dcdac`，等待明确授权。
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

## 6. 尚未完成

- [ ] 通过产品 validate Actions 生成并提交首个 `Cargo.lock`，取得 release EXE 与依赖树。
- [ ] 在 Actions/物理机验证 GPUI 空窗口启动、退出、默认子进程和基础内存；当前只有源码。
- [ ] 实现 SQLite initial schema/migration/storage actor。
- [ ] 实现 Credential Manager/DPAPI SecretStore。
- [ ] 实现 fake provider 聊天纵向切片。
- [ ] 实现真实 OpenAI-compatible provider。
- [ ] 实现 Agent/tool approval/Job Object。
- [ ] 实现 MCP stdio/Streamable HTTP。
- [ ] 实现轻量产品完善与发布流程。

## 7. 精确下一步

下一位执行者先闭合 M0，不再做框架泛泛选型：

1. 读取提交 `07643ab45f1eaabfa6e44d5a57116496ad1c25d2` 的 workspace、两个 ADR 和手动 validate workflow。
2. 提交 CI 精确格式修复与 Actions 生成的 `Cargo.lock`，推送后只重跑 Product validate。
3. 记录重跑 URL/ID、commit、结论与 artifact；若仍失败，按日志继续修复同一 workflow。
4. 成功后检查 release EXE 与依赖树，确认无 queued/in_progress，再恢复仓库为 `PRIVATE`。
5. 另行补齐默认进程树和窗口启动/退出验证；物理微软拼音/无障碍仍单独标记人工未验。
6. M0 通过后进入 M1：先建 SQLite storage actor/schema/migration，再实现 Credential Manager/DPAPI SecretStore。

详细验收见 `docs/ROADMAP.md` 的 M0。

## 8. 性能与质量门

以下仍是目标，未实际验证：

- 冷启动到可交互 P50 <= 400 ms、P95 <= 800 ms。
- 默认 idle 整树 Working Set <= 80 MiB。
- 默认子进程数 0。
- 10,000 消息使用真实虚拟列表。
- 流式更新按 16–33 ms 合并。
- 工具取消到整棵进程树退出 P95 <= 2 s。
- SQLite、日志、导出、artifact 中明文 secret 命中数 0。

本机未运行项目编译、测试、GUI、benchmark 或 package。任何门必须在后续 Actions/物理机实际运行后再标记通过。

## 9. 当前阻塞与风险

- `PRIVATE_ACTIONS_QUOTA`：2026 年 8 月私库 Actions 分钟已耗尽。
- `PUBLIC_CYCLE_ACTIVE`：本次 public -> Product validate -> private 已获明确授权且正在执行；仓库尚未恢复 private。
- `LICENSE_PENDING`：根仓库无 LICENSE，最终开源/闭源策略未定。
- `GPUI_PRE_1_0`：必须固定 commit，升级单独处理。
- `M0_CI_FAILED_FMT`：首轮只暴露出格式差异；check/test/clippy/release build 尚未执行，仍可能有 API 或编译问题。
- `CARGO_LOCK_UNCOMMITTED`：首个产品锁文件已由 Actions 生成并核对，正与格式修复一起提交。
- `DIRECT_GPUI_UI_WORK`：M0 已拒绝当前 `gpui-component` 依赖，聊天输入、Markdown 和组件需要直接实现与维护。
- `IME_ACCESSIBILITY_GAP`：真实微软拼音、日文 IME、DPI、多显示器、UI Automation 尚未验证。
- `PRODUCT_METRICS_UNRUN`：42 MiB/113 ms 是 benchmark 壳，不是产品性能。
- `SIGNING_PENDING`：签名证书、MSIX/安装器和更新通道未决定，安排在 M7。

## 10. Actions 事实记录

- [历史 benchmark Validate #32017467536](https://github.com/oarw/cakify/actions/runs/32017467536)：`success`，commit `40209896dca0009b747efc51ac885bed32b81f25`，artifact `cargo-lock-32017467536`。GitHub 曾因短暂复用 workflow 路径而改变其显示名；它不是产品 M0 run。
- [Benchmark candidates #32017470781](https://github.com/oarw/cakify/actions/runs/32017470781)：`success`，同一 commit，artifacts `benchmark-{gpui,avalonia,flutter,tauri}-32017470781`。
- [Product validate #32032509531](https://github.com/oarw/cakify/actions/runs/32032509531)：`failure`，commit `9b6e71e07514c6f447de084a527d9a571b8368bd`，artifact `product-validation-32032509531`（ID `9289483786`，含 `Cargo.lock`、`dependency-tree.txt`，无 EXE）。依赖/许可证边界通过，`cargo fmt --check` 失败，后续步骤被跳过。
- 本次 visibility 授权闭环仍在进行；仓库当前为 PUBLIC，必须在无 queued/in_progress 后恢复 PRIVATE。

### 公开前审计记录

- 审计目标 HEAD：`b87789ce6c145cb8b1507ba077d8112d744dcdac`。
- 18 个可达 commit、141 个历史路径，高置信 secret 与敏感文件名 0 命中。
- Actions/Dependabot/Codespaces secrets、variables、environments 均为 0。
- 10 个历史 run 的约 1,719,506 字符日志，高置信 secret 0 命中。
- 20 个 artifact 实际解包为 221 个文件、410,466,234 bytes，高置信 secret 与敏感文件名 0 命中；本地临时目录已删除。
- 两份旧 Flutter cache 只核对了 key、来源 workflow 和创建日志，未逐文件扫描；当前 workflow 不使用 cache。
- LFS、Release、Issue、PR、tag、fork 均为 0；仓库无 `LICENSE`。
- 结论：安全审计未发现阻止本次临时公开的问题，但仍需用户针对本次 visibility 切换明确授权。

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
- 本轮未切换 visibility、未 dispatch workflow，等待本次明确授权。

### 2026-08-17：Product validate 首轮

- 用户明确确认本次临时 PUBLIC -> 只运行 Product validate -> 核对产物 -> PRIVATE。
- 将仓库临时设为 PUBLIC，只触发 Product validate `32032509531`，目标 commit `9b6e71e07514c6f447de084a527d9a571b8368bd`。
- 首轮依赖树与许可证边界通过，但格式检查失败；check/test/clippy/release build 未执行，结论准确记录为 `failure`。
- artifact `product-validation-32032509531` 已取回：`Cargo.lock` SHA-256 `F6FF23586B01F6569C32CE3359F517E01F3C9E7591ED25798D52D4B2D7FC99C6`，依赖树 SHA-256 `4F424A9412718C56907ECE687A2342E55F491C9C6F7E4B3BFE3712E3276E729A`。
- 已按 runner 的 rustfmt 精确差异修复 4 个源码文件；等待提交、推送并重跑同一 workflow。

## 12. 更新规则

每次有实质进展后：

1. 更新当前快照、完成项、阻塞与精确下一步。
2. Actions 必须记录 run URL/ID、commit SHA、artifact 和结论；未运行不得写通过。
3. 架构决定写文档/ADR，不只留在聊天。
4. 停止或更换模型/供应商前更新 `HANDOFF.md`。
5. 源码/文档完成后自动 commit/push；Actions 与可见性授权规则不因自动推送而放宽。
