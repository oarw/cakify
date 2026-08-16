# Cakify 跨供应商交接文档

> 用途：任何新的 AI 模型、供应商或工程师应在开始工作前完整阅读本文件。  
> 最后更新：2026-08-16（Asia/Shanghai）  
> 交接状态：私有远端已建立并推送共享骨架；四个真实 UI 尚未实现，等待实现和公开运行授权。

## 1. 五分钟上下文

Cakify 目标是一个 Windows-first 的高性能 AI Chat 客户端：

- 启动快、空闲内存低，避免 Electron 级常驻负担。
- UI/UX 学习 ChatGPT、Cherry Studio、RikkaHub，但不复制受许可证约束的源码。
- 首版需要基础聊天、Provider、function calling、工具审批和 MCP。
- Agent 理念学习 Pi：小型状态机、显式工具事件、流式事件、可取消，不捆绑整个 Node/Bun agent。
- 本机尽量不安装 Rust、Node、Python、Flutter、Visual Studio 等大批环境。
- 编译、测试、基准、打包、发布通过 GitHub Actions。

目前没有选定最终 UI 框架。第一步是比较四种同规格原型：

1. GPUI + Rust
2. Avalonia + C# + Rust
3. Flutter + Rust
4. Tauri + Svelte + Rust

## 2. 用户已确认的偏好

- 不死绑 WinUI 3，也不排斥 C#/C++；以美观、低内存和交付成本为准。
- TypeScript UI 很容易做漂亮，但不希望为此接受过高常驻内存。
- TypeScript/Python 可以作为生态兼容层，但不能默认成为常驻后端。
- 四个原型和测试源码全部由 AI 完成。
- 需要持续的进度与交接文档，避免更换模型后重新讨论或误操作。

## 3. 不可违反的约束

- 始终使用简体中文与用户交流。
- 本机原则上只编辑源码，不执行项目编译、测试、基准和打包。
- 不提交或打印真实 API Key、签名证书、私人端点、客户数据。
- 不自动修改 GitHub 仓库可见性。
- 2026 年 8 月私库 Actions 分钟已经耗尽。
- 本月运行 Actions 的顺序必须是：
  1. 完成公开前安全检查。
  2. 获得用户对本次切换的明确授权。
  3. 将仓库设为 public。
  4. 运行并确认 Actions、日志和 artifact。
  5. 确认没有排队或运行中的任务。
  6. 将仓库恢复为 private。
- 公开过的历史和第三方 fork 应视为无法收回。
- 进入 2026 年 9 月后先核实新额度，不要继续机械执行临时公开流程。

完整规则见：

- AGENTS.md
- .cursor/rules/github-actions-visibility.mdc
- .cursor/rules/project-continuity.mdc

## 4. 当前真实状态

- 工作区路径：C:\Users\admin\Desktop\code\cakify
- 本地分支：main，跟踪 origin/main；共享骨架基线 4e605d730ca61f3461e517d34955eefba9aa8b92，最近已推送文档提交 cd5a4e2；实际 HEAD 以 git rev-parse HEAD 为准。
- GitHub remote：`https://github.com/oarw/cakify.git`，仓库为 PRIVATE，默认分支 `main`。
- 已有 Cargo workspace：
  - `crates/bench-protocol`
  - `crates/bench-core`
- 已有统一 fixture、视觉 token、附件、结果 schema 和 Windows 采集脚本。
- `apps/gpui-bench`、`apps/avalonia-bench`、`apps/flutter-bench`、`apps/tauri-bench` 已建立契约 README，真实 UI 尚未实现。
- 已有 `.github/workflows/validate.yml` 与 `benchmark.yml`；只允许 `workflow_dispatch`，从未运行。
- 首次 push 后 gh run list 为空；没有 Action run ID、URL、Cargo.lock、构建产物、截图或 benchmark artifact。
- 本机没有 Cargo；没有执行编译、测试、GUI 启动或 benchmark。
- 已完成源码级 secret 扫描和静态解析，结果记录在 `docs/PUBLIC_ACTIONS_CHECKLIST.md`。
- 普通沙箱曾因 keyring/代理隔离误报 token 失效；在授权环境中 gh auth status 与 gh api user 已验证账号 oarw 有效。私有 cakify 远端已创建并核实。

