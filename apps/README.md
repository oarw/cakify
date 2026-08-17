# UI shell contract

四个目录分别承载同一 benchmark 的 UI 壳。四个 Windows x64 release 壳已在 [`Benchmark candidates #32017470781`](https://github.com/oarw/cakify/actions/runs/32017470781) 编译、启动并各完成三轮采样；详细结论见 `docs/FRAMEWORK-BENCHMARK-REPORT.md`。

每个壳都必须：

1. 启动同一份 `cakify-bench-core.exe`，读取 stdout 的 `CAKIFY_READY` 和一次性 session token。
2. 只通过带 `x-cakify-session` 的 HTTP/SSE 访问 `/health`、`/fixture/manifest`、`/fixture/messages` 和 `/run/events`。
3. 支持 10,000 条消息虚拟列表、Markdown 基础块、工具时间线、附件缩略图、主题切换和中文输入。
4. 在 release x64 模式下由同一个 Actions workflow 采集 ready、整树内存、协议和退出指标；帧时间/GPU/暗色截图留给下一轮回归。

最终 commit 为 `40209896dca0009b747efc51ac885bed32b81f25`，artifact 为 `benchmark-{gpui,avalonia,flutter,tauri}-32017470781`。当前选择 GPUI 为下一阶段主线，Avalonia 为回退；其余壳保留为可复现基线。
