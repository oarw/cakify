# Flutter + Rust benchmark shell

状态：首个可执行壳已落地，等待 GitHub Actions 编译验证。

- GitHub Actions 固定 Flutter stable `3.47.0`，Windows runner 生成平台 runner 后构建 release。
- `ListView.builder` 负责 10,000 条消息虚拟滚动，Dart 通过 `dart:io` 访问共享 core。
- `--core-path` 与 `--core-ready-file` 由统一采集器传入；不引入 Node/Python 常驻进程。
