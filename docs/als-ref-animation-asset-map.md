# ALS-Ref 动画资产对照表

本表用于定位 ALS-Refactored 的每个 BlendSpace、AnimBlueprint 与 AnimMontage 在当前项目中的对标资产、图或运行时实现。左列使用 Unreal package path，右列使用仓库相对路径。

- 源盘点日期：2026-08-31。
- 源盘点方法：通过本机 ALS-Refactored 工程的 Unreal AssetTools 递归查询 `/ALS`；
- 盘点结果：9 个 BlendSpace、26 个 AnimBlueprint、9 个 AnimMontage。
- 列出目标文件仅表示已有显式资产、图、参数表或运行时替代物；不表示当前 CHECKLIST 模块已经通过行为 Gate。
- “仅导出”表示保留了源序列／notify 数据，但当前没有图或运行时 selector 播放它。“明确排除”表示该源资产有意不作为目标架构。

播放器、循环、混合与资产的细项审计见 `bf100-24-animation-audit.md`。

## BlendSpaces (9)

| ALS-Ref package                                                  | Current exported definition                                                         | Current graph consumer                                                                                                                                          | 等价度 |
| ---------------------------------------------------------------- | ----------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- | --- |
| `/ALS/Animations/Air/Lean/BS_Als_Air_Lean`                       | `assets/animations/blendspaces/Air/Lean/BS_Als_Air_Lean.json`                       | `assets/graphs/ab_als/locomotion/air_lean.animgraph.ron`                                                                                                        | 0%  |
| `/ALS/Animations/Grounded/Lean/BS_Als_Lean`                      | `assets/animations/blendspaces/Grounded/Lean/BS_Als_Lean.json`                      | `assets/graphs/ab_als/stances/standing/lean.animgraph.ron`<br>`assets/graphs/ab_als/stances/crouching/lean.animgraph.ron`                                       | 0%  |
| `/ALS/Animations/Grounded/WalkRun/BS_Als_WalkRun_Backward`       | `assets/animations/blendspaces/Grounded/WalkRun/BS_Als_WalkRun_Backward.json`       | `assets/graphs/ab_als/stances/standing/state_movement_backward.animgraph.ron`<br>`assets/graphs/ab_als/stances/standing/state_detail_cycle.animgraph.ron`       | 0%  |
| `/ALS/Animations/Grounded/WalkRun/BS_Als_WalkRun_Forward`        | `assets/animations/blendspaces/Grounded/WalkRun/BS_Als_WalkRun_Forward.json`        | `assets/graphs/ab_als/stances/standing/state_movement_forward.animgraph.ron`<br>`assets/graphs/ab_als/stances/standing/state_detail_cycle.animgraph.ron`        | 0%  |
| `/ALS/Animations/Grounded/WalkRun/BS_Als_WalkRun_Left_Backward`  | `assets/animations/blendspaces/Grounded/WalkRun/BS_Als_WalkRun_Left_Backward.json`  | `assets/graphs/ab_als/stances/standing/state_movement_left_backward.animgraph.ron`<br>`assets/graphs/ab_als/stances/standing/state_detail_cycle.animgraph.ron`  | 0%  |
| `/ALS/Animations/Grounded/WalkRun/BS_Als_WalkRun_Left_Forward`   | `assets/animations/blendspaces/Grounded/WalkRun/BS_Als_WalkRun_Left_Forward.json`   | `assets/graphs/ab_als/stances/standing/state_movement_left_forward.animgraph.ron`<br>`assets/graphs/ab_als/stances/standing/state_detail_cycle.animgraph.ron`   | 0%  |
| `/ALS/Animations/Grounded/WalkRun/BS_Als_WalkRun_Right_Backward` | `assets/animations/blendspaces/Grounded/WalkRun/BS_Als_WalkRun_Right_Backward.json` | `assets/graphs/ab_als/stances/standing/state_movement_right_backward.animgraph.ron`<br>`assets/graphs/ab_als/stances/standing/state_detail_cycle.animgraph.ron` | 0%  |
| `/ALS/Animations/Grounded/WalkRun/BS_Als_WalkRun_Right_Forward`  | `assets/animations/blendspaces/Grounded/WalkRun/BS_Als_WalkRun_Right_Forward.json`  | `assets/graphs/ab_als/stances/standing/state_movement_right_forward.animgraph.ron`<br>`assets/graphs/ab_als/stances/standing/state_detail_cycle.animgraph.ron`  | 0%  |
| `/ALS/Animations/View/BS_Als_Look`                               | `assets/animations/blendspaces/View/BS_Als_Look.json`                               | `assets/graphs/ab_als/head.animgraph.ron`                                                                                                                       | 0%  |

