# D163a 原生 Rotator 保留有符号半转

`UnrealMath.cpp:691–718` 在源四元数基中计算 double Rotator，直接保留 atan2 的 ±180° 端点。`AlsRigUnit_ApplyFootOffsetRotation.cpp:60–89` 用当前角度扩展目标角限制。当前端点的符号会改变限制区间。

原生重采 21 组精确半转、半转邻域、奇异与普通角度。报告的 JSON 将有符号零显示为 0，重放按名称使用原请求的零符号。旧代数改写在四组精确半转选错端点。代表 Roll 从源 −180° 变为 +180°，以旧端点约束 −177.4759° 会得到 0°。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALSExtras/Environment/Meshes/SM_Als_Stairs_01 -GeometryQueryProbe docs/audit/bf144/d163a/rotator.request.json -Output docs/audit/bf144/d163a/rotator.native.json
python docs/audit/bf144/d163a/replay_rotator.py
```

`control_rig_euler_keeps_the_native_signed_half_turns` 使用源报告中的角度对照。D163b 接续小腿空间的四元数组合。
