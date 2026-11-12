# D232d 加性、差分与槽的双精度姿态

`AnimationRuntime.cpp:1109–1118,1150–1210,1263–1342` 的差分在对应的局部或组件旋转空间求逆相乘，平移作差，scale 乘参考倒数再减一。Mesh Apply 先转组件旋转，累积后逆序转回局部。 `TransformVectorized.h:1091–1105,1163–1203` 的满权重以 FMA 更新平移及 scale，部分权重先给 delta 加权。公共 API 和 Apply 节点分别在自己的末尾阶段归一化。

`AnimInstanceProxy.cpp:2016–2094` 使用 D169 的分母。Turn 依原 Montage 次序累积，最后加 Source pose。Transition 逐条应用网格加性，每条单独转换空间和归一化。保留有序源姿态及已资格权重直到实际槽消费。槽原曲线、根运动及不完整输入的运行路径沿用现有合同。

10 个 Local API 控制、474 对 Mesh API/节点骨骼、474 个 Local/Mesh 差分骨骼及 948 个 Turn/Transition 槽骨骼均与源逐值一致。提前投影在各活动域都有反例。Turn 预合并改变 79 个 double 与 74 个 f32 骨骼结果，Transition 预合并改变 474 个 double 与 10 个 f32 骨骼结果。差分采用新读回的同输入原生数据核算算子。实际共同 raw 参考资产选择由 D235 验收。

Lean 首次使用 Look 采样时刻超出 0.033333335 秒长度。使用本资产三个有效时刻重采通过。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/AB_Als_Head -MeshAdditiveProbe docs/audit/bf144/d231/head.request.json -Output docs/audit/bf144/d232d/mesh.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALSExtras/Environment/Meshes/SM_Als_Stairs_01 -GeometryQueryProbe docs/audit/bf144/d170/additive.request.json -Output docs/audit/bf144/d232d/local.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/View/A_Als_Look_Forward -SequencePoseProbe docs/audit/bf144/d232d/absolute.request.json -Output docs/audit/bf144/d232d/look-absolute.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/View/A_Als_Look_Forward -SequencePoseProbe docs/audit/bf144/d232d/delta.request.json -Output docs/audit/bf144/d232d/look-delta.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Base/A_Als_Stand_Pose -SequencePoseProbe docs/audit/bf144/d232d/reference.request.json -Output docs/audit/bf144/d232d/stand-reference.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Grounded/Lean/A_Als_Lean_Pose -SequencePoseProbe docs/audit/bf144/d232d/reference.request.json -Output docs/audit/bf144/d232d/lean-reference.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Grounded/Lean/A_Als_Lean_Forward -SequencePoseProbe docs/audit/bf144/d232d/lean-absolute.request.json -Output docs/audit/bf144/d232d/lean-absolute.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Grounded/Lean/A_Als_Lean_Forward -SequencePoseProbe docs/audit/bf144/d232d/lean-delta.request.json -Output docs/audit/bf144/d232d/lean-delta.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/Stances/AB_Als_Standing -SlotProbe docs/audit/bf144/d169/standing-turn.request.json -Output docs/audit/bf144/d232d/turn.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AB_Als -SlotProbe docs/audit/bf144/d169/transition.request.json -Output docs/audit/bf144/d232d/transition.native.json
python docs/audit/bf144/d232d/replay_source_additive.py
```

Rust fixture 复现原生 Turn 第二帧 pelvis 源 Z 平移，随统一验证运行。
