# Cakify Benchmark 进度记录

> 本文件是项目状态的单一事实来源。
> 最后更新：2026-08-17（Asia/Shanghai）
> 当前阶段：Phase 2 - 四候选 UI 壳与 CI wiring
> 当前状态：LOCAL_UI_SHELLS_READY

## 1. 当前快照

- 工作目录：C:\Users\admin\Desktop\code\cakify
- Git 状态：本地 main 跟踪 origin/main；共享骨架基线为 4e605d730ca61f3461e517d34955eefba9aa8b92；实际 HEAD 以 `git rev-parse HEAD` 为准。
- GitHub 远端：`https://github.com/oarw/cakify.git`。
- 原型源码：共享 Cargo workspace、benchmark protocol/core、fixture、视觉 token、结果 schema、采集脚本，以及 GPUI、Avalonia、Flutter、Tauri 四个首版可执行 UI 壳均已写入。
- GitHub Actions：`validate.yml` 与 `benchmark.yml` 均仅允许 `workflow_dispatch`；benchmark 已从 scaffold matrix 升级为四候选 Windows x64 release 构建/采集矩阵，尚未运行。
- 仓库可见性：当前为 PRIVATE；本轮没有 public/private 切换。
- 远端审计：2026-08-17 已核实 4 个提交、Secrets/Variables/Environments/Release/Issue/PR/Artifacts/Caches 为 0、Actions run 为空、main 无保护；Packages 因 token 缺 `read:packages` 尚待补核。
- 当前目标：完成源码审阅与公开前安全复核；获得本次明确授权后，临时公开运行真实四候选 matrix，再恢复 PRIVATE。
- 执行方式：源码由 AI 完成；本机只做源码编辑和静态解析；构建、测试、基准、打包通过 GitHub Actions。
- 本地验证：JSON 可解析、PowerShell 语法可解析、常见 secret/私钥文件扫描无命中、workflow 触发器与固定 SHA 已检查；未进行编译、测试或 GUI 启动。
- 未验证：本机没有 Cargo，未执行 Rust 格式化、编译、测试、GUI 启动或 benchmark；不得把当前状态写成构建通过。

## 2. 候选技术栈

1. GPUI + Rust
2. Avalonia + C# + Rust
3. Flutter + Rust
4. Tauri + Svelte + Rust

WinUI 3、C++/WinRT、Slint 暂不进入第一轮矩阵，但保留为后续候选。第一轮结束前不选最终 UI 框架。

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

## 7. 尚未开始

- [x] 配置 Git 作者身份并创建初始 commit。
- [x] 配置并核实 oarw/cakify 私有 GitHub 远端。
- [ ] 选择公开仓库许可证；当前没有 LICENSE，Rust 包为 `publish = false`。
- [ ] 在 Actions 中运行 Rust 格式化与契约测试（需先实现真实壳并获得临时公开授权）。
- [ ] 从首次 validate artifact 取得并提交 `Cargo.lock`。
- [ ] 在 Actions 编译并验证 GPUI UI 原型。
- [ ] 在 Actions 编译并验证 Avalonia UI 原型。
- [ ] 在 Actions 编译并验证 Flutter UI 原型。
- [ ] 在 Actions 编译并验证 Tauri + Svelte UI 原型。
- [ ] 运行四套 x64 release 构建与真实采集矩阵。
- [ ] 生成第一轮安装包、截图和 JSON 指标。
- [ ] 至少完成三轮稳定基准。
- [ ] 输出最终技术选型报告。

## 8. 精确下一步

下一位执行者从这里继续，不要重新做框架泛泛调研：

1. 在源码层完成 GPUI、Avalonia、Flutter、Tauri 的 API/路径审阅，不在本机编译。
2. 重新执行公开前历史、Secrets、LFS、Release、Issue/PR、cache/artifact 与许可证审计。
3. 向用户展示审计结果并获得本次 `public -> Actions -> private` 的明确授权。
4. 临时公开后先跑 validate，再跑真实四候选 matrix；确认无 queued/in_progress 后恢复 PRIVATE。
5. 记录每个 run 的 URL/ID、commit、artifact、失败原因和最终结论；未运行不得写成通过。

## 9. 预计节奏

- 快速粗测：8–16 小时 AI 工作时间。
- 可用于架构决策：2–4 个自然日，取决于 Actions 反馈轮次。
- 接近产品体验的四套原型：5–8 个自然日，不作为第一轮目标。

## 10. 当前阻塞与授权门

- CI_COMPILE_PENDING：四个 UI 壳已写入，但尚未在 Windows runner 编译和启动验证。
- PRIVATE_ACTIONS_QUOTA：2026 年 8 月私库 Actions 分钟耗尽；当前仓库保持 PRIVATE，不运行 workflow。
- PUBLIC_AUTH_REQUIRED：任何 public/private 切换必须由用户针对本次操作明确授权。
- SECURITY_REQUIRED：公开前必须完成完整历史、Secrets、LFS、Release、Issue、PR、Actions artifact/cache 和许可证检查。
- LICENSE_PENDING：当前没有 LICENSE；临时 public 只表示源码可见，不表示已选择开源许可。
- BOOTSTRAP_PENDING：尚无已审核的 Cargo.lock；由首次 validate/release workflow 生成 artifact 后决定是否提交。
- MANUAL_RISK：真实中文 IME、物理 GPU、DPI 和多显示器无法仅靠普通托管 runner 得出最终结论。

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
- 远端审计记录已写入 `docs/PUBLIC_ACTIONS_CHECKLIST.md`；Packages 权限缺口不能当作“无包”结论。

## 12. 更新规则

每次有实质进展后必须：

1. 更新“当前快照”与“已完成/尚未开始”。
2. 把“精确下一步”改为真实可执行的下一步。
3. 记录最新 commit、branch、Action run URL/ID、失败原因和 artifact。
4. 在“进度日志”追加一条，不覆盖历史。
5. 若文档与工作区冲突，先以实际状态修正文档，再继续开发。
6. 未执行的测试不得写成已通过。
