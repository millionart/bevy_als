# BF110 结构化动画上下文作者合同

`tools/animgraph_contexts/ab_als.animcontext.json` 是 `AB_Als` 根图的结构化连接合同。它位于非运行时资源目录；运行时继续加载标准 `.animgraph.ron`。

每个 context 只声明两个固定边界和一个开放集合：

- pose：source pose pin 到 target pose pin；
- time：独立的 source `NodeTime` 到 target time pin；
- curves：`inherit`，从 source 的已声明 F32 output 与 target 的已声明 F32 input 的命名交集自动发现。

`target_prefix` 说明 target 侧怎样命名继承通道。例如 source 的 `FootLeftLock` 与 target 的 `standing_FootLeftLock` 会自动构成一个继承项。合同不允许列出单独曲线字段；也不允许把 event、attribute 或控制输入塞入 context。

新增曲线时，只需在 source 图声明 F32 output，并在 consumer 图声明同名或带该 context prefix 的 F32 input。重新运行合同和后续展开器后，所有引用该 context 的边界都会发现并继承新通道；不需要逐条手接 root edge。没有 consumer input 的 source output 不会被错误传播。

当前 Bevy 图运行时不支持通用 attribute payload，因此 `attributes` 和 `events` 在 BF110 中显式拒绝。它们保留为普通、独立的图语义，不能被伪装为曲线通道。

合同命令：

```text
python tools/animgraph_context_contract.py --self-test
python tools/animgraph_context_contract.py --manifest tools/animgraph_contexts/ab_als.animcontext.json --baseline docs/bf110/ab_als.context-baseline.json --check
```
