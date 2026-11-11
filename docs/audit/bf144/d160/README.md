# D160 Rig 射线的初始重叠

`SQVisitor.h:29–38` 的零距离 ray 缺少位置和法线标志。`CollisionConversions.cpp:373–421` 用射线起点及反向射线的安全归一化法线补足。`AlsRigUnit_FootOffsetTrace.cpp:24–26,39–59` 读取阻挡命中并按法线计算脚底偏移。

原生盒内和起点贴盒面均产生距离 0、raw normal 0。源转换后朝下的射线得到向上法线，偏移为 +0.5 m。斜向控制按射线反方向产生法线，正距离控制保留形体表面给出的法线。恢复直接转发 raw normal 时，三个零距离组都被坡度判定剔除。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALSExtras/Environment/Meshes/SM_Als_Stairs_01 -GeometryQueryProbe docs/audit/bf144/d160/overlap.request.json -Output docs/audit/bf144/d160/overlap.native.json
python docs/audit/bf144/d160/replay_initial_overlap.py
```

`initial_box_overlap_reaches_the_rig_foot_offset` 通过实际 Rig 查询进入 FootOffsetTrace。Rust、消融恢复和 Playground Gate 待统一验证。
