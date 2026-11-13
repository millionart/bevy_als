# BF146.06 CMC 帧结果

UE CMC 在物理前应用根运动，Falling 保留物理竖直轴，求解结束更新组件速度。ALS 角色随后刷新速度，动画从角色读取。目标用 `AlsCmcFrameResult` 保存当前碰撞速度、模式、空中扫掠、动画根运动与 Mantle 交接标记。Mantle 可在物理前取走待用速度。物理后 `locomotion_state_sync_system` 同时取出并清除余下结果；速度快照沿已有阶段读取，保留单步自由移动和基座速度语义。

原 `AlsCmcAdapterRuntime` 仍包含探针数据、模拟统计、输入缓存与移动计划。新的反例从真实速度快照读取结果，确认提交只发生一次，并检查不再把该写入标记到 CMC 运行参数。原根运动保留物理轴、碰撞模式、速度与场景回归分别经过定向和公开 Gate。

[源 CMC 与 ALS 快照证据](source-facts.json) · [失败反例](baseline.json)

```sh
CARGO_TARGET_DIR=target_game cargo test --locked --profile play --jobs 2 --lib -- movement_results_do_not_mark_sensor_configuration_changed animation_root_velocity_restoration_preserves_falling_physics_axis single_walking_step_keeps_penetration_velocity_after_physics
pwsh -NoProfile -File tools/run_animation_visual_validation.ps1 -Case moving_base -CaptureCount 0 -FunctionalOnly
pwsh -NoProfile -File tools/run_animation_visual_validation.ps1 -Case grounded -CaptureCount 0 -FunctionalOnly
```

历史审计脚本由其对应提交复现。接到当前版本时，从 `bevy_als::als::motion::AlsCmcFrameResult` 读取上述碰撞与根运动结果。

全量串行回归 2,155 单元、37 集成通过。并行回归曾在 Quick Stop 夹具的相机装配阶段报告 `GlobalInputDataMissing("locomotion_action")`，同二进制定向与串行复验通过，原因未确定。公开 startup、moving_base、grounded 功能验收通过。 [消融](ablation.json) · [全量回归](full-regression.json) · [并行记录](parallel-quick-stop.json) · [公开验收](runtime.json)