## AnimBlueprints (26)

### Character Base (9)

| ALS-Ref package                                              | Current counterpart files                                                                                                   | 等价度 |
| ------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------- | --- |
| `/ALS/Character/AnimationInstances/Stances/AB_Als_Standing`  | `assets/graphs/ab_als/stances/standing.animgraph.ron`<br>`assets/graphs/ab_als/stances/standing/standing_states.fsm.ron`    | 0%  |
| `/ALS/Character/AnimationInstances/Stances/AB_Als_Crouching` | `assets/graphs/ab_als/stances/crouching.animgraph.ron`<br>`assets/graphs/ab_als/stances/crouching/crouching_states.fsm.ron` | 0%  |
| `/ALS/Character/AnimationInstances/AB_Als_Grounded`          | `assets/graphs/ab_als/grounded.animgraph.ron`<br>`assets/graphs/ab_als/grounded/grounded_states.fsm.ron`                    | 0%  |
| `/ALS/Character/AnimationInstances/AB_Als_Locomotion`        | `assets/graphs/ab_als/locomotion.animgraph.ron`<br>`assets/graphs/ab_als/locomotion/air_lean.animgraph.ron`                 | 0%  |
| `/ALS/Character/AnimationInstances/AB_Als_Layering`          | `assets/graphs/ab_als/layering.animgraph.ron`<br>`assets/graphs/ab_als/layering/base_poses.animgraph.ron`                   | 0%  |
| `/ALS/Character/AnimationInstances/AB_Als_Head`              | `assets/graphs/ab_als/head.animgraph.ron`                                                                                   | 0%  |
| `/ALS/Character/AnimationInstances/AB_Als_Ragdolling`        | `assets/graphs/ab_als/ragdolling.animgraph.ron`<br>`src/ragdolling_layer.rs`                                                | 0%  |
| `/ALS/Character/AB_Als`                                      | `assets/graphs/ab_als/ab_als.animgraph.ron`                                                                                 | 0%  |
| `/ALS/Character/ALI_Overlay`                                 | `assets/graphs/ab_als/ali_overlay.animgraph.ron`                                                                            | 0%  |

### Control Rig & Physics Asset (2)

| ALS-Ref package                      | Current counterpart files                                                                                                  | 等价度 |
| ------------------------------------ | -------------------------------------------------------------------------------------------------------------------------- | --- |
| `/ALS/Character/CR_Als`              | `assets/graphs/ab_als/ab_als.animgraph.ron`<br>`crates/als-ext/src/animation_graph_ext/root_graph_boundaries.rs`<br>`src/control_rig_apply.rs` | 0%  |
| `/ALS/Character/PA_Als`              | `assets/ragdolls/als_manny.rag.ron`<br>`assets/ragdoll_bone_maps/als_manny.bm.ron`<br>`src/ragdoll_link.rs`                | 0%  |

### Character Overlays (13)

* ALS-Refactored Path: /ALS/Character/AnimationInstances/Overlays/
* als_bevy Path: assets/graphs/ab_als/overlays/

| ALS-Ref package          | Current counterpart files         | 等价度 |
| ------------------------ | --------------------------------- | --- |
| `AB_Als_Default`         | `default.animgraph.ron`           | 0%  |
| `AB_Als_Torch`           | `torch.animgraph.ron`             | 0%  |
| `AB_Als_Rifle`           | `rifle.animgraph.ron`             | 0%  |
| `AB_Als_PistolTwoHanded` | `pistol_two_handed.animgraph.ron` | 0%  |
| `AB_Als_PistolOneHanded` | `pistol_one_handed.animgraph.ron` | 0%  |
| `AB_Als_Masculine`       | `masculine.animgraph.ron`         | 0%  |
| `AB_Als_Injured`         | `injured.animgraph.ron`           | 0%  |
| `AB_Als_HandsTied`       | `hands_tied.animgraph.ron`        | 0%  |
| `AB_Als_Feminine`        | `feminine.animgraph.ron`          | 0%  |
| `AB_Als_Box`             | `box.animgraph.ron`               | 0%  |
| `AB_Als_Bow`             | `bow.animgraph.ron`               | 0%  |
| `AB_Als_Binoculars`      | `binoculars.animgraph.ron`        | 0%  |
| `AB_Als_Barrel`          | `barrel.animgraph.ron`            | 0%  |

### Others (2)

