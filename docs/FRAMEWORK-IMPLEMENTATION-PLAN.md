# 四候选实现与运行计划

> 最后更新：2026-08-17（Asia/Shanghai）
> 当前仓库：`https://github.com/oarw/cakify`（PRIVATE）
> 共享骨架基线 commit：`4e605d730ca61f3461e517d34955eefba9aa8b92`
> 最终 benchmark commit：`40209896dca0009b747efc51ac885bed32b81f25`
> 当前实现状态：四个 UI 壳均已在 Actions 编译并完成三轮采集；进入 GPUI 产品纵向切片阶段。

## 第一轮已经完成

本轮经用户明确授权完成临时公开、Validate、四候选 Benchmark、artifact 核对和恢复 private。最终 run：

- [`Validate shared core #32017467536`](https://github.com/oarw/cakify/actions/runs/32017467536)：success。
- [`Benchmark candidates #32017470781`](https://github.com/oarw/cakify/actions/runs/32017470781)：GPUI、Avalonia、Flutter、Tauri 四个 release job 全部 success。

第一轮结论为 **GPUI + Rust 主线，Avalonia + C# + Rust 回退**。详细指标、原始样本、限制和工作量见 `docs/FRAMEWORK-BENCHMARK-REPORT.md`。

## 已锁定的共同层

- `crates/bench-protocol`：版本化 JSON 类型和事件协议。
- `crates/bench-core`：localhost HTTP/SSE sidecar，随机 session token，CORS，确定性 10,000 消息分页，工具时间线和取消。
- `bench/fixtures/manifest.json`：唯一 fixture 输入，hash 为 `chat-10k-v1:deterministic-20260816`。
- `bench/visual-spec/tokens.json`：四个壳必须共用的窗口、字体、间距、亮暗主题 token。
- `bench/result-schema.v1.json`：真实 benchmark 结果格式。
- `scripts/windows/measure-process-tree.ps1`：主进程、Rust sidecar、WebView2 和子进程整树采样。

四个壳不得复制 fixture 生成逻辑，也不得连接真实模型、API key 或 MCP。

## 第一轮候选实现记录

### 1. GPUI + Rust

- 目标依赖：GPUI `0.2.2`。
- 当前官方仓库 commit：`b2d9c2e122fbc408d42276b4456243ba4f90f181`。
- 注意：Zed 的 `gpui_platform` 不是独立 crates.io 包，Windows 构建需要评估 pinned Git workspace 依赖；这是该候选的首要 CI 风险。
- 最小功能：原生窗口、侧栏、虚拟列表、输入框、主题切换、core SSE 客户端。

### 2. Avalonia + C# + Rust

- 当前 NuGet 稳定版本检查结果：`Avalonia`、`Avalonia.Desktop`、`Avalonia.Themes.Fluent` 均为 `12.1.1`。
- 使用 `net8.0-windows` x64 release，Rust core 作为独立 sidecar。
- 先用虚拟化 `ListBox` 完成 10,000 条消息，再评估 `ItemsRepeater` 是否必要。

### 3. Flutter + Rust

- CI 必须固定 Flutter/Dart 版本，并记录 Windows 渲染后端（Impeller/Skia）到 artifact metadata。
- 使用 `ListView.builder`，通过 `dart:io` HTTP/SSE 访问 core；不引入 Python/Node 常驻进程。
- Windows 平台目录由 Actions 生成或提交后固定，不能在本机安装 Flutter。

### 4. Tauri + Svelte + Rust

- 使用 Tauri v2 + Svelte，Node 只在 CI 构建阶段存在。
- Svelte 页面通过带 `x-cakify-session` 的 fetch/stream 访问 core；WebView2 进程组必须纳入整树内存。
- workflow 必须同时记录 Tauri 主进程、WebView2 和 core sidecar 的退出状态。

## 已落地的首版壳与 workflow

- GPUI：固定 Zed commit `b2d9c2e122fbc408d42276b4456243ba4f90f181`，原生窗口与 `uniform_list`。
- Avalonia：12.1.1 + .NET 8，虚拟 `ListBox` 与原生 `TextBox`。
- Flutter：3.47.0 stable，`ListView.builder` 与 `dart:io` core 客户端。
- Tauri：2.11.5 + Svelte 5.56.9，Rust 生命周期管理与固定行高虚拟窗口。
- 四个壳都接受 `--core-path` / `--core-ready-file`；token 只来自运行时 ready JSON。
- `scripts/windows/run-candidate-benchmark.ps1` 负责启动、ready、health、分页、SSE ready/cancelled、整树采样、截图尝试、清理和 `result.v1`。
- 最终三轮中位数（ready / idle Working Set）：GPUI `113.745 ms / 42.016 MiB`；Avalonia `565.515 ms / 125.102 MiB`；Flutter `1,642.973 ms / 126.180 MiB`；Tauri `554.475 ms / 326.496 MiB`。

## 真实 benchmark workflow 的最低要求

当前 `benchmark.yml` 的四个 matrix job 已做到：

1. Windows x64 release 构建，并记录 runner、编译器和框架版本。
2. 启动同一 `cakify-bench-core.exe`，读取 ready JSON 和 session token。
3. 完成 core ready、10,000 消息 fixture 分页、60 秒 idle、SSE ready/cancelled、工具取消和进程清理探针。
4. 上传原始进程树采样、light 截图、日志、portable 目录体积和 `result.v1` JSON。
5. 每个候选至少三次有效运行；任何失败都记录原因，不用最好一次掩盖失败。

当前没有测到真实滚动帧时间、30 秒渲染期间 CPU/GPU、dark/tool-running 截图和安装器大小；这些不能写成已完成。

## 下一阶段：GPUI 产品纵向切片

1. 固化产品 core 的边界：Provider 配置、会话/消息存储、流式事件、工具审批、MCP transport、取消/重试和错误恢复。
2. 在 GPUI 实现真实文本输入，先过中文 IME、组合文本、焦点恢复和无障碍硬门。
3. 完成一条可见工作流：创建会话 -> 选择 Provider -> 流式回复 -> 工具请求 -> 用户审批 -> 工具输出 -> 取消/重试。
4. 密钥存储只定义 Windows Credential Manager/DPAPI 接口；测试使用假 Provider，不提交真实密钥。
5. 增加 UI 自动化窗口探针、dark/tool-running 截图、帧时间和 CPU/GPU 采样；物理机 smoke 与托管 runner 结果分开记录。
6. 若 GPUI 无法满足 IME/无障碍或稳定性硬门，保持 Rust core/协议不变，把 UI 切换到 Avalonia。

## 后续 Actions 运行门

当前仓库为 PRIVATE，2026 年 8 月私库分钟已耗尽。源码改动完成后应自动 commit/push，但 workflow 保持 `workflow_dispatch`。未来需要 Actions 时仍必须重新：

1. 在私库完成源码实现和公开前安全审阅。
2. 展示历史、Secrets、LFS、Release、Issue/PR、cache/artifact 与许可证状态。
3. 获得该次 `public -> Actions -> private` 明确授权。
4. 临时公开，运行必要的最小 workflow；不要机械重跑已完成的四候选。
5. 核对日志/artifact，确认无 queued/in_progress 后恢复 PRIVATE 并记录闭环。

## 明确禁止

- 不把 `scaffold_only` artifact 当成性能结果。
- 不在私库 Actions 额度耗尽时运行 workflow。
- 不为了赶进度删除 session token、进程树采样或统一 fixture 约束。
- 不在没有用户授权的情况下公开仓库，即使 `gh` 已登录。
