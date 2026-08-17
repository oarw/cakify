# Tauri + Svelte + Rust benchmark shell

状态：首个可执行壳已落地，等待 GitHub Actions 编译验证。

- Tauri 2.11.5 + Svelte 5.56.9，Node 只在 Actions 构建阶段存在。
- Rust backend 只负责窗口、core 子进程和 ready/session；Svelte 使用固定行高的可视窗口渲染 10,000 条消息。
- `--core-path` 与 `--core-ready-file` 透传给共享 core；采集器会把 WebView2 与 Rust 子进程纳入整树。
