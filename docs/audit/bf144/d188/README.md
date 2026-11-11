# D188 Roll 提前 TickPose 的动画输入

fresh 角色 CDO 的 CMC `bTickBeforeOwner=True` 对应 UE `MovementComponent.cpp:186–188`。ALS `AlsCharacter.cpp:118–128,274–291` 让 Mesh 依赖角色并在角色 Tick 刷新运动缓存。活动根运动 Roll 的 `CharacterMovementComponent.cpp:2920–2935,12027–12065` 在物理前推进姿态，ALS `AlsAnimationInstance.cpp:687–716` 从此前的角色缓存读取速度和输入。已确认 Roll 序列的原生根运动标志。

fresh 蹲姿图从 `GetParent.LocomotionState.bMovingSmooth` 取 Move 条件，`AlsAnimationInstance.cpp:1084–1097` 从同一速度计算 stride 和 play rate。Roll 槽准备在物理前采集已缓存的动画速度与输入，与物理完成后的 `AlsActualMotion` 并存。自动混出窗口、显式中断、Ragdoll 和低于 CMC 最小步长的帧按组件时点发布当前速度。

30Hz 清单反例由 0.471937m/s 变为本帧 1.582839m/s，旧路径提前置 MovingSmooth。60Hz 受控速度对照产生 1.285530 和 1.499798 的蹲姿速率。取消快照时这两组失配；在淡出、零步长或普通运动仍沿用快照也会失配。Rust fixture 经真实角色入口比较 30/60Hz 的物理前缓存、图输入、蹲姿速率与淡出切换。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/B_Als_Character -Output docs/audit/bf144/d188/character.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/Stances/AB_Als_Crouching -Output docs/audit/bf144/d188/crouching.native.json
python docs/audit/bf144/d188/replay_animation_speed.py
```
