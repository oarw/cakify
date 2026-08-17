# Cakify 四候选 Windows 基准报告

> 报告日期：2026-08-17（Asia/Shanghai）  
> 测试 commit：`40209896dca0009b747efc51ac885bed32b81f25`  
> 结论状态：第一轮 Actions benchmark 完成；仓库已恢复为 `PRIVATE`

## 结论先行

本轮如果把“Windows 优先、启动快、常驻内存低”放在第一位，推荐 **GPUI + Rust 作为主线**。在相同 fixture、相同 Rust core、Windows x64 release 和三轮采样下，GPUI 的整棵进程树空闲 Working Set 中位数为 **42.016 MiB**，ready 中位数为 **113.745 ms**；其余候选分别是 125.102–326.496 MiB 和 554.475–1,642.973 ms 的量级。

这不是“GPUI 已经可以直接做完整产品”的结论。它仍是 pre-1.0，中文 IME、无障碍、Markdown 编辑体验和 API 稳定性必须在下一轮真实纵向切片中验证。**Avalonia + C# + Rust 是务实的回退/备选**：成熟度和原生输入风险更好，但运行时内存和发布目录明显更大。Flutter 适合把跨平台和 UI 组件成熟度置于首位；Tauri + Svelte 适合交付漂亮 UI 的速度，但不适合作为本项目的默认低内存方案。

建议决策：

1. 先用 GPUI 做真实纵向切片（Provider 设置、聊天流式输出、工具审批、MCP 适配、持久化）。
2. 在物理 Windows 机器上设置硬门：中文 IME/无障碍无阻断、10k 消息虚拟滚动稳定、冷启动到可交互中位数 < 500 ms、空闲整树 Working Set < 80 MiB。
3. GPUI 任一硬门失败时，保留共享 Rust core 和协议，切换到 Avalonia；不要因为 Tauri 包体最小就把它当作内存方案。

## 测试范围与公平性