| ALS-Ref package                      | Current counterpart files                                                                                                  | 等价度 |
| ------------------------------------ | -------------------------------------------------------------------------------------------------------------------------- | --- |
| `/ALS/OverlayObjects/Bow/AB_Als_Bow` | `assets/graphs/overlay_objects/bow.animgraph.ron`<br>`src/overlay_object.rs`                                               | 0%  |
| `/ALSCamera/AB_Als_Camera`           | `crates/als-camera/src/lib.rs`<br>`assets/params/als_camera_profiles.ron`<br>`assets/params/als_camera_sockets.ron`        | 0%  |

## AnimMontages (9)

全部 9 个源 Montage 也登记在 `assets/animation_events/als_notify_manifest.json`。目标以序列 clip、图与 ECS state 组合替代 UE 风格的 Montage 资产。

| ALS-Ref package                                             | Current counterpart files                                                                                                                                                                                                                                        | 等价度 | Mapping                                |
| ----------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --- | -------------------------------------- |
| `/ALS/Animations/Actions/GetUp/AM_Als_GetUp_Back`           | `assets/animations/Actions/GetUp/A_Als_GetUp_Back.anim.ron`<br>`assets/animation_events/als_notify_manifest.json`                                                                                                                                                | 0%  | 仅导出；当前无图／运行时 playback selector         |
| `/ALS/Animations/Actions/GetUp/AM_Als_GetUp_Front`          | `assets/animations/Actions/GetUp/A_Als_GetUp_Front.anim.ron`<br>`assets/animation_events/als_notify_manifest.json`                                                                                                                                               | 0%  | 仅导出；当前无图／运行时 playback selector         |
| `/ALS/Animations/Actions/Mantle/AM_Als_Mantle_High`         | `assets/animations/Actions/Mantle/A_Als_Mantle_High.anim.ron`<br>`assets/graphs/actions.animgraph.ron`<br>`crates/als-core/src/mantle.rs`<br>`src/mantle_drive.rs`<br>`src/animation_events.rs`                                                                  | 0%  | High 与 InAir mantle 替代物                |
| `/ALS/Animations/Actions/Mantle/AM_Als_Mantle_Low`          | `assets/animations/Actions/Mantle/A_Als_Mantle_Low_Right.anim.ron`<br>`assets/graphs/actions.animgraph.ron`<br>`crates/als-core/src/mantle.rs`<br>`src/mantle_drive.rs`<br>`src/animation_events.rs`                                                             | 0%  | Low-right 源序列与 source-montage event 映射 |
| `/ALS/Animations/Actions/Mantle/AM_Als_Mantle_Low_BothArms` | `assets/animations/Actions/Mantle/A_Als_Mantle_Low_Right.anim.ron`<br>`assets/graphs/actions.animgraph.ron`<br>`crates/als-core/src/mantle.rs`<br>`src/mantle_drive.rs`<br>`src/animation_events.rs`                                                             | 0%  | Low-right 源序列与 BothArms event 映射       |
| `/ALS/Animations/Actions/Mantle/AM_Als_Mantle_Low_Box`      | `assets/animations/Actions/Mantle/A_Als_Mantle_Low_Right.anim.ron`<br>`assets/graphs/actions.animgraph.ron`<br>`crates/als-core/src/mantle.rs`<br>`src/mantle_drive.rs`<br>`src/animation_events.rs`                                                             | 0%  | Low-right 源序列与 Box event 映射            |
| `/ALS/Animations/Actions/Mantle/AM_Als_Mantle_Low_Left`     | `assets/animations/Actions/Mantle/A_Als_Mantle_Low_Left.anim.ron`<br>`assets/graphs/actions.animgraph.ron`<br>`crates/als-core/src/mantle.rs`<br>`src/mantle_drive.rs`<br>`src/animation_events.rs`                                                              | 0%  | Low-left mantle 替代物                    |
| `/ALS/Animations/Actions/Mantle/AM_Als_Mantle_Low_Right`    | `assets/animations/Actions/Mantle/A_Als_Mantle_Low_Right.anim.ron`<br>`assets/graphs/actions.animgraph.ron`<br>`crates/als-core/src/mantle.rs`<br>`src/mantle_drive.rs`<br>`src/animation_events.rs`                                                             | 0%  | Low-right mantle 替代物                   |
| `/ALS/Animations/Actions/Roll/AM_Als_Roll`                  | `assets/animations/Actions/Roll/A_Als_Roll.anim.ron`<br>`assets/graphs/ab_als/grounded/state_roll.animgraph.ron`<br>`assets/graphs/ab_als/grounded/roll_slot.animgraph.ron`<br>`crates/als-core/src/roll.rs`<br>`src/roll_drive.rs`<br>`src/animation_events.rs` | 0%  | Roll 图、状态、root-motion 与 notify 替代物     |
