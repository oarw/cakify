# Cakify Desktop

M0 的 GPUI 原生窗口与 Core command/event bridge。

当前窗口只提供 fake conversation/draft 操作，用于验证：

- GPUI pinned revision `b2d9c2e122fbc408d42276b4456243ba4f90f181`。
- Core 在同一桌面进程内启动，不使用 benchmark HTTP sidecar。
- bounded command queue、异步 event bridge 和 revision 更新。
- Windows 本地数据目录边界。

真实 composer/IME、SQLite、Provider、Credential Manager/DPAPI 和 MCP 按 `docs/ROADMAP.md` 接入。
