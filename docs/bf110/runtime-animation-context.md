# BF110 Runtime Animation Context

`AB_Als` 直接使用 runtime animation-context node。每个 context consumer 在当前 `GraphContext` 的 source node/state key 中读取曲线状态；首层 source 从 node output cache 捕获，后续 source 从同一 context state 读取。该状态不会跨角色、图实例或 state key 共享。

实际根图保留 12 个运行节点和 13 条可见 edge。Standing/Crouching、Grounded、两个 Slot、Overlay/Inertialization、Layering、Head、ControlRig 与 Ragdolling 之间只保留参考图中的 Pose 主链；所有 `NodeTime` 请求在项目侧边界节点内从当前 `GraphContext` 获取并注入 child graph，不再形成根图 pin 或连线。

根图没有 `Inputs`，输出只保留 `pose` 与 Avian ragdoll runtime 固定读取的 `ragdoll_config`。最终 Ragdolling context 与 Inertialization signal context 在 `AnimationGraphSet::Final` 后投影为 `AlsRootAnimationContext`；Layering、Head、ControlRig、Foot IK 和真实入口验证都从该 state-key 隔离投影读取 F32 值，而不再依赖 root `Outputs` 的逐曲线 ABI。

链接图可声明 `input_aliases`，将 child graph 的技术性 pin 名映射为 ALS 原图可见名称，例如 `grounded_pose`→`Grounded Input`、`locomotion_pose`→`Locomotion Input`。Overlay 的 selector 与 reference pose 通过内部 global-data 输入注入，因此根图没有 selector/reference bridge 节点。

`ContextInput` 的 `curve_prefix` 与 child graph F32 input 匹配。新增 source output 和匹配 target input 后，node 会自动枚举、捕获并注入该曲线，不需要修改 root edge。`missing_curve_value: Some(0.0)` 只用于源图未提供、ALS 根图此前显式接零值的已验证通道。

验证命令：

```text
python tools/apply_runtime_animation_context.py --manifest tools/animgraph_contexts/ab_als.animcontext.json --graph assets/graphs/ab_als/ab_als.animgraph.ron --check
python tools/import/audit_ab_als_root.py --check
```
