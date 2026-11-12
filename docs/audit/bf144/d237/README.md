# D237 View 的相对 yaw 精度

`RefreshGroundedMovement` 将源 float VelocityYaw 提升后减 double ViewYaw。 `RefreshView` 用 double ViewYaw 与 BodyYaw 求差。两者都在求差后转 float，再调用 `UnwindDegrees`。

Movement、Head 和可见 Playground 的转角检查在 radians 输入上消费同一精度阶段。Head 保留 D258 的正半转规范表示。现有 yaw speed 使用公开 yaw 字段。

323 个原生同输入控制逐值精确。Head 的第79例从 44.999996185302734° 恢复为 45°，其余控制合计319个角度受早投影影响。Movement 的正负边界为 ±75° 与 ±115°，提前转 float 使四个方向分类改变。Rust fixtures 覆盖 Head 反例、四个 Movement 分类及原半转。

```sh
python docs/audit/bf144/d237/replay_view_yaw.py --request
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALSExtras/Environment/Meshes/SM_Als_Stairs_01 -GeometryQueryProbe docs/audit/bf144/d237/yaw.request.json -Output docs/audit/bf144/d237/yaw.native.json
python docs/audit/bf144/d237/replay_view_yaw.py
```
