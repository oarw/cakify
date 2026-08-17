# Cakify

Cakify 是一个 Windows-first 的高性能原生 AI Chat 客户端。当前产品路线已经确定：

- **GPUI + Rust** 负责原生窗口、交互与渲染。
- **Rust Core** 在同一进程内负责 Provider、流式会话、Agent 状态机和 MCP。
- **SQLite** 保存会话、设置、工具记录和附件元数据。
- **Windows Credential Manager** 保存 API Key 与刷新令牌；DPAPI 仅作为用户范围的结构化秘密后备。
- 默认不常驻 WebView、Node、Python 或额外 sidecar；只有用户启用的 MCP/工具可以产生子进程。

第一轮四框架 benchmark 已结束并归档。实测 GPUI 原型的中位 ready 时间为 `113.745 ms`，整棵进程树 idle Working Set 为 `42.016 MiB`，因此进入产品主线；Avalonia 保留为框架风险回退，不再并行开发。

## 开始位置

- [产品总计划](docs/PRODUCT-PLAN.md)
- [离线 HTML 总览](docs/PRODUCT-PLAN.html)
- [技术架构](docs/ARCHITECTURE.md)
- [数据与安全](docs/SECURITY-AND-DATA.md)
- [路线图与验收门](docs/ROADMAP.md)
- [架构决定记录](docs/decisions/README.md)
- [Product validate 公开前安全审计](docs/PUBLIC-ACTIONS-AUDIT.md)
- [调研来源与许可证边界](docs/RESEARCH-SOURCES.md)
- [当前进度](docs/PROGRESS.md)
- [跨供应商交接](docs/HANDOFF.md)
- [历史 benchmark 归档](archi/framework-benchmark-2026-08/README.md)

## 当前阶段

现在处于 `M0 - 产品工作区与技术验证`。根目录已经建立产品 Cargo workspace、GPUI 空窗口、同进程 Core command/event bridge、Windows 数据目录边界和仅手动触发的 validate workflow。SQLite、Credential Manager/DPAPI 和真实聊天能力从 M1 起按路线图接入。

本机原则上只编辑源码。编译、测试、基准、打包和发布使用 GitHub Actions。2026 年 8 月私库 Actions 分钟已耗尽；任何本月 Actions 运行仍需逐次完成安全检查、取得用户明确授权、临时公开、核对结果并恢复私有。

许可证尚未决定，仓库当前没有 `LICENSE`。在许可证确定前，不复制 Zed 的 GPL Agent/AI 业务代码。
