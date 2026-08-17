# Product validate 公开前安全审计

> 审计日期：2026-08-17（Asia/Shanghai）
> 审计源码基线：commit `b87789ce6c145cb8b1507ba077d8112d744dcdac`
> 当前结论：未发现 secret 或用户数据；等待用户对本次 public -> Product validate -> private 的明确授权。

本文件及同步进度文档是审计完成后的记录增量，提交前同样执行敏感模式和 staged diff 检查。真正切换 public 前还必须对当时的实际 HEAD 做一次增量复核。

## 1. 审计边界

本审计只判断当前仓库是否适合为了 GitHub Actions 临时公开，不代表产品源码已经编译、测试或达到安全发布标准。

临时公开仍有不可逆风险：公开期间任何人都可能读取、下载或克隆可达 Git 历史和公开 Actions 内容。恢复 private 不能收回第三方已经取得的副本，也不能证明没有发生无人记录的下载。

## 2. Git 历史

- Remote 只有 `origin/main`，没有其他远端分支或 tag。
- 扫描全部 18 个可达 commit、141 个历史路径。
- 高置信 token、API key、私钥头和带凭据 URL 模式：0 命中。
- `.env`、私钥、证书、数据库、密码库和 credential/secret 命名文件：0 命中。
- 启发式 password/client_secret/api_key/Authorization 赋值：0 命中。
- 最大历史 blob 约 55 KiB，没有大型用户数据或未知二进制进入 Git 历史。
- Commit author 使用 GitHub noreply 地址；仓库文档中已经出现本机路径 `C:\Users\admin\Desktop\code\cakify`。该路径在前次临时公开时已经可见，不能视为可收回信息。

## 3. GitHub Secrets 与配置

- Actions secrets：0。
- Dependabot secrets：0。
- Codespaces secrets：0。
- Actions variables：0。
- Environments：0。
- 当前只有一个 active workflow：`.github/workflows/product-validate.yml`。
- `Product validate` 只有 `workflow_dispatch`，没有 push、pull_request、schedule 或外部事件触发器；权限为 `contents: read`。

## 4. Actions 日志、artifact 与 cache

- 历史 Actions run：10 个；全部日志可读取。
- 扫描日志约 1,719,506 字符，高置信 token、私钥和带凭据 URL：0 命中。
- 历史 artifact：20 个，来自四候选 benchmark 与 lockfile。
- 实际下载并解包 artifact：221 个文件，共 410,466,234 bytes。
- artifact 高置信 secret 模式与敏感文件名：0 命中；临时下载目录已删除。
- GitHub 仍保存两份 Flutter 工具链 cache：`flutter-pub-windows-stable-3.47.0...` 约 24 MiB，`flutter-windows-stable-3.47.0...` 约 1.79 GiB。
- cache 由已归档的 Flutter benchmark workflow 创建；创建 run 的日志已纳入上述扫描。当前 `Product validate` 没有 cache restore/save 步骤，不会读取这些 cache。

Cache 内容不能通过当前 GitHub API 直接做与 artifact 相同的逐文件扫描，因此这是残余风险而不是“已证明内容为空”。依据 cache key、创建 workflow、无 repository secrets 和完整创建日志扫描，当前没有证据表明其中含 Cakify 用户数据或 secret。它们不是本次 product validate 的输入。

## 5. LFS、Release、Issue 与公开协作面

- `.gitattributes` 不存在，Git 历史没有 LFS pointer，未使用 Git LFS。
- Release：0；没有 release asset。
- Issue：0。
- Pull request：0。
- Fork：0。
- GitHub Environment：0。
- 分支保护当前未启用；本次只通过已登录 owner 手动 dispatch，不开放外部写权限。

Fork 计数为 0 只代表 GitHub 当前记录的 fork。前次公开期间是否有人直接 clone/download 无法证明，后续临时公开同样如此。

## 6. 许可证

- 仓库根目录没有 `LICENSE`，GitHub `licenseInfo` 为 `null`。
- 临时公开会让源码可见，但不自动授予常规开源许可证权利。
- 许可证未定不妨碍 owner 手动运行 CI，但会增加公开分发和第三方理解成本；正式长期公开或发布前必须解决。
- 不得因为临时公开而引入或复制 Zed GPL Agent/AI 业务代码。产品依赖树仍由 workflow 的禁止列表检查。

## 7. 本次允许范围

用户明确授权后，本次只执行：

1. 再次确认 HEAD、private 状态和没有 queued/in_progress run。
2. 将 `oarw/cakify` 临时设为 public。
3. 只 dispatch `Product validate`，目标分支 `main`。
4. 记录 run URL/ID、commit SHA、所有 job/step 结论和 `product-validation-<run_id>` artifact。
5. 核对依赖树、release EXE 和生成的 `Cargo.lock`；失败则记录真实失败，不扩大运行范围。
6. 确认没有 queued/in_progress run 后恢复 private，并再次核对 visibility。

本次授权不包含 benchmark、package、release、删除 cache/artifact、创建 Release 或长期公开。任何新一轮 public/private 切换都需要重新授权。

## 8. 审计结论

没有发现阻止本次临时公开运行 `Product validate` 的 secret 或用户数据问题。已知残余风险是：

- 公开可见性和第三方下载不可逆。
- 两份旧 Flutter cache 未逐文件扫描，但当前 workflow 不使用，且创建来源和日志无 secret 证据。
- 仓库没有许可证。
- M0 产品代码未编译，workflow 很可能暴露需要修复的格式或 API 错误；这属于验证目标，不是安全审计失败。

在用户明确确认本次 visibility 切换前，仓库必须保持 private，不得 dispatch workflow。