如果接手时上述状态已经变化，先检查实际工作区并更新本节，不要假设本文永远正确。

## 5. 第一轮 benchmark 的正确边界

不要做四个完整 AI 客户端。只做一个统一视觉和统一数据的测试壳。

功能：

- 会话栏、消息区、输入区。
- 10,000 条消息虚拟列表。
- Markdown 基础块和代码块。
- 固定速率流式输出。
- 工具调用时间线。
- 图片附件。
- 亮/暗主题。
- 中文输入区域。

禁止：

- 真实模型请求。
- 真实 API Key。
- 真实 MCP Server。
- Node/Python 常驻 sidecar。
- 四个框架分别实现不同功能。
- 用 debug build 比较内存。

## 6. 建议仓库结构

~~~text
apps/
  gpui-bench/
  avalonia-bench/
  flutter-bench/
  tauri-bench/
crates/
  bench-core/
  bench-protocol/
bench/
  fixtures/
  assets/
  visual-spec/
  baselines/
scripts/
  windows/
  reports/
results/
  .gitkeep
.github/
  workflows/
    validate.yml
    benchmark.yml
docs/
  PROGRESS.md
  HANDOFF.md
~~~

## 7. 共享 benchmark core

为了公平，四个 UI 都连接同一个 Rust 子进程：

~~~text
cakify-bench-core.exe
  ├── health / ready
  ├── fixture manifest / paged messages
  ├── 30-second SSE stream / cancel
  ├── deterministic tool timeline
  └── metrics markers
~~~

当前实现使用 localhost HTTP + SSE：

- 端口默认使用 0 自动分配，通过 stdout 的 `CAKIFY_READY {json}` 或 ready 文件传递。
- 只绑定 `127.0.0.1`。
- 每次启动使用系统随机源生成 32 字节 session token。
- 所有 HTTP/SSE 请求必须带 `x-cakify-session`；四个 UI 从 ready JSON 读取，不把 token 写入源码。
- fixture 按索引确定性生成，10,000 条消息不以膨胀 JSON 提交。
- benchmark 结束必须关闭 core 并验证无残留进程。

后续产品是否改 named pipe 或 FFI 不在第一轮决定。

## 8. Actions 设计

现有两个 workflow 都只允许 `workflow_dispatch`，顶层权限为 `contents: read`，第三方 Action 使用完整 commit SHA：

- `validate.yml`
  - 校验 fixture manifest。
  - 执行 `cargo fmt --check`。
  - 执行 Rust workspace 契约测试。
  - 上传首次生成的 `Cargo.lock`。
- `benchmark.yml`
  - 当前只是 gpui/avalonia/flutter/tauri 四项 scaffold matrix。
  - 每项生成明确标记 `scaffold_only` 的 JSON artifact，不冒充真实性能结果。
  - 四个 UI 完成后再升级为 Windows x64 release 构建、启动和采集。

安全约束：

- 不使用真实 secrets。
- 不存在 push、PR、schedule 或 `pull_request_target` 触发器。
- runner 镜像和工具版本最终必须写入结果 JSON。
- 未经用户明确授权，不得运行任何 workflow 或修改 visibility。

## 9. 结果格式

每个 framework 至少上传：

~~~text
artifact/
  app/
  screenshots/
    light.png
    dark.png
    tool-running.png
  metrics/
    startup.json
    memory.json
    scroll.json
    streaming.json
    process-cleanup.json
    package-size.json
  logs/
    app.log
    core.log
    environment.txt
~~~

所有 JSON 必须包含：

- framework
- commit SHA
- runner image
- build mode
- run timestamp
- fixture version/hash
- measurement unit
- raw samples
- P50/P95/P99 或相应摘要

## 10. 选型评分

第一轮建议权重：

- 整棵进程树内存：30%
- 启动速度：20%
- 长列表和流式帧时间：20%
- UI 实现质量与代码复杂度：15%
- 中文输入/无障碍风险：10%
- CI 稳定性：5%

不要只选最低内存。必须同时记录实现同一视觉所需的代码量、依赖数、特殊 workaround 和失败率。

