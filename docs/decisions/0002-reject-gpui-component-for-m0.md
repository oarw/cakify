# ADR 0002：M0 不引入 gpui-component

- 状态：Rejected for M0
- 日期：2026-08-17
- 调研对象：`longbridge/gpui-component` commit `81305ef4a0fd86f64777791dd38ead5c303a15f4`

## 背景

`gpui-component` 提供 textarea、Markdown、virtual list、主题和大量桌面组件，许可证字段为 Apache-2.0，功能方向符合 Cakify。它也是一个很有价值的交互和 API 参考。

但 Cakify 必须固定已经跑过 benchmark 的 GPUI revision，不能让产品构建随 Zed HEAD 漂移。静态核对发现：

- 该调研 commit 的 workspace 对 `gpui`、`gpui_platform` 使用 Zed Git 依赖，但没有声明 revision。
- 其提交的 `Cargo.lock` 把 `gpui` 解析到 `cc053a4a6fa2fd0e8793201ed9099466af1be0b1`。
- Cakify 固定的 `b2d9c2e122fbc408d42276b4456243ba4f90f181` 比该 revision 向前 88 个 Zed 提交。
- 完整 `gpui-component` 还会引入 Markdown/HTML parser、编辑器和其他当前空壳不需要的依赖，体积影响尚未在同一 pin 下测量。

因此目前无法证明它与 Cakify 的 GPUI 类型、输入接口和单 revision 依赖图兼容。这个兼容性门在运行 textarea/IME、Markdown streaming 和体积测试之前就已经失败。

## 决定

- M0 产品 workspace 不依赖 `gpui-component` 或 `gpui-base`。
- 首版 UI 使用直接 GPUI primitives，并在 Cakify 内部只创建聊天所需的少量组件。
- 可以学习其公开交互和 Apache-2.0 API 设计，但当前不复制、vendor 或 fork 组件源码。
- 不把尚未运行的微软拼音、Markdown streaming、virtual list 或二进制体积测试写成通过。

## 重新评估条件

只有同时满足以下条件才重新开启采用评估：

1. 上游发布版或指定 commit 能和 Cakify 使用完全相同的 Zed revision，依赖树只有一份 `gpui`/`gpui_platform`。
2. 手动 Actions 能编译最小 textarea + Markdown + virtual list 程序，并输出锁文件、依赖树和 release EXE 体积。
3. 物理 Windows 机器验证微软拼音 composition、候选窗位置、焦点恢复和高 DPI。
4. 相比直接 primitives 的启动、内存、体积和维护收益足够明确。

在条件满足前，M1/M2 不等待该组件库，继续直接实现产品纵向切片。
