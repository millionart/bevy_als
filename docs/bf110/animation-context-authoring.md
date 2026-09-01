# BF110 结构化动画上下文作者合同

`tools/animgraph_contexts/ab_als.animcontext.json` 是 `AB_Als` 根图的 runtime animation-context 合同。它位于非运行时资源目录；实际 `.animgraph.ron` 直接使用 `als-ext` context node，不再依赖外部预览或展开图。

每个 context 只声明两个固定边界和一个开放集合：

- pose：source pose pin 到 target pose pin；
- time：独立的 source `NodeTime` 到 target time pin；
- curves：`inherit`，从 source 的已声明 F32 output 与 target 的已声明 F32 input 的命名交集自动发现。

`target_prefix` 说明 target 侧怎样命名继承通道。例如 source 的 `FootLeftLock` 与 target 的 `standing_FootLeftLock` 会自动构成一个继承项。合同不允许列出单独曲线字段；也不允许把 event、attribute 或控制输入塞入 context。

新增曲线时，只需在 source 图声明 F32 output，并在 consumer 图声明同名或带该 context prefix 的 F32 input。运行时 context node 会从 child graph ABI 自动枚举并注入该通道；不需要逐条手接 root edge。没有 consumer input 的 source output 不会被错误传播。

当前 Bevy 图运行时不支持通用 attribute payload，因此 `attributes` 和 `events` 在 BF110 中显式拒绝。它们保留为普通、独立的图语义，不能被伪装为曲线通道。

合同命令：

```text
python tools/apply_runtime_animation_context.py --self-test
python tools/apply_runtime_animation_context.py --manifest tools/animgraph_contexts/ab_als.animcontext.json --graph assets/graphs/ab_als/ab_als.animgraph.ron --check
```
