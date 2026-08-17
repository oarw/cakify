# GitHub Actions 临时公开检查表

> 最近检查：2026-08-17（Asia/Shanghai）
> 当前结论：本轮经用户明确授权完成 `PRIVATE -> PUBLIC -> Actions -> PRIVATE`；Validate 与四候选 Benchmark 成功，artifact 已核对，当前远端为 PRIVATE。

## 已完成的本地检查

- [x] 本地仓库已初始化为 main，初始 commit 为 4e605d730ca61f3461e517d34955eefba9aa8b92。
- [x] 扫描常见云密钥、GitHub token、OpenAI 风格 token 和私钥头，没有命中。
- [x] 没有发现 `.env`、PFX、P12、PEM、KEY、SSH 私钥文件。
- [x] 两个 workflow 都只有 `workflow_dispatch`，没有 push、PR、schedule 或 `pull_request_target`。
- [x] workflow 顶层权限为 `contents: read`。
- [x] `actions/checkout`、`actions/upload-artifact` 与 `subosito/flutter-action` 均固定完整 commit SHA；前两者及 Flutter action SHA 已通过官方 API/仓库元数据核实。
- [x] workflow 不读取 secrets，不连接真实模型、MCP 或私人端点。
- [x] JSON 与 PowerShell 文件已做本地静态解析；四个 UI 的依赖版本、ready-file 边界和 workflow 路径已做源码级检查。
- [x] Rust 格式检查、契约测试和 workflow 实际运行均由 GitHub Actions 完成；本机没有执行项目构建或测试。

## 远端建立后必须重新检查

- [x] 在授权环境中验证 `gh auth status` 和 `gh api user`；账号 `oarw` 有效。普通沙箱的 keyring/代理隔离不代表账号失效。
- [x] remote 已核实为 https://github.com/oarw/cakify.git，仓库为 PRIVATE、默认分支为 main。
- [x] 已重新检查完整 Git 历史：实现审计基线为 `60b0a2c8eb8e51c9b184b0f36b45cd4d043fa725`，随后只追加审计/交接文档；共享骨架基线为 `4e605d730ca61f3461e517d34955eefba9aa8b92`。公开前必须用 API 重新读取实际 HEAD/提交数，避免文档提交造成自引用失真。
- [x] 检查 GitHub Actions secrets、variables、environments 和 OIDC 入口：没有 secrets、variables 或 environments；Actions enabled/all。
- [x] 公开前 LFS endpoint 返回 404（未配置）、Release、Issue/PR、Actions cache 和 artifact 均为空；远端敏感路径扫描无命中。运行后仅出现本轮预期的 Actions runs/artifacts。
- [x] 仓库级 GraphQL `packages.totalCount` 返回 0；读取包名/类型详情需要 `read:packages`，但确认当前仓库无 Packages 不需要扩 scope。
- [x] 检查分支保护和默认 Actions 权限：main 无分支保护；workflow 顶层为 contents: read，第三方 Action 固定 SHA。
- [ ] 明确许可证选择。当前包为 `publish = false`，仓库没有 LICENSE；临时 public 只代表源码可见，不应误写成已开源授权。
- [x] 已记录 PRIVATE 状态和共享骨架基线 4e605d730ca61f3461e517d34955eefba9aa8b92；公开前必须重新记录最新 HEAD。

## 2026-08-17 公开前复核记录

- visibility：`PRIVATE`；default branch：`main`；远端 URL：`https://github.com/oarw/cakify`。
- commit history：实现审计基线 `60b0a2c8eb8e51c9b184b0f36b45cd4d043fa725`；之后仅追加审计文档。实际 HEAD/提交数以公开前的 `gh api repos/oarw/cakify/commits/main` 为准。
- Actions：run 列表为空；Secrets 0、Variables 0、Environments 0、Artifacts 0、Caches 0；Actions enabled/all，默认 workflow permissions 为 read。
- Releases、Issues、Pull requests、webhooks 均为 0；main 无分支保护；Pages endpoint 未配置（404）。
- 本地与远端树的敏感扩展名/私钥路径扫描无命中；随后取得用户对本轮 visibility 切换与两条 workflow 的明确授权。
- Packages：仓库级 `totalCount` 为 0；没有扩 token scope。

## 本轮执行记录

1. [x] 完成公开前历史、Secrets、Variables、Environments、LFS、Release、Issue/PR、Packages、Artifacts/Caches、许可证和分支保护检查。
2. [x] 用户明确确认本次将 `oarw/cakify` 临时设为 public，运行 Validate 与四候选 Benchmark，并在无活动任务后恢复 private。
3. [x] 仓库临时改为 PUBLIC。
4. [x] 运行 Validate；早期格式错误按 Actions 日志修复，最终 [`#32017467536`](https://github.com/oarw/cakify/actions/runs/32017467536) 在 commit `4020989` 成功，artifact 为 `cargo-lock-32017467536`（id `9284086377`）。
5. [x] 运行 Benchmark；按真实编译/启动反馈修复候选问题，最终 [`#32017470781`](https://github.com/oarw/cakify/actions/runs/32017470781) 四个 matrix job 全部成功。
6. [x] 核对四个 `benchmark-*-32017470781` artifact 的 result JSON、原始采样、app 和 light 截图；fixture 一致、每项三轮、协议探针通过。
7. [x] 再次查询 Actions，`queued/in_progress` 数量为 0。
8. [x] 按本轮授权将仓库恢复 PRIVATE，并通过 GitHub API/CLI 核实 `isPrivate=true`、`visibility=PRIVATE`。

最终 benchmark artifact：

- GPUI：id `9284344766`，2,827,595 bytes。
- Tauri：id `9284312348`，1,622,511 bytes。
- Flutter：id `9284267417`，12,065,123 bytes。
- Avalonia：id `9284201176`，39,451,268 bytes。

公开后已产生的提交和潜在第三方副本无法通过恢复 private 收回；当前没有 LICENSE，临时公开仍不代表授予开源许可。未来任何 visibility 切换必须重新审计并获得新的明确授权，不能复用本轮确认。

进入 2026 年 9 月后，先核实私库 Actions 额度；若额度恢复，不再机械执行临时公开流程。
