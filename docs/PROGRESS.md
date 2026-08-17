# Cakify 进度记录

> 本文件是项目状态的单一事实来源。
> 最后更新：2026-08-17（Asia/Shanghai）
> 当前阶段：M0 - 产品工作区与技术 spike
> 当前状态：M0_SOURCE_BOOTSTRAPPED_CI_PENDING

## 1. 当前快照

- 工作目录：`C:\Users\admin\Desktop\code\cakify`
- 分支：`main`，跟踪 `origin/main`。
- 本轮开始时 HEAD：`36742654d67b276ce964ecaea1b6a5d1a2c4c58f`。
- M0 产品源码提交：`07643ab45f1eaabfa6e44d5a57116496ad1c25d2`（`feat: bootstrap GPUI product workspace`）。
- GitHub remote：`https://github.com/oarw/cakify.git`。
- 仓库可见性：`PRIVATE`，已通过 `gh repo view` 复核。
- 最近成功 Actions：Validate `32017467536`、Benchmark `32017470781`，均针对 benchmark commit `40209896dca0009b747efc51ac885bed32b81f25`。
- 本轮 Actions：**未运行**；没有修改仓库可见性，也没有复用旧授权。
- 根 `.github/workflows/product-validate.yml`：已创建且只有 `workflow_dispatch`，push 不会自动触发。
- 产品源码：根 Cargo workspace 已建立，包含 desktop、core、platform-windows 三个首批成员。
- 产品构建状态：本机没有编译/测试；首个 `Cargo.lock`、release EXE 和依赖树 artifact 尚未由 Actions 生成。
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
2. 阅读 `docs/PUBLIC-ACTIONS-AUDIT.md`，取得用户对这一次 public -> Product validate -> private 的明确授权；旧授权不可复用。
3. Actions 运行后记录 run URL/ID、commit、结论与 artifact，下载 `Cargo.lock` 并提交；若失败，按日志修源码后重复本次闭环所需的授权流程。
4. 检查 release EXE、依赖树、默认进程树和窗口启动/退出；物理微软拼音/无障碍仍单独标记人工未验。
5. M0 通过后进入 M1：先建 SQLite storage actor/schema/migration，再实现 Credential Manager/DPAPI SecretStore。

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
- `PUBLIC_AUTH_REQUIRED`：未来每次 public/private 切换需要用户针对该次明确授权；旧授权已结束。
- `LICENSE_PENDING`：根仓库无 LICENSE，最终开源/闭源策略未定。
- `GPUI_PRE_1_0`：必须固定 commit，升级单独处理。
- `M0_CI_UNRUN`：产品 workspace、测试和 release EXE 尚未实际编译/运行，可能仍有 API 或格式问题。
- `CARGO_LOCK_PENDING`：首个产品锁文件必须由后续 Actions 生成、核对并提交。
- `DIRECT_GPUI_UI_WORK`：M0 已拒绝当前 `gpui-component` 依赖，聊天输入、Markdown 和组件需要直接实现与维护。
- `IME_ACCESSIBILITY_GAP`：真实微软拼音、日文 IME、DPI、多显示器、UI Automation 尚未验证。
- `PRODUCT_METRICS_UNRUN`：42 MiB/113 ms 是 benchmark 壳，不是产品性能。
- `SIGNING_PENDING`：签名证书、MSIX/安装器和更新通道未决定，安排在 M7。

## 10. Actions 事实记录

- [历史 benchmark Validate #32017467536](https://github.com/oarw/cakify/actions/runs/32017467536)：`success`，commit `40209896dca0009b747efc51ac885bed32b81f25`，artifact `cargo-lock-32017467536`。GitHub 曾因短暂复用 workflow 路径而改变其显示名；它不是产品 M0 run。
- [Benchmark candidates #32017470781](https://github.com/oarw/cakify/actions/runs/32017470781)：`success`，同一 commit，artifacts `benchmark-{gpui,avalonia,flutter,tauri}-32017470781`。
- 当前仓库已恢复 PRIVATE；上述 visibility 授权闭环完成，不能用于下一次运行。
- 本轮 M0 源码/ADR/工作流没有 Actions run 或 artifact，不能写成通过。

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

## 12. 更新规则

每次有实质进展后：

1. 更新当前快照、完成项、阻塞与精确下一步。
2. Actions 必须记录 run URL/ID、commit SHA、artifact 和结论；未运行不得写通过。
3. 架构决定写文档/ADR，不只留在聊天。
4. 停止或更换模型/供应商前更新 `HANDOFF.md`。
5. 源码/文档完成后自动 commit/push；Actions 与可见性授权规则不因自动推送而放宽。
