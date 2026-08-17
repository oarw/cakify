# Cakify 数据与安全设计

> 状态：安全基线 v1
> 日期：2026-08-17
> 范围：Windows 本地客户端、模型 Provider、工具与 MCP

## 1. 安全目标与非目标

目标：

- API Key、OAuth refresh token 与 MCP secret 不以明文写入 SQLite、配置、日志或 artifact。
- 模型请求、工具调用和 MCP 数据流对用户可见、可取消、可审计。
- 默认最小权限；有副作用的工具在执行前需要明确确认。
- 应用退出或取消后不遗留工具/MCP 子进程。
- 数据删除、导出、备份和恢复有清晰语义。

非目标：

- 不声称能抵抗已控制同一 Windows 用户会话的恶意软件。
- 不提供企业 DLP、远程设备管理或系统级 sandbox。
- MVP 不做端到端同步、云端账号或远程操控。
- 不做知识库/RAG，因此不建立后台目录抓取、向量数据库或长期索引服务。

## 2. 威胁模型

首版重点处理：

- 仓库、日志、崩溃文件或导出包意外泄露 secret。
- Provider/MCP 配置把 secret 混入普通 JSON。
- 恶意或被攻陷的 MCP server 请求高风险操作。
- 模型生成的参数越权访问文件、命令、网络或超大输出。
- 用户取消后 child/grandchild process 继续运行。
- SQLite 活动库被错误复制、损坏或在迁移中半完成。
- Markdown/链接/附件导致路径穿越、任意协议打开或资源耗尽。
- Prompt injection 诱导工具越过用户授权；模型输出永远不视为可信指令。

信任边界：

- GPUI UI 是可信呈现层，但不得持久持有 secret plaintext。
- Core 可信，Provider response、MCP server、工具输出和附件均不可信。
- Credential Manager/DPAPI 依赖当前 Windows 用户安全边界。
- GitHub-hosted runner 不接触用户真实 Provider key 或签名证书，除受保护 release Environment 外。

## 3. SecretStore

Core 只依赖抽象：

~~~rust
pub trait SecretStore: Send + Sync {
    fn put(&self, id: &SecretId, value: SecretInput) -> Result<(), SecretError>;
    fn get(&self, id: &SecretId) -> Result<SecretValue, SecretError>;
    fn delete(&self, id: &SecretId) -> Result<(), SecretError>;
    fn contains(&self, id: &SecretId) -> Result<bool, SecretError>;
}
~~~

`SecretInput`/`SecretValue` 使用 `secrecy`/`zeroize` 类内存包装，不实现 `Debug`、`Display`、`Serialize` 或 `Clone`。Provider adapter 按请求短暂取得 secret；UI 只接收 `Missing / Configured / Error`。

### Credential Manager 主路径

- 类型：`CRED_TYPE_GENERIC`。
- TargetName：`Cakify/provider/{provider_uuid}/api-key`、`Cakify/mcp/{server_uuid}/{name}`。
- 持久级别：使用面向当前用户、跨登录会话但不漫游的 `CRED_PERSIST_LOCAL_MACHINE`；它仍属于当前用户 credential set，不能与 DPAPI 的 machine scope 混淆。
- SQLite 只保存 TargetName/opaque `credential_ref`，不保存 blob。
- 更新 Provider 时先写新 credential，再事务更新 reference；失败则回滚/清理孤儿。
- 删除 Provider 时删除 credential；删除失败显示可恢复警告并支持重试。
- 读取结果使用 `CredFree`，释放前清零应用创建的临时 buffer。

### DPAPI 后备路径

仅在需要保存一个不可拆分的结构化 token bundle、且 Credential Manager generic blob 语义不合适时使用：

- `CryptProtectData`/`CryptUnprotectData` 默认用户范围。
- 设置 `CRYPTPROTECT_UI_FORBIDDEN`，不使用 prompt UI。
- 禁止 `CRYPTPROTECT_LOCAL_MACHINE`。
- DPAPI ciphertext 可保存在单独 secret 文件；SQLite 仍只存 reference、版本和算法标识。
- `LocalFree` 释放系统输出；plaintext buffer 使用后清零。
- 如果保护/解保护失败，不允许静默回退到明文文件。

Credential Manager 可用时，不再额外用 DPAPI 包一层 API Key；多层加密并不会改善同用户恶意进程威胁，反而增加恢复失败面。

