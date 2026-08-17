# ADR 0001：产品运行时与 GPUI pin

- 状态：Accepted
- 日期：2026-08-17

## 背景

四候选 benchmark 已验证 GPUI 原型在 Windows runner 上具备当前最好的启动和空闲内存基线。产品同时要求不默认携带 WebView、Node、Python 或 HTTP sidecar，并且 Core 必须能脱离 UI 做确定性测试。

GPUI 仍处于 pre-1.0，Zed 仓库中的 API 会随提交变化。Zed 的 `agent`、`agent_ui`、`language_model`、`context_server` 等 AI 业务 crate 是 GPL-3.0-or-later，不能因为使用 GPUI 而被间接带入当前许可证未定的产品。

## 决定

- 产品 UI 使用 Zed commit `b2d9c2e122fbc408d42276b4456243ba4f90f181` 的 `gpui 0.2.2` 和同 revision 的 `gpui_platform`。
- Rust toolchain 固定为 `1.97.1`；依赖在根 workspace 统一声明，Git 依赖必须写完整 commit SHA。
- 桌面程序保持单进程。GPUI 位于主线程，框架无关的 Rust Core 位于独立服务线程；二者通过有界 `AppCommand`/`AppEvent` 通道通信。
- M0 不包含 SQLite、真实 Provider、密钥或工具执行，只验证 composition root 和 Core 边界。
- validate workflow 输出依赖树，并拒绝上述 Zed GPL AI 业务 crate 出现在产品依赖图中。

## 后果

- GPUI 升级必须单独提交，在 Actions 中重新跑编译、窗口、IME、无障碍和性能门。
- Core 不得依赖 GPUI、Win32 或具体存储实现，后续可以做 headless 测试，也保留框架硬门失败时的 Avalonia 回退能力。
- 产品不会复制 Zed 的 Agent UI 或业务实现；只独立实现同类用户行为。
- 首个 `Cargo.lock` 和 release EXE 只能在获得当次可见性授权后由 GitHub Actions 生成，目前不能写成已验证。

## 依据

- Validate shared core run `32017467536`：success，commit `40209896dca0009b747efc51ac885bed32b81f25`。
- Benchmark candidates run `32017470781`：success，同一 commit。
- GPUI benchmark 三轮中位 ready `113.745 ms`，整树 idle Working Set `42.016 MiB`。这些数字只代表 benchmark 壳，不代表产品门已通过。
