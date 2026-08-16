# Cakify 项目协作规则

- 始终使用简体中文交流。
- 本机原则上只编辑源码；编译、测试、基准、构建、打包和发布均使用 GitHub Actions。除非用户明确改变该约束，不在本机安装大批开发环境或执行项目构建。

## 2026 年 8 月 GitHub Actions 临时约束

- 当前私有仓库的 GitHub Actions 分钟已经耗尽。有效期至 2026-08-31 23:59（Asia/Shanghai）；进入新月份后先核实额度，不要继续盲目套用。
- 仓库处于私有状态时，不得触发会消耗私库分钟的 workflow。
- 本月确需运行 Actions 时，顺序必须是：公开前安全检查 → 获得用户对本次可见性切换的明确确认 → 将仓库设为 public → 运行并确认 Actions 与产物 → 确认没有待运行任务 → 将仓库恢复为 private。
- 公开前必须检查 Git 历史、Secrets、Actions 日志/缓存、LFS、Release、Issue 与许可证。公开过的提交和第三方 fork 应视为无法收回。
- 不得仅凭本规则自动修改仓库可见性；每一次 public/private 切换仍需用户明确授权。

## 会话连续性

- 每次开始工作前必须完整阅读 docs/HANDOFF.md 和 docs/PROGRESS.md。
- 阅读后先检查实际文件、Git 状态、remote 和 Actions；实际状态优先于文档。
- 不重新进行已完成的泛泛选型讨论，优先执行 PROGRESS.md 中的“精确下一步”。
- 完成任何实质改动后，更新 PROGRESS.md 的当前快照、清单、阻塞与进度日志。
- 停止工作或更换模型/供应商前，更新 HANDOFF.md 的交接槽位。
- Actions 结果必须记录 run URL/ID、commit SHA、artifact 和结论；未实际运行不得写成通过。
