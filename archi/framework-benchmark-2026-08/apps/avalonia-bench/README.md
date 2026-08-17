# Avalonia + C# + Rust benchmark shell

状态：Windows x64 release 编译与三轮 benchmark 已通过，GPUI 回退候选。

- Avalonia 12.1.1 + .NET 8 `net8.0-windows`，使用 `ListBox` 虚拟化 10,000 行。
- `--core-path` 启动共享 Rust core，`--core-ready-file` 可将 ready/session 信息交给 Actions 采集器。
- C# 只负责协议适配、主题、中文 `TextBox` 输入和取消探针，不复制 fixture 生成逻辑。
- 最终 run `32017470781`：ready 中位数 `565.515 ms`，整树 idle Working Set 中位数 `125.102 MiB`；当前 `MainWindowHandle` 探针会误报，截图确认窗口存在。
