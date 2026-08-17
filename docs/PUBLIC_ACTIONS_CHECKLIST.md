# GitHub Actions 临时公开检查表

> 最近检查：2026-08-17（Asia/Shanghai）
> 当前结论：远端保持 PRIVATE；新增四个 UI 壳、采集器和真实 benchmark workflow 已做源码级审阅；未执行 public/private 切换，也未运行 Actions。

## 已完成的本地检查

- [x] 本地仓库已初始化为 main，初始 commit 为 4e605d730ca61f3461e517d34955eefba9aa8b92。
- [x] 扫描常见云密钥、GitHub token、OpenAI 风格 token 和私钥头，没有命中。
- [x] 没有发现 `.env`、PFX、P12、PEM、KEY、SSH 私钥文件。
- [x] 两个 workflow 都只有 `workflow_dispatch`，没有 push、PR、schedule 或 `pull_request_target`。
- [x] workflow 顶层权限为 `contents: read`。
- [x] `actions/checkout`、`actions/upload-artifact` 与 `subosito/flutter-action` 均固定完整 commit SHA；前两者及 Flutter action SHA 已通过官方 API/仓库元数据核实。
- [x] workflow 不读取 secrets，不连接真实模型、MCP 或私人端点。
- [x] JSON 与 PowerShell 文件已做本地静态解析；四个 UI 的依赖版本、ready-file 边界和 workflow 路径已做源码级检查。
- [ ] Rust 编译、测试和 workflow YAML 运行尚未执行；按约束留给 GitHub Actions。

## 远端建立后必须重新检查

- [x] 在授权环境中验证 `gh auth status` 和 `gh api user`；账号 `oarw` 有效。普通沙箱的 keyring/代理隔离不代表账号失效。
- [x] remote 已核实为 https://github.com/oarw/cakify.git，仓库为 PRIVATE、默认分支为 main。
- [x] 已重新检查完整 Git 历史：当前共 4 个提交，最新实现提交为 `60b0a2c8eb8e51c9b184b0f36b45cd4d043fa725`；共享骨架基线为 `4e605d730ca61f3461e517d34955eefba9aa8b92`。
- [x] 检查 GitHub Actions secrets、variables、environments 和 OIDC 入口：没有 secrets、variables 或 environments；Actions enabled/all。
- [x] LFS endpoint 返回 404（未配置）、Release、Issue/PR、Actions cache 和 artifact 均为空；远端敏感路径扫描无命中。
- [ ] 重新确认 Packages：当前 `gh` token 没有 `read:packages`，用户级 packages API 返回 403，不能将其误记为空。
- [x] 检查分支保护和默认 Actions 权限：main 无分支保护；workflow 顶层为 contents: read，第三方 Action 固定 SHA。
- [ ] 明确许可证选择。当前包为 `publish = false`，仓库没有 LICENSE；临时 public 只代表源码可见，不应误写成已开源授权。
- [x] 已记录 PRIVATE 状态和共享骨架基线 4e605d730ca61f3461e517d34955eefba9aa8b92；公开前必须重新记录最新 HEAD。

## 2026-08-17 远端复核记录

- visibility：`PRIVATE`；default branch：`main`；远端 URL：`https://github.com/oarw/cakify`。
- commit history：4 个提交，HEAD `60b0a2c8eb8e51c9b184b0f36b45cd4d043fa725`。
- Actions：run 列表为空；Secrets 0、Variables 0、Environments 0、Artifacts 0、Caches 0；Actions enabled/all，默认 workflow permissions 为 read。
- Releases、Issues、Pull requests、webhooks 均为 0；main 无分支保护；Pages endpoint 未配置（404）。
- 本地与远端树的敏感扩展名/私钥路径扫描无命中；本轮没有修改 visibility 或触发 workflow。
- Packages 尚有权限缺口，公开前必须补核。

## 本月运行顺序

1. 完成上述远端检查并向用户展示结果。
2. 获得用户对“本次 public → Actions → private”的明确授权。
3. 改为 public。
4. 首先只运行 `Validate scaffold`，记录 run URL/ID 和 artifact。
5. 修复失败并取得 `Cargo.lock`、框架依赖锁文件和真实 benchmark artifact；未运行前不把 workflow 写成通过。
6. 确认没有 queued/in_progress run，也没有需保留的 public artifact。
7. 获得对应授权后恢复 private，并核实最终 visibility。

进入 2026 年 9 月后，先核实私库 Actions 额度；若额度恢复，不再机械执行临时公开流程。
