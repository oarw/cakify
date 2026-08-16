# GitHub Actions 临时公开检查表

> 最近检查：2026-08-16（Asia/Shanghai）  
> 当前结论：远端已建立为 PRIVATE；公开前审计已完成；未执行 public/private 切换，也未运行 Actions。

## 已完成的本地检查

- [x] 本地仓库已初始化为 main，初始 commit 为 4e605d730ca61f3461e517d34955eefba9aa8b92。
- [x] 扫描常见云密钥、GitHub token、OpenAI 风格 token 和私钥头，没有命中。
- [x] 没有发现 `.env`、PFX、P12、PEM、KEY、SSH 私钥文件。
- [x] 两个 workflow 都只有 `workflow_dispatch`，没有 push、PR、schedule 或 `pull_request_target`。
- [x] workflow 顶层权限为 `contents: read`。
- [x] `actions/checkout` 与 `actions/upload-artifact` 均固定完整 commit SHA；SHA 已通过 GitHub 官方 API 核实。
- [x] workflow 不读取 secrets，不连接真实模型、MCP 或私人端点。
- [x] JSON 与 PowerShell 文件已做本地静态解析。
- [ ] Rust 编译、测试和 workflow YAML 运行尚未执行；按约束留给 GitHub Actions。

## 远端建立后必须重新检查

- [x] 在授权环境中验证 `gh auth status` 和 `gh api user`；账号 `oarw` 有效。普通沙箱的 keyring/代理隔离不代表账号失效。
- [x] remote 已核实为 https://github.com/oarw/cakify.git，仓库为 PRIVATE、默认分支为 main。
- [x] 检查完整 Git 历史：当前只有初始 commit 4e605d730ca61f3461e517d34955eefba9aa8b92。
- [x] 检查 GitHub Actions secrets、variables、environments 和 OIDC 入口：没有 secrets、variables 或 environments；Actions enabled/all。
- [x] 检查 LFS、Release、Packages、Issue/PR、Actions cache 和 artifact：均为空；没有 LFS 文件。
- [x] 检查分支保护和默认 Actions 权限：main 无分支保护；workflow 顶层为 contents: read，第三方 Action 固定 SHA。
- [ ] 明确许可证选择。当前包为 `publish = false`，仓库没有 LICENSE；临时 public 只代表源码可见，不应误写成已开源授权。
- [x] 已记录当前 PRIVATE visibility 和 commit 4e605d730ca61f3461e517d34955eefba9aa8b92；公开前仍需重新记录一次。

## 本月运行顺序

1. 完成上述远端检查并向用户展示结果。
2. 获得用户对“本次 public → Actions → private”的明确授权。
3. 改为 public。
4. 首先只运行 `Validate scaffold`，记录 run URL/ID 和 artifact。
5. 修复失败并取得 `Cargo.lock` artifact；锁文件进入源码后再实现四个 UI 壳。
6. 确认没有 queued/in_progress run，也没有需保留的 public artifact。
7. 获得对应授权后恢复 private，并核实最终 visibility。

进入 2026 年 9 月后，先核实私库 Actions 额度；若额度恢复，不再机械执行临时公开流程。
