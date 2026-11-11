# D163d Head 加性姿态的单精度旋转乘法

`AnimationRuntime.cpp:1150–1210,1290–1342` 依次组合父组件和局部旋转、应用 mesh-space additive，再按父组件逆旋转还原局部。`TransformVectorized.h` 的 `AccumulateWithAdditiveScale` 与 `BlendFromIdentityAndAccumulate` 用同一个 `VectorQuaternionMultiply2` 左乘加性旋转。三处复用 D163c 已对照 `FQuat4f` 的源基累加。共用加性 helper 的局部与 root-motion 调用也使用该乘法。各处权重、F32 精度及归一化阶段仍沿各自调用。

重采实际 `AB_Als_Head` 节点的 `000:cc` 和 `000:ss`，两份原生姿态与封存记录逐值一致。`.454` 的乘法单因素及删除控制覆盖 318 帧。在 `000:cc`，`VB hand_l_to_ik_hand_gun` 的 y 源投影及仅顺序控制均为 −0.6696646809577942，旧分组为 −0.669664740562439。这个历史对照固定当时的其余阶段。新增 Rust fixture 直接运行公开加性入口，使用已对照原生的组件积和受控半权重加性旋转。D248b1 的原方法隔离测试继续用封存的旧乘积分组。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/AB_Als_Head -MeshAdditiveProbe docs/audit/bf144/d248b1/head.request.json -Output docs/audit/bf144/d163d/head.native.json
python docs/audit/bf144/d163d/compare_head.py
```
