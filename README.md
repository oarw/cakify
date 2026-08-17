# Cakify

Windows-first AI Chat 客户端的四方案可复现 benchmark 与产品纵向切片工程。

第一轮四候选 Windows Actions benchmark 已完成，当前决策为 **GPUI + Rust 主线、Avalonia + C# + Rust 回退**。benchmark fixture 不连接真实模型或 MCP 服务。已比较方案：

1. GPUI + Rust
2. Avalonia + C# + Rust
3. Flutter + Rust
4. Tauri + Svelte + Rust

## 开始位置

- [架构调研](cakify-architecture-options.md)
- [四候选最终报告](docs/FRAMEWORK-BENCHMARK-REPORT.md)
- [离线 HTML 报告](docs/FRAMEWORK-BENCHMARK-REPORT.html)
- [进度记录](docs/PROGRESS.md)
- [交接文档](docs/HANDOFF.md)
- [共享协议](crates/bench-protocol/README.md)
- [benchmark core](crates/bench-core/README.md)
- [视觉规范](bench/visual-spec/README.md)

本机只编辑源码。编译、测试、基准和发布工作流为 `workflow_dispatch`；2026 年 8 月私库 Actions 分钟约束下，未来每次运行仍需完成公开前检查和本次可见性切换授权。

许可证尚未决定；当前没有 LICENSE，Rust 包均设置为 `publish = false`。
