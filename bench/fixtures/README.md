# Benchmark fixture

`manifest.json` 是第一轮唯一的输入数据源。消息正文由 `cakify-bench-core` 的确定性算法按索引生成，因此不会把 10,000 条重复文本提交进仓库。

固定约束：

- `message_count=10000`，分页上限 `page_size=200`。
- 消息索引 42 带固定图片附件。
- 消息类型循环覆盖标题、列表、引用、表格、代码块、中文和普通文本。
- 流式场景发送 30 个 delta，每个间隔 1 秒；工具时间线发送 8 个阶段事件。

任何修改 fixture 都必须同时更新 `fixture_hash`、协议版本和进度文档，并在 Actions 中生成新的 artifact。
