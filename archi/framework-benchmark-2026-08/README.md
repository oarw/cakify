# 2026-08 四框架 benchmark 归档

状态：**已完成、只读归档**。

本目录保存 Cakify 第一轮 Windows UI 框架比较的完整工程。它不是产品实现，不能把其中的 benchmark sidecar、静态输入框或采集协议直接当作产品架构。

## 最终结果

- 测试 commit：`40209896dca0009b747efc51ac885bed32b81f25`
- [Validate shared core #32017467536](https://github.com/oarw/cakify/actions/runs/32017467536)：成功
- [Benchmark candidates #32017470781](https://github.com/oarw/cakify/actions/runs/32017470781)：成功
- Artifact：`cargo-lock-32017467536`、`benchmark-{gpui,avalonia,flutter,tauri}-32017470781`
- 结论：GPUI 主线，Avalonia 回退

三轮中位数：

- GPUI：ready `113.745 ms`，idle Working Set `42.016 MiB`
- Avalonia：ready `565.515 ms`，idle Working Set `125.102 MiB`
- Flutter：ready `1,642.973 ms`，idle Working Set `126.180 MiB`
- Tauri：ready `554.475 ms`，idle Working Set `326.496 MiB`

## 内容

- `apps/`：四个候选 UI 壳。
- `crates/`：共享 benchmark protocol/core。
- `bench/`：fixture、视觉 token 与结果 schema。
- `scripts/`：Windows 采集脚本。
- `workflows/`：当时实际使用的手动 Actions workflow；移出根 `.github/workflows` 后不会被 GitHub 自动发现。
- `docs/`：初步选型、实施计划、公开检查表与最终报告。
- `Cargo.toml`：归档工程原有 workspace 根；相对目录仍保持一致。

## 复现边界

原始 run 已在 GitHub-hosted `windows-2025` runner 上完成。当前仓库是 PRIVATE，2026 年 8 月私库分钟已耗尽；不要为了重跑归档实验而恢复根 workflow。确需复现时，应先重新审计依赖与 Actions 安全，并取得新的仓库可见性切换授权。

该 benchmark 没有证明物理机中文 IME、DPI、多显示器、无障碍、真实 GPU 帧时间或安装器体验。相关验证属于产品路线图，不应改写为已通过。
