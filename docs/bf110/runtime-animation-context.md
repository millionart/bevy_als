# BF110 Runtime Animation Context

`AB_Als` 直接使用 runtime animation-context node。每个 context consumer 在当前 `GraphContext` 的 source node/state key 中读取曲线状态；首层 source 从 node output cache 捕获，后续 source 从同一 context state 读取。该状态不会跨角色、图实例或 state key 共享。

实际根图保留 16 个节点和 94 条 edge。Standing/Crouching、Grounded、两个 Slot、Overlay/Inertialization、Layering、Head、ControlRig 与 Ragdolling 之间只保留 Pose、必要 Time 和独立控制边；254 条逐曲线／零值 bridge edge 已删除。

`ContextInput` 的 `curve_prefix` 与 child graph F32 input 匹配。新增 source output 和匹配 target input 后，node 会自动枚举、捕获并注入该曲线，不需要修改 root edge。`missing_curve_value: Some(0.0)` 只用于源图未提供、ALS 根图此前显式接零值的已验证通道。

验证命令：

```text
python tools/apply_runtime_animation_context.py --manifest tools/animgraph_contexts/ab_als.animcontext.json --graph assets/graphs/ab_als/ab_als.animgraph.ron --check
python tools/import/audit_ab_als_root.py --check
```
