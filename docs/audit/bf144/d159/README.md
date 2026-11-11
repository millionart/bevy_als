# D159 Rig 射线的端点资格

`ChaosCore/Private/Chaos/Raycasts.cpp:79–84` 的箱体 ray 接受达到长度的有效命中。`Triangle.h:390–398` 与 `TriangleMeshImplicitObject.cpp:948–955` 对三角形内核使用相同边界。`ChaosInterfaceUtils.cpp:252–281,298–317` 为带尺度的凸包和网格选择缩放包装。缩放包装用 `ImplicitObjectScaled.h:648–665` 的 NewTime 严格小于长度判断。

实际 Stairs 顶面三角形来自原生 cooked 数据。箱体接收 162.5 cm 的端点。128 cm 用例中，单位和近单位尺度 Stairs 接收端点。尺度 2 的 Stairs 内核命中 64 cm，外层拒绝恰好等于世界长度的结果。100 cm 控制保留了浮点边界差异。单位网格的三角形内核在该长度未返回命中，1.25 倍的外层计算距离略小于长度并返回命中。端点资格以计算的命中时间为准。

现有 F64 网格遍历的 AABB 与单三角形调用已允许内核返回端点。外层和复合子形体接收第一个有效端点，并在已有命中时只取更近的结果。按原始形体类型和应用尺度识别缩放包装。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALSExtras/Environment/Meshes/SM_Als_Stairs_01 -GeometryQueryProbe docs/audit/bf144/d159/scan.request.json -Output docs/audit/bf144/d159/scan.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALSExtras/Environment/Meshes/SM_Als_Stairs_01 -GeometryQueryProbe docs/audit/bf144/d159/endpoint.request.json -Output docs/audit/bf144/d159/endpoint.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALSExtras/Environment/Meshes/SM_Als_Stairs_01 -GeometryQueryProbe docs/audit/bf144/d159/computed-limit.request.json -Output docs/audit/bf144/d159/computed-limit.native.json
python docs/audit/bf144/d159/replay_ray_limit.py
```

原生初次扫描位于网格范围外。其 cooked 三角形导出确定后续射线的位置。`first_endpoint_hit_respects_the_source_shape_wrapper` 覆盖箱体、复合箱体和三角形查询。`exact_box_endpoint_reaches_foot_offset_trace` 在 Rig scale 1.25 时核对实际 offset。Rust、消融恢复和 Playground Gate 待统一验证。