- Validate：[`Validate shared core #32017467536`](https://github.com/oarw/cakify/actions/runs/32017467536)，结论 `success`。
- Benchmark：[`Benchmark candidates #32017470781`](https://github.com/oarw/cakify/actions/runs/32017470781)，结论 `success`。
- 所有结果来自 commit `40209896dca0009b747efc51ac885bed32b81f25`。
- runner：`windows-2025` / `win25-vs2026`，image version `20260810.198.2`，4 logical cores，约 16 GiB RAM。
- fixture：`chat-10k-v1:deterministic-20260816`，每个候选 10,000 条消息、相同视觉 token、相同 Rust benchmark core。
- 构建：Windows x64 `release`；每个候选 3 次运行；每次空闲采样 60 秒。
- 协议探针：health、首 200 条分页、取消请求、SSE `ready` 与 `cancelled` 均通过。
- 内存：统计应用根进程及其整棵子进程树，包含 WebView2、sidecar 和辅助进程。

`startup_ms` 只代表进程创建返回的时间，受 Windows 进程启动调用影响，不能当作用户可感知启动时间。本报告主要使用 `ready_ms`（应用启动到 core ready-file 可用）和整树内存。当前脚本还没有真实帧时间、GPU 内存、冷/热启动分离和安装器大小，因此这些指标不作推断。

## 结果摘要

| 候选 | 版本 | 发布目录 | ready 中位数 / P95 | 空闲 Working Set 中位数 | 空闲 Private Bytes 中位数 | 峰值 Working Set 中位数 | 进程数 |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| GPUI | gpui 0.2.2（Zed `b2d9c2e`） | 5.509 MiB | 113.745 / 125.014 ms | 42.016 MiB | 33.688 MiB | 44.770 MiB | 2 |
| Avalonia | 12.1.1 / .NET 8 | 128.782 MiB | 565.515 / 998.396 ms | 125.102 MiB | 52.359 MiB | 138.887 MiB | 3 |
| Flutter | 3.47.0 stable | 26.317 MiB | 1,642.973 / 2,047.155 ms | 126.180 MiB | 99.199 MiB | 130.504 MiB | 3 |
| Tauri | Tauri 2.11.5 / Svelte 5.56.9 | 2.687 MiB | 554.475 / 3,042.485 ms | 326.496 MiB | 113.785 MiB | 330.195 MiB | 8 |

这里的 P95 是每个候选只有 3 个样本时的最大值，不能伪装成大样本统计。发布目录大小也不是安装器大小：Avalonia 包含 .NET/Skia 文件，Tauri 使用系统 WebView2 runtime，后者没有重复计入磁盘目录，但其运行时子进程已经计入内存。

### 原始三轮样本

以下数值均为 MiB，ready 为毫秒；原始 JSON 保存在对应 artifact 中。

- GPUI：`ready 125.014 / 113.676 / 113.745`；Working Set `44.270 / 41.992 / 42.016`；Private `35.188 / 33.676 / 33.688`；三轮均 2 个进程，协议通过。
- Avalonia：`ready 998.396 / 538.643 / 565.515`；Working Set `126.332 / 125.102 / 124.949`；Private `52.359 / 56.090 / 52.191`；三轮均 3 个进程，协议通过。
- Flutter：`ready 2047.155 / 1642.973 / 1633.876`；Working Set `128.090 / 126.180 / 125.727`；Private `99.199 / 99.254 / 98.047`；三轮均 3 个进程，协议通过。
- Tauri：`ready 3042.485 / 554.475 / 542.957`；Working Set `326.848 / 326.496 / 323.875`；Private `113.844 / 113.785 / 112.207`；三轮均 8 个进程，协议通过。

## 如何解读

### GPUI + Rust

GPUI 在本轮四项运行时指标都占优：空闲整树 Working Set 约为 Avalonia/Flutter 的三分之一，约为 Tauri 的七分之一；ready 中位数也明显更短；发布目录约 5.5 MiB。它没有 WebView2 或托管 runtime 的进程组，符合本项目“高性能、低常驻负担”的初始目标。

代价是工程风险：GPUI 仍 pre-1.0，官方文档和 API 变化速度较快；当前壳的输入区仍是占位实现，截图中可以看到原生输入和窗口布局还需要打磨。必须先验证中文 IME、组合文本、焦点恢复、无障碍树、DPI 和多显示器，再把它当作产品基线。

### Avalonia + C# + Rust

Avalonia 的截图最接近传统桌面工具，XAML/控件生态和 TextBox 输入路径成熟，适合快速补齐设置页、弹窗、审批对话框和无障碍语义。它的运行时内存约 126 MiB、发布目录约 129 MiB，不满足“理论最低占用”，但在 Windows 桌面应用中是可接受的务实折中。

本轮 `window_ready` 探针三轮都为 false，然而截图确实捕获到可见窗口；这说明当前窗口句柄采集方式对 Avalonia 不可靠，不能据此判定窗口没有显示。下一轮应改为 UI 自动化/窗口标题探针，并在物理机验证 IME。

### Flutter + Rust

Flutter 的 UI 壳完整、输入框行为直观，发布目录约 26 MiB，跨平台迁移成本低。但 Windows Flutter engine 让整树 Working Set 约 128 MiB、Private Bytes 约 99 MiB，ready 中位数约 1.64 秒；对于一个强调瞬时启动和低占用的 Windows-first 客户端，它不是首选。

### Tauri + Svelte + Rust

Tauri 的发布目录最小（约 2.7 MiB），Svelte 也最容易快速做出接近 ChatGPT/Cherry Studio 的视觉细节。但这轮把 WebView2 进程组完整计入后，整树 Working Set 约 327 MiB、8 个进程，明显高于其他三项；ready 首轮还达到 3 秒。它证明了“包小”不等于“运行时内存小”，也回答了对 WebView2 的疑虑：在本项目目标下，Tauri 不能作为默认低内存答案。

## 截图与人工验证状态

本轮 artifact 中实际生成并检查的是 `light.png`。四个窗口都能被截图捕获，但有以下待修项：

- GPUI 截图出现窗口边界/桌面控制台背景，输入区明确标注 IME 仍待下一轮接入。
- Avalonia 截图可见窗口，但窗口句柄探针误报 false，且未做统一最大化/裁切。
- Flutter 截图视觉完整，部分轮次同样未被句柄探针识别。
- Tauri 截图视觉完整，但 WebView2 子进程树很大。

暗色主题、工具运行中、统一尺寸裁切、真实中文 IME、DPI/多显示器和无障碍自动化还没有完成，不能把当前截图当成最终 UI 评审结果。

## Actions 产物索引

Benchmark run `32017470781`：

- `benchmark-gpui-32017470781`（artifact id `9284344766`，2,827,595 bytes）
- `benchmark-tauri-32017470781`（artifact id `9284312348`，1,622,511 bytes）
- `benchmark-flutter-32017470781`（artifact id `9284267417`，12,065,123 bytes）
- `benchmark-avalonia-32017470781`（artifact id `9284201176`，39,451,268 bytes）

Validate run `32017467536` 生成 `cargo-lock-32017467536`（artifact id `9284086377`，3,819 bytes）。它已留在 Actions artifact 中，当前没有把生成锁文件机械复制进仓库，因此不会改变已测 commit 的可复现性记录。

## 失败迭代与修复记录

早期失败没有被隐藏，而是逐项修复后重新跑完整矩阵：

- GPUI：补充 `Range<usize>` 闭包类型。
- Avalonia：修正 `FuncDataTemplate` 命名空间和不支持的控件属性，并改用当前 placeholder API。
- Tauri：隔离 workspace、在 CI 生成 Windows 图标、补齐 `ReadyResponse` 序列化/反序列化和生命周期借用。
- 共享 core：修复取消请求先于第一次 SSE poll 时丢失 `ready` 事件的竞态。

最终 Validate 和 Benchmark 都在同一 commit 成功；早期失败属于实现/CI 反馈轮次，不应被误读为四个框架均不可用。

## 下一阶段方案与工作量

以下是基于现有壳和共享 core 的 AI 执行量粗估，不包含 Actions 排队、公有化审批和人工硬件验证时间：

| 方案 | 做到可用纵向切片 | 主要风险 | 建议 |
| --- | ---: | --- | --- |
| GPUI + Rust | 10–15 个 AI 工作日 | pre-1.0 API、IME、无障碍、Markdown 组件 | 主线，先过硬门 |
| Avalonia + C# + Rust | 8–12 个 AI 工作日 | 包体和内存较大、Rust/C# 边界 | GPUI 失败时回退 |
| Flutter + Rust | 9–14 个 AI 工作日 | Windows engine 常驻、Dart/Rust bridge | 跨平台优先时再选 |
| Tauri + Svelte + Rust | 6–9 个 AI 工作日 | WebView2 内存、渲染进程和版本差异 | 只在 UI 交付速度优先时选 |

纵向切片必须包括：Provider 配置与密钥存储接口、会话持久化、流式消息、工具审批状态机、MCP transport 抽象、取消/重试、错误恢复和最小日志。真实 API key、真实 MCP 和发布签名仍不能进入 benchmark fixture。

## 限制与下一道门

本轮是可复现的候选筛选，不是最终性能认证：

- 托管 runner 的 GPU、DPI 和窗口管理与用户机器不同。
- 当前只采集 ready、进程树内存和协议探针；没有帧时间、GPU 内存、CPU、磁盘冷缓存和安装器大小。
- 每个候选只有三轮，P95 等于样本最大值，统计置信度有限。
- `window_ready` 对部分框架存在探针误报，必须补 UI 自动化探针。
- 只有 light 截图；dark/tool-running/IME 尚未完成。

因此下一次 Actions 不应盲目重跑四矩阵，而应在 GPUI 纵向切片完成后只跑回归矩阵，并在用户授权且额度允许时增加真实 Windows 硬件 smoke。仓库当前已恢复 `PRIVATE`；本月私库分钟约束下，workflow 保持 `workflow_dispatch`，源码修复可以自动提交/推送，但不会在私库 push 上隐式消耗 Actions 分钟。
