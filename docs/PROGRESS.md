# Cakify 进度记录

> 本文件是项目状态的单一事实来源。
> 最后更新：2026-08-17（Asia/Shanghai）
> 当前阶段：M0 - 产品工作区与技术 spike
> 当前状态：PRODUCT_PLAN_COMPLETE_READY_TO_BOOTSTRAP

## 1. 当前快照

- 工作目录：`C:\Users\admin\Desktop\code\cakify`
- 分支：`main`，跟踪 `origin/main`。
- 本轮开始时 HEAD：`c28cce92afb4462b0475895e0514cc00709c4bb6`。
- GitHub remote：`https://github.com/oarw/cakify.git`。
- 仓库可见性：`PRIVATE`，已通过 `gh repo view` 复核。
- 最近成功 Actions：Validate `32017467536`、Benchmark `32017470781`，均针对 benchmark commit `40209896dca0009b747efc51ac885bed32b81f25`。
- 本轮 Actions：**未运行**；没有修改仓库可见性，也没有复用旧授权。
- 根 `.github/workflows`：产品 workflow 尚未创建；旧 benchmark workflow 已归档，因此本轮 push 不会触发项目 Actions。
- 产品源码：尚未 bootstrap；根目录暂时没有产品 `Cargo.toml`。
- 产品计划：Markdown 架构/安全/路线图/来源和离线 HTML 已写入。
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
- Apache-2.0 `gpui-component` 有 textarea、Markdown、virtual list 和主题，但其 GPUI Git 依赖未固定且兼容性未验证。M0 必须隔离 spike 后当轮 adopt/partial/reject。

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

## 6. 尚未开始

- [ ] 创建产品 Rust workspace 和首个 `Cargo.lock`。
- [ ] 创建 GPUI 产品窗口与 core command/event bridge。
- [ ] 完成 `gpui-component` 隔离 spike。
- [ ] 实现 SQLite initial schema/migration/storage actor。
- [ ] 实现 Credential Manager/DPAPI SecretStore。
- [ ] 实现 fake provider 聊天纵向切片。
- [ ] 实现真实 OpenAI-compatible provider。
- [ ] 实现 Agent/tool approval/Job Object。
- [ ] 实现 MCP stdio/Streamable HTTP。
- [ ] 实现轻量产品完善与发布流程。

## 7. 精确下一步

下一位执行者直接完成 M0，不再做框架泛泛选型：

1. 创建根 `rust-toolchain.toml`、产品 `Cargo.toml` 与 workspace dependency policy。
2. 创建 `apps/cakify-desktop`、`crates/cakify-core`、`crates/cakify-platform-windows` 最小可编译骨架；其余 crate 可在边界首次使用时加入。
3. 定义 `AppCommand`、`AppEvent`、request/run ID 和 bounded bridge，写 fake core loop 测试。
4. 创建最小 GPUI window，只连接 fake event，不接真实模型与密钥。
5. 建隔离 spike 验证 `gpui-component` textarea/微软拼音、Markdown streaming、virtual list、主题与体积，形成 ADR。
6. 建仅 `workflow_dispatch` 的 product validate workflow，固定第三方 Action SHA。
7. 源码写完后自动 commit/push；未经新的 visibility 授权不运行 workflow。

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
- `GPUI_COMPONENT_DECISION`：M0 spike 前不能把它当已选依赖。
- `IME_ACCESSIBILITY_GAP`：真实微软拼音、日文 IME、DPI、多显示器、UI Automation 尚未验证。
- `PRODUCT_METRICS_UNRUN`：42 MiB/113 ms 是 benchmark 壳，不是产品性能。
- `SIGNING_PENDING`：签名证书、MSIX/安装器和更新通道未决定，安排在 M7。

## 10. Actions 事实记录

- [Validate shared core #32017467536](https://github.com/oarw/cakify/actions/runs/32017467536)：`success`，commit `40209896dca0009b747efc51ac885bed32b81f25`，artifact `cargo-lock-32017467536`。
- [Benchmark candidates #32017470781](https://github.com/oarw/cakify/actions/runs/32017470781)：`success`，同一 commit，artifacts `benchmark-{gpui,avalonia,flutter,tauri}-32017470781`。
- 当前仓库已恢复 PRIVATE；上述 visibility 授权闭环完成，不能用于下一次运行。
- 本轮规划/归档没有 Actions run 或 artifact。

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

## 12. 更新规则

每次有实质进展后：

1. 更新当前快照、完成项、阻塞与精确下一步。
2. Actions 必须记录 run URL/ID、commit SHA、artifact 和结论；未运行不得写通过。
3. 架构决定写文档/ADR，不只留在聊天。
4. 停止或更换模型/供应商前更新 `HANDOFF.md`。
5. 源码/文档完成后自动 commit/push；Actions 与可见性授权规则不因自动推送而放宽。
