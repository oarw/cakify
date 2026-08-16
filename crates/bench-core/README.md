# Benchmark core

`cakify-bench-core` 是一个无真实网络依赖的 localhost 服务。它按 `cakify-bench-protocol` 生成固定 fixture，并提供分页、SSE 流式事件和取消接口。四个 UI 原型必须把它作为独立进程启动，记录整个进程树的资源，不得把数据生成逻辑复制到 UI 内。

启动参数：

```text
cakify-bench-core.exe [--port 0] [--ready-file <path>]
```

启动后会在 stdout 输出一行 `CAKIFY_READY {json}`，其中包含端口和该进程唯一的随机 `session_token`。默认绑定 `127.0.0.1`，端口 0 让系统选择空闲端口；所有接口都要求 `x-cakify-session` header。

第一轮不做 FFI、命名管道或真实 provider；这些属于 UI 方案确定后的第二阶段工程。
