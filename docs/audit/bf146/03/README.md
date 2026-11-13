# BF146.03 地面探针与空中状态调度

Tnua 的 Sensors 读取角色上的探针形状。目标把形状更新放在同一 `Update` 的 Sensors 之前。`AlsAirState` 要求碰撞前速度组件，安装 `AlsAirPlugin` 的角色直接拥有两者。速度采样在出生模式初始化后、探测之前。落地状态在 `PostUpdate` 的物理与速度提交后更新，使用当前变换和 `Time<Virtual>`。

ALS 角色的落地回调先用上次保留的角色速度选动作，再刷新角色运动状态。落地消息驱动的 Roll 因此保留原来消费阶段。`AlsAirState` 的落地观察发生在后续阶段，与源角色回调使用的速度各有用途。

[原生取证与源代码](source-facts.json) · [最小失败](baseline.json) · [调度和动作定向测试](focused.json)

```sh
CARGO_TARGET_DIR=target_game cargo test --locked --profile play --jobs 2 --lib -- playable_character_samples_air_velocity_and_refreshes_the_sensor_before_tnua air_state_samples_on_spawn_and_observes_landing_in_the_same_frame landing_roll_uses_the_retained_character_velocity
pwsh -NoProfile -File tools/run_animation_visual_validation.ps1 -Case uphill_jump -FrameDeltaSeconds 0.025 -CaptureCount 0 -FunctionalOnly
```
