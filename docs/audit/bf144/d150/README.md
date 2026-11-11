# D150 High Mantle 使用 Custom 制动上限

`AlsCharacter_Actions.cpp:496–497,569` 先切到并锁定 MOVE_Custom，再播放 Mantle。`CharacterMovementComponent.cpp:4351–4368` 对 Custom 返回制动 0。`AlsAnimationInstance.cpp:703–710,829–847,903–918` 把它用于负点积时的 Grounded lean，非负点积仍走最大加速度。

原生 High Montage 的 PostLocomotion 组混入为 0.2 秒 HermiteCubic。第 2 帧 slot 权重为 0.07407408。`AnimNode_Slot.cpp:79–85` 仍更新剩余权重的地面姿态。原生 Standing 的 BS_Als_Lean 绑定父实例 LeanState.ForwardAmount。静态消融恢复 Walking 制动时，只有负点积帧的目标 lean 和插值变化。默认半衰期取自 [AIS_Als_Default 原生报告](../d147/settings.native.json)。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Actions/Mantle/AM_Als_Mantle_High -MontageClockProbe docs/audit/bf144/d150/mantle.request.json -Output docs/audit/bf144/d150/mantle.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/Stances/AB_Als_Standing -Output docs/audit/bf144/d150/standing.native.json
gzip -dc docs/audit/bf144/d150/standing.native.json.gz | shasum -a 256
python docs/audit/bf144/d150/replay_custom_braking.py
```

Rust 反例 `high_mantle_grounded_lean_uses_custom_braking_limit` 核对实际角色处于 Custom、部分 slot 权重及当帧 Grounded 回调结果。Rust、消融恢复和 Playground Gate 待统一验证。
