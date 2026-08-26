# AP7 差异矩阵（AB_Als v4.18 架构事实 × 当前实现）

状态三类：**已对齐** / **刻意差异**(附录A, ECS 等价) / **待办**。

## 主图链

| ALS-Refactored | bevy_als | 状态 | 落点 |
|---|---|---|---|
| Standing/Crouching LG 分层 | base_locomotion 子图 stance_weight 混合 + stance_trans 真 clip | 已对齐 | AP1/AP5 |
| Grounded SM{Standing/Crouching/CrouchToStand/StandToCrouch/Roll} | SM→system+权重包络(规则12 刻意差异)；CrouchToStand/StandToCrouch 真 clip | 已对齐 | AP5 |
| Slot(Transition) | actions 层 roll/stop/mantle/turn override 链 | 已对齐 | RG2+AP6 |
| Locomotion LG SM 六向{Fwd/Bwd/RFw/RBk/LFw/LBk} | BlendSpace1D×3(walk/run/crouch) 挂 direction_angle，扇区 60/120/150+hysteresis 5° | 已对齐 | AP3 |
| Movement Details SM{Walk/Run/Sprint} | gait 离散三链 move/run/sprint weight 包络 0.25s | 已对齐 | AP2 |
| Jump SM{LeftFoot/RightFoot/Loop} | 四变体 Jump_Walk|Run_L/R + Jump_Loop | 已对齐 | AP4 |
| Fall/Fall_Fast blend | fall_fast_weight 语义保留 | 已对齐 | M8 |
| Land Movement/Land 二分 | land_light/heavy blend + land_weight | 已对齐 | M8 |
| Stop States SM{Plant L/R Foot} | A_Als_Stop_L/R DistanceMatch+hold-weight 补完到 1.0 | 已对齐 | AP5 |
| TurnInPlace/RotateInPlace clips | turn override 链(Turn_90/180_L/R)+检测器驱动 | 已对齐 | AP6 |
| Layering LG(Locomotion+Overlay 双输入) | locomotion_overlay 四层组合根(LayeredGraphNode) | 刻意差异 | RG2 |
| View(Head) LG 在 ControlRig 后 | overlay_view 层 look pins | 已对齐 | M6 |
| Ragdolling 独立支路 | ragdoll_link 双资产进出 | 已对齐 | M9 |
| ControlRig(CR_Als) | foot_ik_apply 分析式 IK | 刻意差异 | M7 |
| Overlay 图 LocalRefPose+ALI_Overlay | overlay_blend 差分叠加 | 刻意差异 | M6 |

## 行为规则(C++)

| ALS 规则 | bevy_als | 状态 |
|---|---|---|
| MovementDirection=CalcMovDir(vel_yaw−view_yaw,70°,5°) | movement_direction.rs 同参数+hysteresis | 已对齐 |
| WalkRunBlendAmount 离散 0/1 | GaitWeights 三态离散 | 已对齐 |
| PlayRate=(Speed/AnimatedSpeed)/StrideBlend 钳[0,3]，蹲≤2 | als_play_rate 同公式同钳位 | 已对齐 |
| AnimatedWalk/Run/Sprint/Crouch=150/350/600/150 | params 默认值一致(单测断言) | 已对齐 |
| StrideBlend 曲线按 Speed 采样 lerp | 常量近似 walk1.35/run1.2(M0.5 待导真曲线) | 待办(M0.5 增补) |
| JumpPlayRate=lerp(1.2,1.5,speed/600) | air_jump::jump_play_rate | 已对齐 |
| PivotActivationSpeedThreshold=200 | pivot.rs 配置 | 已对齐 |

## 用户三缺陷复核

| 缺陷 | 根因 | 修复 | Gate |
|---|---|---|---|
| ①下蹲不播动画只下沉 | 图无 crouching 分支 | AP1 蹲姿 idle/walk+stance_weight；AP5 真 StandToCrouch clip | stance_gate PASS |
| ②跑步高度一跳一跳 | 连续 speed BS 半程混合 walk↔run 异相 loop | AP2 废弃连续 BS 改离散 gait 链+ALS 步频 | locomotion_gate PASS |
| ③跳跃动画错误 | 无变体选择恒 Loop | AP4 四变体起跳+JumpPlayRate | air_gate PASS |

## 待办(不阻塞 AP7 Gate)
1. CF_Als_StrideBlendAmount_* 真曲线导出(M0.5 增补条目)
2. RotateInPlace(Crouch_Rotate_90_L/R)挂接——检测器已有，图侧留 AP7 后增补
3. Flail 长坠落 clip 接入(air 图已留 fall_fast 权重通道)
