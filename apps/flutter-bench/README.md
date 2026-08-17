# Flutter + Rust benchmark shell

状态：Windows x64 release 编译与三轮 benchmark 已通过，当前不进入产品主线。

- GitHub Actions 固定 Flutter stable `3.47.0`，Windows runner 生成平台 runner 后构建 release。
- `ListView.builder` 负责 10,000 条消息虚拟滚动，Dart 通过 `dart:io` 访问共享 core。
- `--core-path` 与 `--core-ready-file` 由统一采集器传入；不引入 Node/Python 常驻进程。
- 最终 run `32017470781`：ready 中位数 `1,642.973 ms`，整树 idle Working Set 中位数 `126.180 MiB`。
