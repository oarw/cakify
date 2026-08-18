# GitHub Actions 临时公开安全审计记录

> 最近审计日期：2026-08-18（Asia/Shanghai）
> 原 Product validate 审计基线：commit `b87789ce6c145cb8b1507ba077d8112d744dcdac`
> 最近 runtime 审计基线：commit `a1f10429a7f48b5a7ca5968976676d6e2594554d`
> 最近 M1 Product validate 基线：commit `054aaf6b0ea939d41f455921ced714e4461ed5fa`
> 最近 M1 repository 基线：commit `621097cdc08a9ac5129eef2200c2b8c7628504e2`
> 当前结论：Product validate、M0 runtime smoke、M1 storage foundation、repository/crash recovery、Provider profile 与 SecretStore 的受控 public -> Actions -> private 均已完成；仓库已恢复 PRIVATE。

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
- 当前有两个 active workflow：`.github/workflows/product-validate.yml` 与 `.github/workflows/windows-runtime-smoke.yml`。
- 两者都只有 `workflow_dispatch`，没有 push、pull_request、schedule 或外部事件触发器；权限均为 `contents: read`。

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

## 7. 本次允许范围与结果

用户明确授权后，本次只执行了：

1. 再次确认 HEAD、private 状态和没有 queued/in_progress run。
2. 将 `oarw/cakify` 临时设为 public。
3. 只 dispatch `Product validate`，目标分支 `main`。
4. 记录 run URL/ID、commit SHA、所有 job/step 结论和 `product-validation-<run_id>` artifact。
5. 核对依赖树、release EXE 和生成的 `Cargo.lock`；失败则记录真实失败，不扩大运行范围。
6. 确认没有 queued/in_progress run 后恢复 private，并再次核对 visibility。

本次没有运行 benchmark、package、release，没有删除 GitHub cache/artifact、创建 Release 或长期公开。

实际 Product validate 记录：

- `32032509531`，commit `9b6e71e07514c6f447de084a527d9a571b8368bd`，`failure`：依赖/许可证边界通过，格式检查失败；artifact `product-validation-32032509531`（ID `9289483786`）。
- `32033412479`，commit `2fe81c3a4b2e1b744c9c0d003577da5482a7e24b`，`failure`：格式通过，workspace check 发现 6 个同源 E0503；artifact `product-validation-32033412479`（ID `9289873982`）。
- `32034202488`，commit `a2d19ceb5647ce050a5012ed2b8fdc1d7f7db4ab`，`success`：fmt、check、tests、Clippy、release build、artifact upload 全部通过；artifact `product-validation-32034202488`（ID `9290400569`）。

最终 artifact archive digest 为 `sha256:e9c4f5f0db1488d8f946acfcb2766d2d0ccd4f313fa4f7a476747639f9a8a7b5`。release EXE 为 5,722,112 bytes，SHA-256 `4EB5AF9970EAFFC35850C599CD2A91685D6C1CC9FCB11B45526CA5B8D7DBF8DF`，当前未签名；锁文件逐行匹配仓库，越界框架包与 artifact 文本 secret 均为 0 命中。恢复 private 前确认 queued/in_progress 均为空，随后已复核仓库为 PRIVATE。

用户另于 2026-08-17 持续授权 2026 年 8 月后续的受控临时公开闭环。后续每次仍须按本审计维度复核；无新增实质风险时可自动执行 public -> 当前任务所需手动 workflow -> 核对 -> 无活动任务 -> private，不再逐次询问。长期公开、Release/发包、无关 workflow 或新增风险不在该授权内；授权于 2026-08-31 23:59（Asia/Shanghai）或用户撤销时失效。

## 8. 审计结论

没有发现阻止本次临时公开运行 `Product validate` 的 secret 或用户数据问题。本次运行和恢复 PRIVATE 已完成。已知残余风险是：

- 公开可见性和第三方下载不可逆。
- 两份旧 Flutter cache 未逐文件扫描，但当前 workflow 不使用，且创建来源和日志无 secret 证据。
- 仓库没有许可证。
- M0 产品代码已通过编译、测试、Clippy、release build 与最终 Windows runtime smoke；真实 IME/accessibility 仍是后续物理机独立门，不属于公开审计结论。

后续执行者必须遵守持续授权的边界，任何公开状态都不得闲置或跨会话遗留。

## 9. M0 Windows runtime smoke 审计与闭环

2026-08-17 至 2026-08-18 又执行了三次只针对 `Windows runtime smoke` 的受控循环。前两次没有被误写成最终验收：`32037554962` 的顶层内存聚合错误为 0；`32038434473` 的性能/生命周期证据有效，但截图显示窗口底部被任务栏遮挡。最终源提交为 `a1f10429a7f48b5a7ca5968976676d6e2594554d`。

最终公开前复核结果：

