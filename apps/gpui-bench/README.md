# GPUI + Rust benchmark shell

状态：首个可执行壳已落地，等待 GitHub Actions 编译验证。

- 使用 Zed 官方固定提交 `b2d9c2e122fbc408d42276b4456243ba4f90f181` 的 `gpui` 与 `gpui_platform`。
- 原生窗口使用 `uniform_list` 渲染 10,000 行，统一视觉 token 由 benchmark 规范约束。
- `--core-path` 启动共享 `cakify-bench-core.exe`，读取 ready/session 与首个 200 条消息分页。
- 当前 composer 先提供统一视觉占位；原生 IME 输入控件会在首轮编译通过后单独补齐，避免把 GPUI API 风险与四框架基准混在一起。
