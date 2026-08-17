# Cakify 项目协作规则

- 始终使用简体中文交流。
- 本机原则上只编辑源码；编译、测试、基准、构建、打包和发布均使用 GitHub Actions。除非用户明确改变该约束，不在本机安装大批开发环境或执行项目构建。

## 2026 年 8 月 GitHub Actions 临时约束

- 当前私有仓库的 GitHub Actions 分钟已经耗尽。有效期至 2026-08-31 23:59（Asia/Shanghai）；进入新月份后先核实额度，不要继续盲目套用。
- 仓库处于私有状态时，不得触发会消耗私库分钟的 workflow。
- 用户已于 2026-08-17 给出本月后续受控闭环的持续授权。确需运行 Actions 时，执行者在完成安全复核且未发现新增实质风险后，自动按以下顺序执行，无需逐次停下来询问：确认 private 与无活动任务 → 将仓库设为 public → 只运行当前任务所需的手动 workflow → 核对 Actions 与产物 → 确认没有待运行任务 → 立即恢复 private。
- 公开前必须检查 Git 历史、Secrets、Actions 日志/缓存、LFS、Release、Issue 与许可证。公开过的提交和第三方 fork 应视为无法收回。
- 自动授权不包含长期公开、创建 Release、发布包、运行与当前任务无关的 workflow，或在审计发现 secret、新敏感历史、未知外部输入等新增实质风险后继续公开；这些情况必须先请求用户确认。
- 公开状态不得闲置或跨会话遗留。出现失败时只可为修复当前 workflow 主动维持公开；停止工作前必须先确认无 queued/in_progress 并恢复 private。
- 本持续授权于 2026-08-31 23:59（Asia/Shanghai）失效，用户明确撤销时也立即失效；进入新月份先核实额度和规则。

## 会话连续性

- 每次开始工作前必须完整阅读 docs/HANDOFF.md 和 docs/PROGRESS.md。
- 阅读后先检查实际文件、Git 状态、remote 和 Actions；实际状态优先于文档。
- 不重新进行已完成的泛泛选型讨论，优先执行 PROGRESS.md 中的“精确下一步”。
- 完成任何实质改动后，更新 PROGRESS.md 的当前快照、清单、阻塞与进度日志。
- 停止工作或更换模型/供应商前，更新 HANDOFF.md 的交接槽位。
- Actions 结果必须记录 run URL/ID、commit SHA、artifact 和结论；未实际运行不得写成通过。