- 全部 25 个可达 commit、145 个历史路径，高置信 secret、敏感历史文件名与 LFS pointer 均为 0。
- Actions/Dependabot/Codespaces secrets、Actions variables、Environments 均为 0。
- 三次 runtime 日志共 250,394 字符，高置信 secret 命中 0。
- 两份旧 Flutter cache 的 ID、key、大小与来源未变化；当前 workflow 不读取 cache。
- Release、Issue、PR、tag、fork 均为 0；仓库仍无 `LICENSE`。
- 切换前仓库为 PRIVATE，HEAD 与 `origin/main` 均为 `a1f10429a7f48b5a7ca5968976676d6e2594554d`，queued/in_progress 均为空。

最终 run [Windows runtime smoke `32093988986`](https://github.com/oarw/cakify/actions/runs/32093988986) 为 `success`，job `95581655025`。artifact `windows-runtime-smoke-32093988986` 的 ID 为 `9309416529`，archive digest 为 `sha256:8a09c0785d1cc77257c798f26247a5bec16ae8e63b95ab129faa04a18430a6c3`。三轮窗口矩形均为 `(24,55)-(1000,714)`，完整位于 `(0,0)-(1024,720)` 工作区；空闲整树 Working Set 为 `37.121/35.480/35.477 MiB`，峰值最高 `38.320 MiB`，默认进程数 1、子进程 0，WM_CLOSE 后均以 code 0 正常退出且无残留。JSONL 独立复算与 result JSON 逐项一致，6 份 stdout/stderr 只有空白，artifact 文本 secret 命中 0，截图目视无遮挡。

artifact 内 EXE 为 5,722,624 bytes，SHA-256 `CE54D290BD0F0A19F1CDDE0322C4A7C2D09838D62CCE4B5DDDAD276EA035EA78`；result JSON SHA-256 `9E9FEEB09AB3266E9098020B48E23F5DB55BDFA951811D0C85307E3F98FA5930`；截图 SHA-256 `34062732DE298CDED4B8BF9D58D0650C6D7F44B2B67C9C607C82509A2B202E12`。核对后再次确认 queued/in_progress 为空，随即恢复 PRIVATE 并复核 `isPrivate=true`。本循环没有运行 benchmark、package、release 或无关 workflow。

## 10. M1 storage foundation Product validate 审计与闭环

2026-08-18 对 M1 SQLite/storage foundation 执行一次持续授权范围内的受控修复循环。公开前目标 commit `900bcde26847fc9910d50823469262bb4295ee9c`：27 个可达 commit、152 个历史路径，高置信 secret、敏感文件名与 LFS pointer 0 命中；Actions/Dependabot/Codespaces secrets、variables、environments 均为 0；Release、Issue、PR、tag、fork 均为 0。两份旧 Flutter cache 未变化且当前 workflow 不使用，仓库仍无 `LICENSE`。

首轮 [Product validate `32097396883`](https://github.com/oarw/cakify/actions/runs/32097396883) 为 `failure`：依赖树与许可证边界成功，`cargo fmt --check` 失败，其余编译/测试步骤未运行。artifact `product-validation-32097396883`（ID `9310434562`，digest `sha256:f1c82871e39b1e5ac87188fa1c9608211a52826d5f8b3ae470a7bb75ca2add34`）已成功下载并核验；由此取回 runner 生成的 `Cargo.lock`，其 SHA-256 为 `731531574FD1B25AA23F8B0476BF60365D2529B894F50FE5A0AC020B34441E30`。`rusqlite 0.40.2` registry checksum 与官方值一致，依赖树未出现网络栈、向量库或密钥库越界包；migration artifact 与目标提交内容一致，仅 checkout 行尾为 CRLF。

按 runner 精确格式差异修复并提交锁文件后，最终 [Product validate `32097907337`](https://github.com/oarw/cakify/actions/runs/32097907337) 在 commit `785241720db087ce38121b095ea5f192063ab2b4` 上为 `success`，job `95592703383`。fmt、workspace check、全量 tests、专门的 5 项 storage contract、Clippy、release build 与 artifact upload 全部成功；测试覆盖 PRAGMA/schema、外键孤儿拒绝、重复打开、migration checksum 篡改与未来 schema 拒绝。

最终 artifact `product-validation-32097907337` 的 ID 为 `9310763337`，上传日志和 API 均记录 5 个文件、2,385,820 bytes、archive digest `sha256:66b893168eadc5ead939c71c4059ca65f15cc6c9f2b2c38c2e3f49a2274ab118`。本机随后通过 GitHub CLI、GitHub API/curl 三种路径下载时，均在 Azure Blob `productionresultssa8.blob.core.windows.net:443` 连接超时；因此最终 ZIP 的锁文件/EXE/SQL/secret 独立解包检查仍明确为待办，不能伪造为已经完成。

下载超时后没有让仓库继续公开等待：再次确认 queued/in_progress 为 0，立即恢复 PRIVATE，并复核 `isPrivate=true` 与最终 run `completed/success`。本循环只运行 Product validate，没有运行 benchmark、runtime smoke、package、release 或任何无关 workflow。

## 11. M1 repository/crash recovery Product validate 闭环

2026-08-18 对 repository/crash recovery 源码执行下一次持续授权范围内的受控循环。公开前目标 commit `2f4b8688fc71ae727781baa7ac9306db48f9e2aa`：30 个可达 commit、156 个历史路径，高置信 secret、敏感文件名与 LFS pointer 0 命中；Actions/Dependabot/Codespaces secrets、variables、environments 均为 0；18 个历史 run 无活动任务，Release、Issue、PR、tag、fork 均为 0。两份旧 Flutter cache 共 1,946,280,557 bytes，未变化且当前 workflow 不使用；仓库仍无 `LICENSE`。

首轮 [Product validate `32100633458`](https://github.com/oarw/cakify/actions/runs/32100633458) 为 `failure`：依赖树与许可证边界成功，`cargo fmt --check` 失败，其余编译/测试步骤未运行。artifact `product-validation-32100633458`（ID `9311450847`，digest `sha256:b15cb49d2ab6970c9de214e8a215c0856a8c00fd8bfc0663223da0401e8de9a2`）上传成功；按 runner 完整差异修复纯格式后提交 `621097cdc08a9ac5129eef2200c2b8c7628504e2`。

最终 [Product validate `32100910742`](https://github.com/oarw/cakify/actions/runs/32100910742) 为 `success`，job `95601074839`。fmt、workspace check、全量 tests、storage contract 5/5、repository contract 4/4、Clippy、release build 与 artifact upload 全部成功。repository contract 实际覆盖稳定 cursor 分页与软删除、message+parts 聚合事务回滚/顺序、checkpoint revision 幂等/stale 拒绝、active run 一次性 interrupted 恢复与文本保留、run 单调/终态保护、级联 purge。

最终 artifact `product-validation-32100910742` 的 ID 为 `9311722769`，上传日志与 API 均记录 6 个文件、2,386,139 bytes、archive digest `sha256:51d059c6089178c9afb56c858d594d744892071da2a1b28cd6edf24e96f144af`。本机下载时在另一 Azure Blob 主机 `productionresultssa2.blob.core.windows.net:443` 同样连接超时，因此 ZIP 内容独立解包仍明确待办；未将其伪造为已检查。

下载超时后再次确认 queued/in_progress 为 0，立即恢复 PRIVATE，并复核 `isPrivate=true` 与最终 run `completed/success`。本循环只运行 Product validate，没有运行 benchmark、runtime smoke、package、release 或无关 workflow。

## 12. M1 SecretStore Product validate 审计与闭环

2026-08-18 对 SecretStore 源码执行一次持续授权范围内的受控循环。目标 HEAD 为 `c6e109b5bbc741e37486913fda1ed94e4829d8f0`，公开前复核覆盖 37 个可达 commit、524 个 Git objects、160 个历史路径；高置信 token/私钥/凭据 URL 0 命中，敏感文件名 0 命中，当前 LFS pointer 0 命中。Actions/Dependabot/Codespaces secrets、variables、environments 均为 0，Release、Issue、PR、fork、tag 均为 0，仓库没有 `LICENSE`。两份旧 Flutter cache 未变化，当前 workflow 不读取 cache；本轮只增加 CredMan/DPAPI 源码和 synthetic tests，没有真实用户 secret 或外部输入。

首轮 [Product validate `32127609188`](https://github.com/oarw/cakify/actions/runs/32127609188) 为 `failure`：依赖树/许可证边界通过，rustfmt 发现三个新 Rust 文件的纯格式差异；workspace check、tests、secret contracts、Clippy、release build 未执行。artifact `product-validation-32127609188`（ID `9321000183`，digest `sha256:4f89b37f5f4b44bda62d80eee926732446fe7e4b2adf500dfa903dcf1681c07`）只包含 runner 生成的锁文件和依赖树，不能记作 SecretStore 通过。

按 runner 完整格式差异修复并推送 commit `054aaf6b0ea939d41f455921ced714e4461ed5fa` 后，最终 [Product validate `32127969715`](https://github.com/oarw/cakify/actions/runs/32127969715) 为 `success`，job `95682647629`。fmt、workspace check、全量 tests、storage/repository/provider/secret contracts、Clippy、release build 与 artifact upload 全部成功；secret contract 实际覆盖 Core 两阶段生命周期、Credential Manager put/get/update/delete/idempotent cleanup，以及 DPAPI current-user round-trip、密文不含测试明文、tamper failure 和删除。

最终 artifact `product-validation-32127969715` 的 ID 为 `9321446137`，大小 2,386,299 bytes，archive digest `sha256:1b4f6f03c4a6d0883f5c11d94b87a061d2d38db1e660d8401433d5d6fb6c795d`。实际下载解包得到 6 个文件：`Cargo.lock`、dependency tree、三份 migration 和 release EXE。锁文件 CRLF -> LF 归一化后与仓库一致；依赖树越界 GPL AI crate 0，artifact 文本高置信 secret 0。EXE 为 5,722,624 bytes，SHA-256 `9C63E9A44A8C7AC78D03FDCDAC4B3F9922E9A2388A9122B97F75B226982F3E0D`，`NotSigned`（M7 前预期状态）。

核对产物后确认 queued/in_progress 为 0，立即恢复 PRIVATE 并复核 `isPrivate=true`。本循环没有运行 benchmark、runtime smoke、package、release 或任何无关 workflow；未创建 Release、未删除 cache/artifact、未长期公开。
