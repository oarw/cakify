# 四候选实现与运行计划

> 最后更新：2026-08-17（Asia/Shanghai）
> 当前仓库：`https://github.com/oarw/cakify`（PRIVATE）
> 共享骨架基线 commit：`4e605d730ca61f3461e517d34955eefba9aa8b92`
> 当前实现状态：四个首版 UI 壳与真实 benchmark workflow 已写入，尚未在 Actions 编译或运行。

## 为什么当前仍没有直接跑四候选

四个目录现在已经有首版可执行 UI，但仓库仍为 PRIVATE，且 2026 年 8 月私库 Actions 分钟耗尽。当前不运行不是性能结论：必须先完成本轮源码/远端安全复核，取得用户明确的临时公开授权，再在 Windows runner 上编译和测量。

## 已锁定的共同层

- `crates/bench-protocol`：版本化 JSON 类型和事件协议。
- `crates/bench-core`：localhost HTTP/SSE sidecar，随机 session token，CORS，确定性 10,000 消息分页，工具时间线和取消。
- `bench/fixtures/manifest.json`：唯一 fixture 输入，hash 为 `chat-10k-v1:deterministic-20260816`。
- `bench/visual-spec/tokens.json`：四个壳必须共用的窗口、字体、间距、亮暗主题 token。
- `bench/result-schema.v1.json`：真实 benchmark 结果格式。
- `scripts/windows/measure-process-tree.ps1`：主进程、Rust sidecar、WebView2 和子进程整树采样。

四个壳不得复制 fixture 生成逻辑，也不得连接真实模型、API key 或 MCP。

## 下一阶段实现顺序

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
- `scripts/windows/run-candidate-benchmark.ps1` 负责启动、ready、health、分页、取消、整树采样、截图尝试、清理和 `result.v1`。

## 真实 benchmark workflow 的最低要求

当前 `benchmark.yml` 的四个 matrix job 设计为全部做到：

1. Windows x64 release 构建，并记录 runner、编译器和框架版本。
2. 启动同一 `cakify-bench-core.exe`，读取 ready JSON 和 session token。
3. 完成窗口 ready、10,000 消息打开、60 秒 idle、滚动、30 秒流式、工具取消和进程清理场景。
4. 上传原始采样、截图、日志、portable/package 体积和 `result.v1` JSON。
5. 每个候选至少三次有效运行；任何失败都记录原因，不用最好一次掩盖失败。

## 运行门

当前仓库保持 PRIVATE，Actions 列表为空。下一阶段必须按以下顺序：

1. 先在私库完成源码实现和静态审阅。
2. 向用户展示 public 前历史、Secrets、LFS、Release、Issue、缓存和许可证审计结果。
3. 获得本次 `public -> Actions -> private` 的明确授权。
4. 临时公开，先跑 `Validate scaffold`，再跑真实四候选 matrix。
5. 确认没有 queued/in_progress run 后恢复 PRIVATE，并把 URL、run ID、commit、artifact 和结论写入 `docs/PROGRESS.md`。

## 明确禁止

- 不把 `scaffold_only` artifact 当成性能结果。
- 不在私库 Actions 额度耗尽时运行 workflow。
- 不为了赶进度删除 session token、进程树采样或统一 fixture 约束。
- 不在没有用户授权的情况下公开仓库，即使 `gh` 已登录。
