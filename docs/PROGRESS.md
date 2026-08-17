# Cakify Benchmark 进度记录

> 本文件是项目状态的单一事实来源。
> 最后更新：2026-08-17（Asia/Shanghai）
> 当前阶段：Phase 3 - 选型决策与产品纵向切片
> 当前状态：BENCHMARK_COMPLETE_GPUI_PRIMARY

## 1. 当前快照

- 工作目录：C:\Users\admin\Desktop\code\cakify
- Git 状态：本地 `main` 跟踪 `origin/main`；最终 benchmark commit 为 `40209896dca0009b747efc51ac885bed32b81f25`；文档提交后的实际 HEAD 以 `git rev-parse HEAD` 为准。
- GitHub 远端：`https://github.com/oarw/cakify.git`。
- 原型源码：共享 Cargo workspace、benchmark protocol/core、fixture、视觉 token、结果 schema、采集脚本，以及 GPUI、Avalonia、Flutter、Tauri 四个首版可执行 UI 壳均已写入。
- GitHub Actions：`validate.yml` 与 `benchmark.yml` 均仅允许 `workflow_dispatch`。最终 [`Validate shared core #32017467536`](https://github.com/oarw/cakify/actions/runs/32017467536) 和 [`Benchmark candidates #32017470781`](https://github.com/oarw/cakify/actions/runs/32017470781) 在同一 commit 成功。
- 仓库可见性：本轮经用户明确授权临时切为 PUBLIC，完成运行与 artifact 核对、确认活动任务为 0 后，已恢复为 `PRIVATE`。
- 远端审计：公开前已核实历史、Secrets、Variables、Environments、LFS、Release、Issue/PR、Packages、Artifacts/Caches、许可证和分支保护；当前仍没有 LICENSE，临时公开不代表开源授权。
- 当前目标：以 GPUI + Rust 为主线实现产品纵向切片；保留 Avalonia + C# + Rust 作为 IME、无障碍或 API 稳定性不达标时的回退。
- 执行方式：源码由 AI 完成；本机只做源码编辑和静态解析；构建、测试、基准、打包通过 GitHub Actions。
- Actions 验证：Rust 格式检查/契约测试成功；四个 Windows x64 release job 均成功；每个候选三轮、60 秒空闲采样、同一 fixture 与协议探针通过，artifact 已下载并检查结果 JSON/light 截图。
- 本机验证边界：本机没有执行项目编译、测试、GUI 启动或 benchmark；只解析 Actions 产物和文档。物理机中文 IME、DPI、多显示器、无障碍、帧时间与 GPU 指标仍未验证。

## 2. 候选技术栈

1. GPUI + Rust
2. Avalonia + C# + Rust
3. Flutter + Rust
4. Tauri + Svelte + Rust

第一轮决策为 **GPUI 主线、Avalonia 回退**。WinUI 3、C++/WinRT、Slint 不进入下一轮，除非 GPUI 与 Avalonia 都触发硬性阻断。

## 3. 第一轮统一功能范围

每个原型只实现同一组基准功能，不实现完整 AI 客户端：

- 单窗口、左侧会话栏、主聊天区和输入区。
- 10,000 条固定消息的虚拟滚动。
- Markdown 基础块：标题、列表、引用、表格、代码块。
- 30 秒固定速率的流式文本。
- 工具调用时间线：提出、批准、运行、增量输出、完成、失败、取消。
- 固定图片附件缩略图。
- 亮色与暗色主题。
- 中文文本输入；真实 IME 体验作为人工 artifact smoke 的保留项。
- 不访问真实模型，不使用真实 API Key，不连接真实 MCP 服务。

## 4. 公平性约束

- 全部使用 Windows x64 release 构建。
- 四个 UI 使用相同文字、图片、字体尺寸、布局尺寸和事件序列。
- 四个 UI 都连接同一个 Rust benchmark core 进程，避免只给某些框架内联 Rust 的不公平优势。
- Rust core 只提供固定 fixture、流式事件和工具事件，不进行真实网络请求。
- 记录主进程与整棵进程树；WebView2、Rust sidecar 和辅助进程都计入。
- 同时记录 Working Set、Private Bytes、Commit 和 GPU 内存；不能只抄任务管理器的一列。
- 不比较 debug build。
- UI 美观度使用同一视觉规范和同尺寸截图评估。
- 托管 runner 的 GPU 结果只用于相对回归，不作为真实硬件绝对结论。

## 5. 计划采集指标

- 冷启动 P50 / P95。
- 热启动 P50 / P95。
- 启动到窗口可交互的时间。
- 空闲 60 秒后的整棵进程树 Working Set。
- 峰值 Working Set、Private Bytes 和 Commit。
- 10,000 条消息首次打开耗时。
- 自动滚动期间 P50 / P95 / P99 帧时间。
- 流式输出期间 CPU、内存增长和 UI 长任务。
- 工具取消到全部子进程退出的时间。
- 安装包、解压目录和首次启动缓存体积。
- 首次冷构建、缓存命中构建和测试耗时。

## 6. 已完成

- [x] 完成 Windows 高性能 AI Chat 客户端初步架构调研。
- [x] 生成 Markdown 调研文档。
- [x] 生成离线 HTML 调研文档并校验 18 个章节与目录锚点。
- [x] 调研 Tauri、Flutter、WinUI 3、C++/WinRT、GPUI、Avalonia、Slint、egui 和 iced。
- [x] 确认第一轮比较 GPUI、Avalonia、Flutter、Tauri。
- [x] 确认 AI Core 与 UI 壳解耦，TypeScript/Python 只作为可选扩展运行时。
- [x] 写入 2026 年 8 月 GitHub Actions 私库额度与可见性临时规则。
- [x] 建立进度记录、公开检查表与跨供应商交接机制。
- [x] 初始化本地 Git `main` 仓库并配置 noreply 作者。
- [x] 创建初始 commit 4e605d730ca61f3461e517d34955eefba9aa8b92，创建并推送 oarw/cakify 私有远端。
- [x] 建立 Cargo workspace、`bench-protocol` 与 `bench-core`。
- [x] 固化 10,000 消息 fixture manifest、视觉 token、附件和结果 JSON schema。
- [x] Rust core 实现 localhost HTTP/SSE、确定性分页、30 秒流式事件、工具时间线和取消。
- [x] Rust core 每次启动生成随机会话令牌，所有接口校验 `x-cakify-session`。
- [x] 建立四个客户端目录和统一实现契约。
- [x] 实现 GPUI + Rust 原生窗口、`uniform_list`、主题与 core ready/page 接入。
- [x] 实现 Avalonia + C# + Rust 虚拟 `ListBox`、主题、中文输入和 core 接入。
- [x] 实现 Flutter + Rust `ListView.builder`、主题、中文输入和 core 接入。
- [x] 实现 Tauri + Svelte + Rust 虚拟窗口、主题、中文输入和 core 生命周期。
- [x] 建立进程树采样、fixture 校验和 scaffold artifact 脚本。
- [x] 创建仅 `workflow_dispatch` 的 validate 与四候选四套 release benchmark matrix。
- [x] 建立统一 ready-file、health、分页、SSE ready/cancelled 探针和 `result.v1` 采集脚本。
- [x] 通过 GitHub 官方 API 核实 checkout/upload-artifact 的完整 commit SHA。
- [x] 完成本地 JSON、PowerShell、workflow 触发器和 secret 模式静态检查。
- [x] 经用户明确授权完成一次 `PRIVATE -> PUBLIC -> Actions -> PRIVATE` 闭环。
- [x] Validate run `32017467536` 成功：fixture、Rust 格式检查、workspace 契约测试与 lockfile artifact 均成功。
- [x] Benchmark run `32017470781` 成功：GPUI、Avalonia、Flutter、Tauri 四个 release job 全部成功。
- [x] 四个候选各完成三轮 60 秒整树采样，fixture、分页和 SSE 取消协议探针全部通过。
- [x] 下载并检查四个 benchmark artifact 的 `result.json`、进程树日志、应用产物和 light 截图。
- [x] 输出 `docs/FRAMEWORK-BENCHMARK-REPORT.md` 与离线 HTML 报告，形成 GPUI 主线、Avalonia 回退决策。

## 7. 状态清单

- [x] 配置 Git 作者身份并创建初始 commit。
- [x] 配置并核实 oarw/cakify 私有 GitHub 远端。
- [ ] 选择公开仓库许可证；当前没有 LICENSE，Rust 包为 `publish = false`。
- [x] 在 Actions 中运行 Rust 格式化与契约测试。
- [x] Validate 生成 `cargo-lock-32017467536` artifact；为保持已测 commit 不变，暂未提交生成锁文件。
- [x] 在 Actions 编译并验证 GPUI UI 原型。
- [x] 在 Actions 编译并验证 Avalonia UI 原型。
- [x] 在 Actions 编译并验证 Flutter UI 原型。
- [x] 在 Actions 编译并验证 Tauri + Svelte UI 原型。
- [x] 运行四套 x64 release 构建与真实采集矩阵。
- [x] 生成四套 portable app、light 截图、进程树日志和 `result.v1` JSON。
- [x] 每个候选完成三轮有效基准。
- [x] 输出第一轮技术选型报告（Markdown + HTML）。
- [ ] 在物理 Windows 硬件验证中文 IME、DPI、多显示器、无障碍和 GPU/帧时间。
- [ ] 为 GPUI 实现产品纵向切片；达到硬门后再确认最终产品框架。

## 8. 精确下一步

下一位执行者从这里继续，不要重新做框架泛泛调研，也不要重复跑四候选：

1. 阅读 `docs/FRAMEWORK-BENCHMARK-REPORT.md`，以 GPUI + Rust 为主线、Avalonia 为回退。
2. 先设计并实现 GPUI 产品纵向切片：Provider 设置、会话持久化、流式聊天、工具审批、MCP transport、取消/重试与错误恢复。
3. 保持 UI 与共享 Rust core/协议分离；真实密钥使用 Windows Credential Manager/DPAPI 抽象，不写进仓库或 fixture。
4. 源码修改完成后自动提交并推送；2026 年 8 月 Actions 仍只允许手动 dispatch，必须重新完成安全复核并获得本次 visibility 授权。
5. 下一轮 Actions 增加 UI 自动化窗口探针、dark/tool-running 截图、帧时间和 GPUI IME smoke；物理机验证不允许用托管 runner 结论替代。

## 9. 预计节奏

- 第一轮四候选粗测与架构决策：已完成。
- GPUI 可用纵向切片：粗估 10–15 个 AI 工作日，不含 Actions 等待和人工硬件验证。
- Avalonia 回退纵向切片：粗估 8–12 个 AI 工作日；仅在 GPUI 硬门失败后启动。

## 10. 当前阻塞与授权门

- GPUI_PRODUCT_RISK：GPUI pre-1.0；真实中文 IME、无障碍、Markdown 编辑、DPI 和多显示器尚未验证。
- METRICS_GAP：当前只有 ready/整树内存/协议探针/light 截图，没有真实帧时间、GPU 内存、CPU、冷/热分离或安装器体积。
- WINDOW_PROBE_GAP：Avalonia 三轮、Flutter 两轮未观察到 `MainWindowHandle`，但截图显示窗口存在；下一轮要改成 UI 自动化探针。
- PRIVATE_ACTIONS_QUOTA：2026 年 8 月私库 Actions 分钟耗尽；仓库已恢复 PRIVATE，不在 push 上触发 workflow。
- PUBLIC_AUTH_REQUIRED：未来每次 public/private 切换仍必须由用户针对该次操作明确授权；本轮授权已用完。
- SECURITY_REQUIRED：未来再次公开前仍需重新检查完整历史、Secrets、LFS、Release、Issue/PR、Actions artifact/cache 和许可证。
- LICENSE_PENDING：当前没有 LICENSE；临时 public 只表示源码可见，不表示已选择开源许可。
- LOCKFILE_DECISION：Validate 已生成 `cargo-lock-32017467536` artifact，但没有为了文档提交改变已测 commit；进入产品依赖冻结时再审核并提交。
- MANUAL_RISK：真实中文 IME、物理 GPU、DPI、多显示器和无障碍无法仅靠普通托管 runner 得出最终结论。

## 11. 进度日志

### 2026-08-16

- 完成技术路线调研并生成 Markdown/HTML。
- 从 Tauri-first 调整为框架竞速，不预先绑定 WinUI 3。
- 第一轮候选确定为 GPUI、Avalonia、Flutter、Tauri。
- 确定全部源码由 AI 完成，构建与测试使用 GitHub Actions。
- 记录本月私库 Actions 分钟耗尽，需要经授权临时公开运行。
- 创建 PROGRESS.md、HANDOFF.md 和连续性规则。
- 初始化本地 `main` Git 仓库，但没有 remote、commit 或作者身份。
- 只读检查发现授权环境中的 GitHub CLI 账号 `oarw` 有效；账号仓库列表中没有 `cakify`，没有尝试创建远端。
- 创建共享 Rust protocol/core、确定性 fixture、视觉 token、结果 schema 和采集脚本。
- 为 core 加入一次性随机 session token、分页、SSE、工具事件和取消状态机。
- 创建四个 UI 壳目录和仅手动触发的 validate/benchmark scaffold workflow。
- 本地静态检查通过：3 个 JSON 可解析、3 个 PowerShell 脚本语法可解析、workflow 无自动触发、secret 模式与敏感扩展名无命中。
- 重新在授权环境验证 `gh auth status`/`gh api user` 成功，账号为 `oarw`；仓库列表没有 `cakify`，没有创建或修改远端。
- 本机未安装 Cargo，未运行编译、测试或 benchmark；等待四个真实壳完成和临时公开授权。
- 远端公开前审计完成：单一初始 commit；无 Secrets、Variables、Environments、Issues/PR、Releases、Artifacts、Caches、LFS；无分支保护；Actions 权限为 enabled/all，但 workflow 内第三方 Action 已固定 SHA。
- 暂不运行四候选 matrix：四个 app 尚未有可执行 UI，当前 workflow 只会生成 scaffold_only。

### 2026-08-17

- 写入 GPUI、Avalonia、Flutter、Tauri 四个首版可执行 UI 壳，统一传入 `--core-path` 与 `--core-ready-file`。
- GPUI 固定 Zed 官方提交 `b2d9c2e122fbc408d42276b4456243ba4f90f181`；Avalonia 固定 12.1.1；Flutter 固定 3.47.0；Tauri 固定 2.11.5 / Svelte 5.56.9。
- `benchmark.yml` 从 `scaffold_only` 改为 Windows x64 release matrix，新增整棵进程树、ready、health、分页、SSE 取消、截图尝试和 result.v1 artifact 采集。
- 本地静态检查通过：JSON、PowerShell、workflow 结构和 secret 模式；未运行编译、测试、GUI 或 Actions。
- 当前仍保持 PRIVATE；等待公开前复核和用户本次 public -> Actions -> private 授权。
- 远端审计记录已写入 `docs/PUBLIC_ACTIONS_CHECKLIST.md`；Packages 通过仓库级 GraphQL `totalCount=0` 核实。
- 用户明确授权本次 `oarw/cakify` 临时公开，运行 Validate 和四候选 Benchmark，并在无活动任务后恢复私有。
- Validate 首两轮因 `cargo fmt --check` 发现格式问题失败；修复后 run `31998398331` 起通过，最终 run `32017467536` 在 commit `4020989` 成功并上传 `cargo-lock-32017467536`。
- Benchmark 早期 run `31998652946` 暴露 GPUI 类型推断、Avalonia API、Tauri workspace 和 core 取消竞态；run `32015445971` 暴露 Tauri icon；run `32016531269` 暴露 Tauri ready 类型/生命周期问题。均按日志修复并自动提交、推送、重跑。
- 最终 [`Benchmark candidates #32017470781`](https://github.com/oarw/cakify/actions/runs/32017470781) 在 commit `40209896dca0009b747efc51ac885bed32b81f25` 成功，四个 matrix job 和 artifact 均成功。
- 三轮中位数：GPUI ready 113.745 ms / idle Working Set 42.016 MiB；Avalonia 565.515 ms / 125.102 MiB；Flutter 1,642.973 ms / 126.180 MiB；Tauri 554.475 ms / 326.496 MiB。
- artifact 已下载检查：四个 `result.json` 均为同一 fixture/hash、三轮无失败、协议探针通过；四个 light 截图存在。暗色/工具截图、真实帧时间和物理机 IME 尚未覆盖。
- 确认 GitHub Actions `queued/in_progress` 数量为 0 后，仓库已恢复为 PRIVATE；本轮 visibility 授权闭环完成。
- 新增第一轮最终报告 Markdown/HTML；技术方向确定为 GPUI 主线、Avalonia 回退，进入产品纵向切片阶段。

## 12. 更新规则

每次有实质进展后必须：

1. 更新“当前快照”与“已完成/尚未开始”。
2. 把“精确下一步”改为真实可执行的下一步。
3. 记录最新 commit、branch、Action run URL/ID、失败原因和 artifact。
4. 在“进度日志”追加一条，不覆盖历史。
5. 若文档与工作区冲突，先以实际状态修正文档，再继续开发。
6. 未执行的测试不得写成已通过。
