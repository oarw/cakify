# Cakify 调研来源与许可证边界

> 调研快照：2026-08-17（Asia/Shanghai）
> 用途：记录影响产品架构的上游事实、版本快照和可复核链接。版本号只是调研时状态，进入实现时仍须锁定并由 Actions 验证。

## 1. GPUI 与 Zed

### GPUI

- [GPUI README](https://github.com/zed-industries/zed/tree/main/crates/gpui)：GPUI 是 Rust 的 GPU 加速 UI 框架，采用混合 immediate/retained 模型；Windows 使用 Win32 与 DirectWrite。
- [GPUI Apache-2.0 许可证](https://github.com/zed-industries/zed/blob/main/crates/gpui/LICENSE-APACHE)：允许在 Cakify 中按 Apache-2.0 条件使用。
- [GPUI 输入示例](https://github.com/zed-industries/zed/blob/main/crates/gpui/examples/input.rs)：展示 `EntityInputHandler`、选择区间、marked/composition range、UTF-8/UTF-16 转换与坐标命中。结论是 GPUI 有 IME 所需底层接口，但 Cakify 仍必须实现并验证真实多行中文输入。
- [GPUI 测试示例](https://github.com/zed-industries/zed/blob/main/crates/gpui/examples/testing.rs)：支持 `#[gpui::test]`、测试上下文、action 分发与受控异步执行。
- 第一轮已验证的固定 Zed revision：`b2d9c2e122fbc408d42276b4456243ba4f90f181`。产品 M0 先以此为保守起点；升级必须走独立 PR 和回归门。

GPUI 仍是 pre-1.0，上游明确提示 API 可能频繁变化。因此不得依赖浮动 `main`，也不允许在普通功能 PR 顺手升级 GPUI。

### Zed 的 AI 对话

- [Zed Agent Panel 文档](https://zed.dev/docs/ai/agent-panel)：成熟行为包括多线程、编辑/排队消息、上下文、模型切换、工具与 MCP、上下文压缩、通知和导出。
- [Zed 工具权限文档](https://zed.dev/docs/ai/tool-permissions)：提供 allow once、deny once、按工具/模式持久授权与内置危险规则。
- [Zed MCP 文档](https://zed.dev/docs/ai/mcp)：展示本地/远程 MCP、启停、错误反馈和工具权限的产品化方式。
- [Zed `agent` crate](https://github.com/zed-industries/zed/tree/main/crates/agent) 与 [`agent_ui` crate](https://github.com/zed-industries/zed/tree/main/crates/agent_ui) 均声明 `GPL-3.0-or-later`。

许可证边界：可以学习公开产品行为、状态划分和 GPUI API 用法；不复制 `agent`、`agent_ui`、`language_model`、`context_server` 等 GPL 业务实现、测试夹具或成段结构。Cakify 独立设计自己的协议、状态机、数据库和 UI。若未来决定采用 GPL 代码，必须先由用户明确决定项目许可证并做完整依赖审计。

### gpui-component

- [项目 README](https://github.com/longbridge/gpui-component)：Apache-2.0，提供输入、textarea、虚拟列表、Markdown、主题和 60+ 桌面组件。
- [许可证](https://github.com/longbridge/gpui-component/blob/main/LICENSE-APACHE)：Apache-2.0。
- 调研 commit：`81305ef4a0fd86f64777791dd38ead5c303a15f4`。
- 调研时发布版：`gpui-component 0.5.1`；仓库 workspace 显示 `0.5.2` 开发中。

风险：其 workspace 对 GPUI 使用 Git 依赖且未固定 revision，和 Cakify 的已验证 GPUI revision 未证明兼容。M0 只做一个隔离 spike：比较直接使用 GPUI primitives、只用 `gpui-base`、使用完整 `gpui-component` 三种路径；未通过依赖锁定、IME、Markdown 流式渲染和二进制体积门之前，不进入产品主依赖。

## 2. Windows 秘密存储

- [CredWriteW](https://learn.microsoft.com/windows/win32/api/wincred/nf-wincred-credwritew)、[CredReadW](https://learn.microsoft.com/windows/win32/api/wincred/nf-wincred-credreadw)、[CredDeleteW](https://learn.microsoft.com/windows/win32/api/wincred/nf-wincred-creddeletew)：在当前用户 credential set 中创建、读取和删除凭据。
- [CREDENTIALW](https://learn.microsoft.com/windows/win32/api/wincred/ns-wincred-credentialw)：`CRED_TYPE_GENERIC` 允许应用保存自己的二进制 secret，并以 TargetName + Type 唯一定位。
- [CryptProtectData](https://learn.microsoft.com/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata) 与 [CryptUnprotectData](https://learn.microsoft.com/windows/win32/api/dpapi/nf-dpapi-cryptunprotectdata)：默认绑定同一登录用户，提供完整性校验。
- [CredFree](https://learn.microsoft.com/windows/win32/api/wincred/nf-wincred-credfree) 与 [LocalFree](https://learn.microsoft.com/windows/win32/api/winbase/nf-winbase-localfree)：分别释放 Credential Manager 和 DPAPI 返回的系统分配块。
- [MoveFileExW](https://learn.microsoft.com/windows/win32/api/winbase/nf-winbase-movefileexw)：同目录替换使用 `MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH`，避免正常更新过程中暴露半写密文文件。
- [Microsoft 密码处理建议](https://learn.microsoft.com/windows/win32/secbp/handling-passwords)：建议使用 CredWrite/CredRead 保存操作系统凭据，并及时清除敏感内存。

决定：API Key 和 OAuth refresh token 优先用 Credential Manager 的 generic credential；SQLite 只保存 opaque reference。DPAPI 只处理 Credential Manager 不适合的结构化 secret，使用用户范围、禁止交互 UI；绝不设置 `CRYPTPROTECT_LOCAL_MACHINE`，因为那会允许本机其他用户解密。

## 3. SQLite

- [Write-Ahead Logging](https://sqlite.org/wal.html)：WAL 允许 reader 与 writer 并行，但同一时刻仍只有一个 writer；WAL/SHM 是活动数据库状态的一部分。
- [PRAGMA 文档](https://sqlite.org/pragma.html)：`foreign_keys` 是连接级设置，`journal_mode=WAL` 持久生效，`synchronous` 决定断电耐久性权衡。
- [Online Backup API](https://sqlite.org/backup.html)：运行中数据库应通过 backup API 或 `VACUUM INTO` 备份，不直接复制主文件。
- [FTS5](https://sqlite.org/fts5.html)：后续本地全文搜索的候选，不进入第一条纵向切片。
- [rusqlite](https://github.com/rusqlite/rusqlite)：调研时最新 crates.io 版本为 `0.40.2`。

决定：`rusqlite` + bundled SQLite，单独 storage actor 持有连接；启用 WAL、foreign keys、busy timeout 和迁移事务。数据库只放本地磁盘，不支持把活动库放网络盘/同步盘。

## 4. MCP 与工具进程

- [MCP 规范](https://modelcontextprotocol.io/specification/2026-07-28)：基于 JSON-RPC 2.0，定义 capabilities、tools、resources、prompts、进度、取消与错误；强调用户同意和数据边界。
- [官方 Rust SDK](https://github.com/modelcontextprotocol/rust-sdk)：crate 为 `rmcp`，调研时 crates.io 最新版 `3.1.2`，支持 stdio 与 Streamable HTTP。
- [Windows Job Objects](https://learn.microsoft.com/windows/win32/procthread/job-objects)：可以把进程组作为整体管理；`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` 可在最后一个 job handle 关闭时终止关联进程。

决定：只支持当前 stdio 和 Streamable HTTP，不为旧双端点 HTTP+SSE 新写兼容层。所有本地 MCP/工具子进程进入 Job Object，具有超时、取消、输出上限和退出清理；工具默认需要确认。

## 5. Windows 打包与发布

- [MSIX 文档](https://learn.microsoft.com/windows/msix/)：Windows 的现代应用包格式。
- [Windows 应用代码签名选项](https://learn.microsoft.com/windows/apps/package-and-deploy/code-signing-options)：商店发布可由 Microsoft 重签；独立分发需要规划证书与信誉。

路线：开发阶段先发 portable ZIP，减少打包变量；功能和更新通道稳定后再加入签名 MSIX/安装器。签名密钥只能进入 GitHub Environment secret，fork/PR 工作流不得访问。

## 6. 版本锁定规则

- 所有 Git 依赖固定完整 commit SHA。
- crates.io 依赖提交 `Cargo.lock`，workspace 依赖统一声明，不使用 `*`。
- 每月单独开 dependency-refresh PR；GPUI 升级永远独立处理。
- 新依赖先核对许可证、维护活跃度、默认 feature、native/runtime 体积和安全公告。
- 文档中的“调研时最新”不是自动升级指令；Actions 通过的 lockfile 才是产品事实。
