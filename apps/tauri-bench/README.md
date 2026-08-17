# Tauri + Svelte + Rust benchmark shell

状态：Windows x64 release 编译与三轮 benchmark 已通过，因 WebView2 整树内存不进入产品主线。

- Tauri 2.11.5 + Svelte 5.56.9，Node 只在 Actions 构建阶段存在。
- Rust backend 只负责窗口、core 子进程和 ready/session；Svelte 使用固定行高的可视窗口渲染 10,000 条消息。
- `--core-path` 与 `--core-ready-file` 透传给共享 core；采集器会把 WebView2 与 Rust 子进程纳入整树。
- 最终 run `32017470781`：ready 中位数 `554.475 ms`，8 进程整树 idle Working Set 中位数 `326.496 MiB`；发布目录虽仅 `2.687 MiB`，不代表运行时内存低。
