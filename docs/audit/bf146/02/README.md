# BF146.02 Tnua 配置归属

源角色把 `MS_Als_Normal` 引用交给 CMC。每个移动组件持有自己的 `GaitSettings` 和 `MaxWalkSpeed`。目标的静态 `AlsMovementParams` 仍可共享。Bundle 构造器接收配置值和 `Assets<AlsControlSchemeConfig>`，为每个实体创建独立的强 handle，随后由原 Tnua 同步和控制阶段消费。

两个角色共用运动设置，分别用 1× / 2× 缩放和 1× / 0.5× 重力。反例读取实际配置中的浮空高度和跳跃高度。消融将 Bundle 中的 `configs.add(config)` 改成先取 `configs.ids().next()`，有已有资产时返回 `configs.get_strong_handle(id)`，否则新建。两角色再次读到同一对数值。恢复构造器源码字节后在原 target 重验。

```sh
CARGO_TARGET_DIR=target_game cargo check --locked --profile play --jobs 2 --all-targets
CARGO_TARGET_DIR=target_game cargo test --locked --profile play --jobs 2 --lib characters_sharing_movement_settings_use_independent_tnua_configs
pwsh -NoProfile -File tools/run_animation_visual_validation.ps1 -Case uphill_jump -FrameDeltaSeconds 0.025 -CaptureCount 0 -FunctionalOnly
```

[源设置与代码](source-facts.json) · [基线](baseline.json) · [消融](ablation.json) · [调用点编译](all-callers.json) · [定向测试](focused.json)
