# UI shell contract

四个目录分别承载同一 benchmark 的 UI 壳。当前先提交契约和验收清单，避免在共享协议稳定前引入四套依赖树。

每个壳都必须：

1. 启动同一份 `cakify-bench-core.exe`，读取 stdout 的 `CAKIFY_READY` 和一次性 session token。
2. 只通过带 `x-cakify-session` 的 HTTP/SSE 访问 `/health`、`/fixture/manifest`、`/fixture/messages` 和 `/run/events`。
3. 支持 10,000 条消息虚拟列表、Markdown 基础块、工具时间线、附件缩略图、主题切换和中文输入。
4. 在 release x64 模式下由同一个 Actions workflow 采集启动、内存、帧时间、流式和退出指标。

实现完成后，应把本 README 的“待实现”状态改为对应 commit 和 artifact 名称。