## 11. 已知候选特性

GPUI：

- 全 Rust、GPU 加速、不依赖 WebView。
- GPUI crate 为 Apache-2.0。
- 当前仍是 pre-1.0，API 与文档成熟度是主要风险。

Avalonia：

- C# + XAML，Skia 自绘，不依赖 WebView。
- 主框架 MIT。
- .NET runtime 与 Skia 使内存通常不是理论最低，但成熟度与美观能力较均衡。

Flutter：

- Dart + Flutter Engine + Impeller，不依赖 WebView。
- UI 和跨平台能力成熟。
- Engine、Dart runtime、字体和图形缓存需要计入。

Tauri：

- Svelte/TypeScript UI，Rust core。
- 发布版不需要 Node 常驻，但 Windows 仍使用 WebView2 进程组。
- 富 Markdown 和漂亮 UI 的交付速度通常最快。

## 12. 常见错误

- 只看主进程，不统计 WebView2 或 Rust sidecar。
- 使用不同数量或不同复杂度的消息。
- 一个框架做纯文本，另一个做完整 Markdown。
- 用首轮冷构建时间比较运行时性能。
- 把托管 runner 的软件渲染结果当作真实 GPU 绝对性能。
- 为了让某框架过关而隐藏辅助进程。
- 把附件转为多份 Base64。
- 每个 token 都触发整棵消息树重绘。
- 在决定框架之前实现真实 Provider、RAG 或 MCP。
- 看到首次失败就换框架，而没有区分框架问题与 CI 配置问题。

## 13. Definition of Done

第一轮 benchmark 完成必须满足：

- 四个 Windows x64 release 原型均可从 artifact 启动。
- 四个原型使用同一 fixture hash。
- 每个原型至少完成三轮有效测量。
- 结果包含原始样本，而不是只给最好数字。
- 有统一尺寸的亮/暗主题截图。
- 取消测试后没有残留 Rust core 或工具进程。
- 有 Actions run URL/ID 和 commit SHA。
- docs/PROGRESS.md 已更新。
- 输出一份包含推荐、反例和保留风险的最终报告。

## 14. 接手者的精确起点

当前请按以下顺序行动：

1. 读取 AGENTS.md、本文件、docs/PROGRESS.md、docs/PUBLIC_ACTIONS_CHECKLIST.md 和 docs/FRAMEWORK-IMPLEMENTATION-PLAN.md。
2. 实现四个真正可执行的 UI 壳；不要运行当前 scaffold_only matrix 伪装成 benchmark。
3. 更新 workflow，构建 GPUI、Avalonia、Flutter、Tauri 四个 Windows x64 release artifact，并采集统一指标。
4. 完成公开前安全审计并展示结果：当前 PRIVATE、单一初始 commit、无 secrets/variables/environments、无 LFS/Release/Issue/PR/cache/artifact。
5. 获得用户本次 public -> Actions -> private 授权后再改 visibility 和运行 workflow。
6. 记录 run URL/ID、commit、artifact、失败原因，并在确认无 queued/in_progress run 后恢复 PRIVATE。

## 15. 交接槽位

- 当前分支：main
- 当前 HEAD：执行 git rev-parse HEAD；共享骨架基线为 4e605d730ca61f3461e517d34955eefba9aa8b92，最近文档提交为 cd5a4e2
- GitHub remote：https://github.com/oarw/cakify.git
- 当前 visibility：PRIVATE
- 最近 Action run：N/A（首次 push 后为空）
- 最近成功 artifact：N/A
- 当前正在做：准备四个真实 UI 壳；当前 matrix 仍是 scaffold_only
- 已知失败：无 CI 失败记录；源码尚未编译，本机没有 Cargo
- 公开前审计：单一初始 commit；无 Secrets/Variables/Environments、Issues/PR、Releases、Artifacts、Caches、LFS；无分支保护
- 精确下一动作：实现四个最小壳并把 matrix 从 scaffold 升级为真实 release benchmark
- 需要用户决定：许可证、四壳实现完成后的本次 public -> Actions -> private 授权

每次停止工作或更换供应商前，必须更新以上槽位。
