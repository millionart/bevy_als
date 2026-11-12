# D232e Control Rig 的厘米姿态与最终投影

原生 Control Rig 适配器在 CR_Als 的实际节点使用 local pose transfer。 `AnimNode_ControlRigBase.cpp:130–169` 转发局部姿态及曲线。 `ControlRigPoseAdapter.cpp:386–484` 按需从缓存求 local/global，经过相应的 parent 变换和归一化。

从新读回的 CR_Als CDO 和 VM 生成厘米 double 定义。保留原运行坐标轴映射，骨骼位置、长度向量、数值容差及 VM 消费同一单位。VM 内部标量转向量保持厘米，输入物理向量和可见输出各在实际边界换算。Rig 追踪的网格核使用源厘米点，物理 Collider 接口消费米。足部诊断先在原父空间计算，再投影位置。米制定义继续通过默认单位装入。

六组真实 Head 输出的 474 骨在原生适配器输入和输出逐值保持，提前降精度会改变 391 条输入。三段真实或受控历史共24帧，其中21帧进入两侧脚部 setter，最终原始缓存闭合 1659 组 local/global。提前换米改变126条 local double 平移，其中83条改变最终 f32。无观测对照完整骨骼、曲线和 VM 状态一致。独立进程的 FCachedRigElement ContainerVersion 只用于本进程的容器身份。

Rust fixtures 覆盖实际 local 传递、VM 长度字段写入和生成定义的反射加载，随统一验证运行。

全量重转 15 个安装根和递归依赖，对照 374 个产物。Rifle 根首次原生读取失败，隔离重读通过并与本轮其余结果一起重建完整 diff。唯一差异为主图的 Rig 定义，与独立编译结果逐值一致。

```sh
python docs/audit/bf144/d232e/replay_rig_pose.py --requests
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AB_Als -RigPoseTransferProbe docs/audit/bf144/d232e/transfer.request.json -Output docs/audit/bf144/d232e/transfer.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AB_Als -RigNodeHistoryProbe docs/audit/bf144/d232e/history-before.request.json -Output docs/audit/bf144/d232e/history-before.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AB_Als -RigNodeHistoryProbe docs/audit/bf144/d232e/history.request.json -Output docs/audit/bf144/d232e/history.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/CR_Als -Output docs/audit/bf144/d232e/rig.native.json
python tools/animgraph_authoring/test_ue_control_rig_target.py --native-fixture docs/audit/bf144/d232e/rig.native.json --root-fixture docs/audit/bf144/d232e/transfer.native.json --output docs/audit/bf144/d232e/rig.centimeters.json
python tools/animgraph_authoring/regenerate_installed_corpus.py --content-root "$ALS_CONTENT_ROOT" --output target/d232e-corpus
python docs/audit/bf144/d232e/replay_rig_pose.py
```
