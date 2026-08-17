# Avalonia + C# + Rust benchmark shell

状态：首个可执行壳已落地，等待 GitHub Actions 编译验证。

- Avalonia 12.1.1 + .NET 8 `net8.0-windows`，使用 `ListBox` 虚拟化 10,000 行。
- `--core-path` 启动共享 Rust core，`--core-ready-file` 可将 ready/session 信息交给 Actions 采集器。
- C# 只负责协议适配、主题、中文 `TextBox` 输入和取消探针，不复制 fixture 生成逻辑。
