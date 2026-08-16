# Cakify

Windows-first AI Chat 客户端的四方案可复现 benchmark 工程。

当前阶段只搭建统一测试契约和共享 Rust benchmark core，尚未选择最终 UI 框架，也不会连接真实模型或 MCP 服务。候选方案：

1. GPUI + Rust
2. Avalonia + C# + Rust
3. Flutter + Rust
4. Tauri + Svelte + Rust

## 开始位置

- [架构调研](cakify-architecture-options.md)
- [进度记录](docs/PROGRESS.md)
- [交接文档](docs/HANDOFF.md)
- [共享协议](crates/bench-protocol/README.md)
- [benchmark core](crates/bench-core/README.md)
- [视觉规范](bench/visual-spec/README.md)

本机只编辑源码。编译、测试、基准和发布工作流初始为 `workflow_dispatch`，在用户明确授权并完成公开前安全检查前不会触发。

许可证尚未决定；当前没有 LICENSE，Rust 包均设置为 `publish = false`。