## 4. 本地文件布局

通过 Windows Known Folder API 获取路径，不手拼用户名：

~~~text
%LOCALAPPDATA%\Cakify\
  data\cakify.db
  data\cakify.db-wal
  data\cakify.db-shm
  attachments\<sha256>
  logs\cakify.log.*
  cache\models\...
  backups\...
~~~

目录继承当前用户 ACL。默认不把活动数据放 OneDrive、网络盘或 roaming profile。portable mode 和自定义数据目录延期，除非能同时定义 secret、锁、更新和卸载语义。

附件导入流程：

1. 解析 canonical path，拒绝目录、设备路径和超限文件。
2. 流式计算 SHA-256 与 MIME sniff，不信任扩展名。
3. 复制到临时文件，完成后原子 rename 到 content-addressed 路径。
4. SQLite 事务写入 metadata/reference。
5. 未被引用的临时文件和 blob 由显式 GC 清理。

首版大小限制建议：单附件 25 MiB、单消息总附件 50 MiB，可在 Provider 能力和用户设置中收紧。

## 5. SQLite 配置

数据库通过 `rusqlite` bundled SQLite 打开，storage actor 独占 writer connection。每次连接必须设置并校验：

~~~sql
PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA busy_timeout = 2500;
PRAGMA temp_store = MEMORY;
~~~

选择 `synchronous=NORMAL` 是性能/断电耐久性的明确权衡：SQLite 仍保持一致性，但突然断电可能丢失最近已确认事务。M2 必须做 crash/restart test；如果用户消息丢失窗口不可接受，改为 `FULL`，不要在文档外私自改变。

迁移规则：

- `schema_migrations(version, name, checksum, applied_at)`。
- 每个 migration 是不可变 SQL/Rust 文件，有前后 schema contract test。
- 升级前做一致性检查；迁移使用 transaction，失败不启动到半迁移状态。
- 只支持向前迁移；降级先导出兼容格式，不让旧版本直接打开新 schema。
- 启动时执行 `quick_check` 的频率和成本由 M2 benchmark 决定，异常时进入只读恢复页。

### 初始 schema

M1 建立：

- `schema_migrations`
- `app_settings`
- `provider_profiles`
- `provider_models`
- `conversations`
- `messages`
- `message_parts`
- `runs`
- `tool_calls`
- `permission_rules`
- `mcp_servers`
- `conversation_mcp_servers`
- `attachments`

约束：

- `provider_profiles.credential_ref` 唯一允许与 secret 关联的字段；无 `api_key`/`token` 列。
- message/runs/tool_calls 保留明确 foreign key 与 delete policy。
- Provider-specific metadata 可以放有 schema version 的 JSON；核心字段不可全部塞 JSON。
- tool arguments/result 是不可信 JSON，展示前限制深度/大小，日志不默认写全文。
- FTS5 只在轻量本地搜索 milestone 加入；不建立 embedding/向量表。

## 6. 备份、导出与删除

备份活动数据库必须使用 SQLite Online Backup API 或 `VACUUM INTO`，不能只复制 `.db` 而漏掉 WAL。

导出格式首版支持 Markdown 与版本化 JSON：

- 默认包含会话、消息、可选附件。
- 默认不包含 API Key、OAuth token、MCP secret、内部绝对路径或详细日志。
- 工具记录按用户选择包含，并标注可能有敏感输出。
- 导出到现有文件必须二次确认并使用原子替换。

删除语义：

- 删除会话先软删除到“最近删除”，默认 7 天后硬删除；可立即永久删除。
- 永久删除消息后重新计算附件引用，异步清理 orphan blob。
- 删除 Provider 同时尝试删除 Credential Manager entry。
- “清除所有本地数据”显示将删除的类别，关闭数据库后删除文件，并单独枚举/删除 Cakify credential namespace。
- 卸载器默认询问是否保留数据，不能悄悄删除历史。

## 7. Provider 与网络

- HTTPS 为默认；自定义 HTTP endpoint 需要显式风险确认，localhost 可单独处理。
- 不关闭 TLS 证书校验，不提供全局“忽略证书错误”。
- URL 解析使用结构化 URL API；重定向后重新检查 scheme/host，限制重定向次数。
- 默认 User-Agent 只含应用与版本，不含本地用户名、路径或会话内容。
- Authorization header 只在最终目标 origin 生成，不跨 origin 重定向转发。
- 请求超时、连接超时、最大响应 header/body、SSE 单事件大小均有上限。
- Provider base URL、模型 ID 和参数来自配置，但不能注入任意额外 secret header；自定义 header 的 secret value 也必须通过 SecretStore reference。

