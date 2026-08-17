# Cakify 跨供应商交接文档

> 用途：任何新的 AI 模型、供应商或工程师应在开始工作前完整阅读本文件。
> 最后更新：2026-08-17（Asia/Shanghai）
> 交接状态：第一轮四候选 Actions benchmark 已完成；GPUI 为主线、Avalonia 为回退；仓库已恢复 PRIVATE。

## 1. 五分钟上下文

Cakify 目标是一个 Windows-first 的高性能 AI Chat 客户端：

- 启动快、空闲内存低，避免 Electron 级常驻负担。
- UI/UX 学习 ChatGPT、Cherry Studio、RikkaHub，但不复制受许可证约束的源码。
- 首版需要基础聊天、Provider、function calling、工具审批和 MCP。
- Agent 理念学习 Pi：小型状态机、显式工具事件、流式事件、可取消，不捆绑整个 Node/Bun agent。
- 本机尽量不安装 Rust、Node、Python、Flutter、Visual Studio 等大批环境。
- 编译、测试、基准、打包、发布通过 GitHub Actions。

第一轮已比较四种同规格原型：

1. GPUI + Rust
2. Avalonia + C# + Rust
3. Flutter + Rust
4. Tauri + Svelte + Rust

当前决策是 **GPUI + Rust 主线，Avalonia + C# + Rust 回退**。这仍是进入产品纵向切片的工程决策，不是忽略 GPUI pre-1.0、IME 和无障碍风险的永久承诺。完整数据见 `docs/FRAMEWORK-BENCHMARK-REPORT.md`。

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
- 本地分支：`main`，跟踪 `origin/main`；最终 benchmark commit 为 `40209896dca0009b747efc51ac885bed32b81f25`；文档提交后的实际 HEAD 以 `git rev-parse HEAD` 为准。
- GitHub remote：`https://github.com/oarw/cakify.git`，仓库为 PRIVATE，默认分支 `main`。
- 已有 Cargo workspace：
  - `crates/bench-protocol`
  - `crates/bench-core`
- 已有统一 fixture、视觉 token、附件、结果 schema 和 Windows 采集脚本。
- `apps/gpui-bench`、`apps/avalonia-bench`、`apps/flutter-bench`、`apps/tauri-bench` 均已在 Windows x64 release runner 编译、启动并完成三轮采样。
- `.github/workflows/validate.yml` 与 `benchmark.yml` 只允许 `workflow_dispatch`。最终 Validate run `32017467536`、Benchmark run `32017470781` 均在 commit `4020989` 成功。
- 四个 benchmark artifact 已下载检查，包含 portable app、原始进程树日志、`result.json` 和 light 截图；Validate 提供 `cargo-lock-32017467536` artifact。
- 三轮中位数（ready / 整树 idle Working Set）：GPUI `113.745 ms / 42.016 MiB`；Avalonia `565.515 ms / 125.102 MiB`；Flutter `1,642.973 ms / 126.180 MiB`；Tauri `554.475 ms / 326.496 MiB`。
- 本机没有执行项目编译、测试、GUI 启动或 benchmark；编译/测试/采集全部来自 Actions。本机仅下载和解析 artifact。
- 已完成源码级 secret 扫描和静态解析，结果记录在 `docs/PUBLIC_ACTIONS_CHECKLIST.md`。
- 用户已明确授权并完成本轮临时公开；确认无 queued/in_progress 后已恢复 PRIVATE。本轮授权不能自动沿用到未来 visibility 切换。
- 普通沙箱曾因 keyring/代理隔离误报 token 失效；实际 `gh auth status` 与 `gh api user` 已验证账号 `oarw` 有效。

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
  - matrix 为 gpui/avalonia/flutter/tauri 四项 Windows x64 release 构建。
  - 每项启动共享 core，读取 ready-file，做 health/分页/SSE ready-cancelled 探针，采集整棵进程树并上传 `result.v1` 原始 artifact。
  - 最终 run `32017470781` 四个 job 均成功；每项有三轮、60 秒空闲整树采样。

安全约束：

- 不使用真实 secrets。
- 不存在 push、PR、schedule 或 `pull_request_target` 触发器。
- runner 镜像和工具版本最终必须写入结果 JSON。
- 未经用户明确授权，不得运行任何 workflow 或修改 visibility。
- 2026 年 8 月源码改动应在完成后自动 commit/push，但 workflow 继续手动 dispatch；不要给私库 push 增加自动触发而消耗已耗尽的私库分钟。

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

第一轮筛选的实际完成状态：

- [x] 四个 Windows x64 release 原型均从 artifact 成功启动。
- [x] 四个原型使用同一 fixture hash。
- [x] 每个原型完成三轮有效测量。
- [x] 结果包含原始样本，而不是只给最好数字。
- [x] 四个候选均有 light 截图并经人工检查。
- [ ] 统一尺寸的 dark/tool-running 截图尚未生成，留给 GPUI 纵向切片回归。
- [x] 取消协议通过，采集脚本没有记录残留 core/工具进程失败。
- [x] 已记录 Actions run URL/ID、commit SHA 和 artifact。
- [x] `docs/PROGRESS.md` 已更新。
- [x] 已输出包含推荐、反例和保留风险的 Markdown/HTML 最终报告。

## 14. 接手者的精确起点

当前请按以下顺序行动：

1. 读取 AGENTS.md、本文件、`docs/PROGRESS.md` 和 `docs/FRAMEWORK-BENCHMARK-REPORT.md`。
2. 不再做四框架泛泛调研或重复矩阵；从 GPUI 产品纵向切片开始。
3. 先确定共享产品 core 的最小协议：Provider 配置、会话存储、流式消息、工具审批、MCP transport、取消/重试和错误恢复。
4. 在 GPUI 接入真实输入控件与 UI 自动化探针，优先解决中文 IME/焦点/无障碍；保持 Avalonia 回退边界可用。
5. 源码完成后自动 commit/push；需要 Actions 时重新执行安全审计并取得新的 public/private 明确授权，不能复用本轮授权。

## 15. 交接槽位

- 当前分支：main
- 当前 HEAD：最终 benchmark commit 为 `40209896dca0009b747efc51ac885bed32b81f25`；本次文档提交后执行 `git rev-parse HEAD` 取得最新值
- GitHub remote：https://github.com/oarw/cakify.git
- 当前 visibility：PRIVATE
- 最近 Action run：Benchmark `32017470781`（success）；Validate `32017467536`（success）；两者均为 commit `4020989`
- 最近成功 artifact：`benchmark-{gpui,avalonia,flutter,tauri}-32017470781`、`cargo-lock-32017467536`
- 当前正在做：第一轮报告已完成，准备进入 GPUI 产品纵向切片
- 已知失败：早期格式、GPUI 类型、Avalonia API、Tauri workspace/icon/ready 类型和 core 取消竞态均已修复；最终 run 全绿。保留风险是 GPUI IME/无障碍、窗口探针和缺失帧/GPU 指标。
- 公开流程记录：公开前安全审计完成，用户明确授权，本轮临时 PUBLIC 运行并核对产物；确认活动任务为 0 后恢复 PRIVATE。
- 精确下一动作：设计并实现 GPUI 产品纵向切片的共享协议与真实输入控件，不在本机编译。
- 需要用户决定：许可证；未来每次 Actions visibility 切换仍需单独授权。

每次停止工作或更换供应商前，必须更新以上槽位。
