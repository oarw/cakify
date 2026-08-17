# GPUI + Rust benchmark shell

状态：Windows x64 release 编译与三轮 benchmark 已通过，主线候选。

- 使用 Zed 官方固定提交 `b2d9c2e122fbc408d42276b4456243ba4f90f181` 的 `gpui` 与 `gpui_platform`。
- 原生窗口使用 `uniform_list` 渲染 10,000 行，统一视觉 token 由 benchmark 规范约束。
- `--core-path` 启动共享 `cakify-bench-core.exe`，读取 ready/session 与首个 200 条消息分页。
- 当前 composer 仍是统一视觉占位；产品纵向切片首先补原生 IME/组合文本/焦点/无障碍。
- 最终 run `32017470781`：ready 中位数 `113.745 ms`，整树 idle Working Set 中位数 `42.016 MiB`，artifact `benchmark-gpui-32017470781`。