## 8. 工具与 MCP 权限

工具风险级别：

- `read-only`：读取用户明确选择范围内的数据，默认仍首次确认。
- `network`：向显示的域发送数据，展示域名与数据类别。
- `write`：创建/修改用户文件，始终确认，展示 canonical path。
- `process`：启动命令，始终确认，展示 executable、argv、cwd、env key names。
- `destructive`：删除、覆盖、系统配置等；MVP 内建工具不提供，后续默认 deny。

审批 UI 必须展示：工具来源（内建/MCP server）、参数摘要、访问范围、潜在副作用、是否会向网络发送内容。模型生成的“这是安全的”不能降低风险级别。

持久规则按精确 tool identity 和 server identity 保存。server 更新后 capability/schema hash 变化时，使旧 allow rule 失效并重新确认。

### 子进程治理

- `CreateProcessW` 后立即关联专用 Job Object。
- 设置 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`，默认禁止 breakaway。
- 为每次调用设置 wall-clock timeout、stdout/stderr byte limit 和并发上限。
- 使用 argv API；只有用户明确选择 shell tool 时才经过 `cmd`/PowerShell。
- cwd 必须是已授权路径；环境变量使用 allowlist，隐藏系统中无关 secret。
- 取消先发送协议级 cancel/关闭 stdin，短暂等待后终止 job。
- 工具输出视为不可信文本，限制 ANSI/控制字符和超长行。

这不是完整 sandbox。需要运行不可信代码时，应在后续接 Windows Sandbox/AppContainer 等独立方案，而不是用权限弹窗假装隔离。

## 9. Markdown、链接与附件呈现

- Markdown renderer 不执行 HTML script、事件处理器或远程嵌入。
- `file:`、自定义协议和可执行文件链接默认不直接打开。
- 外部链接显示目标域，交给 Windows 默认浏览器前确认高风险 scheme。
- 图片解码有像素、尺寸和内存上限；缩略图后台生成并缓存。
- 代码块只是文本；“运行”属于独立工具调用，不能由 Markdown 自动触发。
- 复制工具输出或路径不会隐式执行任何命令。

## 10. 日志、崩溃与遥测

日志字段在进入 formatter 前执行 denylist + allowlist 脱敏。错误类型可记录，原始 payload 不记录。开发 debug build 也遵守同一 secret redaction。

崩溃报告与遥测 MVP 默认关闭。未来若加入：

- 必须 opt-in，并在发送前说明字段。
- 不发送会话正文、附件、路径、工具参数/输出或 Provider endpoint query。
- 本地可预览和清除队列。
- 后端与隐私政策是独立里程碑，不能仅因 SDK 易接入就启用。

## 11. CI 安全门

GitHub Actions 需要覆盖：

- secret pattern 与高风险文件扩展名扫描。
- `cargo deny` 许可证/来源/重复依赖策略。
- `cargo audit` 或等价 RustSec 检查。
- Windows synthetic Credential Manager put/get/update/delete 测试，使用随机 TargetName 并在 `finally` 清理。
- DPAPI current-user round trip 与 tamper failure。
- SQLite migration、foreign key、crash recovery、backup/restore。
- Job Object child/grandchild cancel 与 app-exit cleanup。
- 导出包扫描，证明不含 fixture secret。

真实 secret 不进入普通 CI。签名证书只允许受保护 release Environment，最小权限、需要审批、日志 masking，且不向 fork/PR 暴露。

## 12. 安全验收条件

进入首个可分发 alpha 前必须全部满足：

- 仓库、构建产物、SQLite、日志和 JSON 导出中不存在测试 API Key 明文。
- Credential Manager/DPAPI 测试成功，失败时无明文 fallback。
- Provider 删除和“清除数据”能清理对应 credential。
- 工具默认 confirm，deny/cancel 在副作用前生效。
- MCP/工具进程取消与应用退出后无残留进程树。
- live database backup 可恢复并通过完整性检查。
- 任意崩溃恢复不会自动重复执行工具。
- 依赖许可证清单已生成，Zed GPL 业务代码未进入依赖图或源码。
