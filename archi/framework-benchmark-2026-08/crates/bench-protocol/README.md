# Shared benchmark protocol

`cakify-bench-protocol` 是四个 UI 壳和 `cakify-bench-core` 之间唯一需要共同理解的 Rust 数据契约。协议版本、fixture hash 和视觉规范版本必须在每次 benchmark artifact 中原样记录。

## HTTP endpoints

默认 core 只绑定 `127.0.0.1`，端口由系统分配（也可用 `--port` 指定）。ready JSON 同时返回一次性 `session_token`；core 允许跨 WebView origin 访问，但下列请求都必须带 `x-cakify-session` header：

- `GET /health`：返回协议版本和 fixture hash。
- `GET /fixture/manifest`：返回 `FixtureManifest`。
- `GET /fixture/messages?offset=0&limit=200`：返回确定性分页消息。
- `GET /run/events?run_id=<id>&scenario=stream`：SSE，先发工具时间线，再按 1 秒间隔发 30 个流式事件。
- `POST /run/cancel`：body 为 `{"run_id":"..."}`，取消对应 SSE 流。

UI 不应自行生成 benchmark 数据，也不应把真实 provider、API key 或 MCP server 接入第一轮测试。
