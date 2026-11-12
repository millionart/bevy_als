# D254a 角色时钟复验

真实 Playground 第99行的计划速度为 `(0.6666666865, 0, -0.0000509129641)` m/s，与原生逐值相同。静止支撑、单CMC步、计划位移和实际终点都满足普通路径。角色时钟为33,333,335 ns，物理时钟为33,333,333 ns，原守卫回落到F32世界位置差分。

ALS `PhysWalking` 的分母来自角色 `timeTick`。按 `Time<Virtual>` 核计划资格后，分离时钟反例及静止、阻挡、自定义时钟、移动支撑等四项定向通过。真实319帧复验中，首移动速度和源Speed投影精确，实际终点逐值一致，首移动 `LayerLegs` 恢复。完整前后数据及其它余差见 `docs/audit/bf144/u07/source-root-history/`。

```sh
cargo test --profile play --lib --jobs 2 -- free_walking_snapshot_preserves_plan_at_nonzero_world_origin actual_velocity_snapshot_retains_full_velocity_and_horizontal_speed free_walking_keeps_the_planned_velocity_before_physics single_walking_step_keeps_penetration_velocity_after_physics
```
