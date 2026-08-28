# 已完成清单大项归档（2026-08-29）

> 本文件从 `CHECKLIST.md` 移出，保留本批全部实时事实、实现和 Gate 证据。

## ANIM-PARITY-2026-08-25（动画状态机全面对标 AB_Als；用户实测缺陷修复）

> 缺陷来源（2026-08-25 用户实测 manual_playground）：①下蹲不播下蹲动画只以站姿下沉；②跑步（非冲刺）高度一跳一跳；③跳跃动画不对。用户指令：对照 ALS-Refactored 审查**所有**动画状态机（AB_Als 及其 linked anim graph 与 linked anim layer），确保表现一致。
> 事实提取方法：UE5.8 uasset 二进制解析（tools/import/abp_structure.py、abp_states.py，含 PACKAGE_SAVED_HASH 前后双格式支持）+ 插件 C++ 源码 AlsAnimationInstance.cpp 行为规则。提取产物存 tmp/（不入库），事实登记于各条目。
> **AB_Als 架构事实（v4.18）**：主图链 Standing/Crouching LG → Grounded LG(SM: Standing/Crouching/CrouchToStand(A_Als_CrouchToStand)/StandToCrouch(A_Als_StandToCrouch)/Roll) → Slot(Transition) → Locomotion LG(SM: Grounded/Jump(SM: LeftFoot/RightFoot/Loop/Flail)/Fall/LandMovement/Land + LandConduct conduit) → Slot(PostLocomotion) → Layering LG(Locomotion+Overlay 双输入, Stand_Pose/Crouch_Pose 基姿, Layer*Slot 曲线清零) ；View(Head) LG 在 ControlRig 后；Ragdolling LG 独立支路；ControlRig(CR_Als)→Inertialization→Root；Overlay 图=LocalRefPose+ALI_Overlay 层(AB_Als_Default 等 13 实现)。Stance 层内部：Crouching States SM{Idle(A_Als_Crouch_Pose)/Move/Stop/RotateL/R(Crouch_Rotate_90)} + Movement States SM 六向{Fwd/Bwd/RFw/RBk/LFw/LBk}(A_Als_Crouch_Walk_* SyncGroup Movement, 转换 Cubic 0.7s)；Standing 层同构六向 + Movement Details SM{Walk/Run/Sprint(+Acceleration, StartPosition 0.1667)/FirstPivot/SecondPivot/RunStartFromWalk}；Stop States SM{Entry/Lock-Plant L/R Foot}用 A_Als_Walk_* 显式帧(21/24/6/7 等)。行为规则(C++)：MovementDirection=CalculateMovementDirection(velocity_yaw−view_yaw, ForwardHalfAngle70°, Threshold5°)，VelocityDirection 模式或 Sprint 恒 Forward；WalkRunBlendAmount=Gait==Walking?0:1（离散）；PlayRate=(Speed/AnimatedWalk|Run|SprintSpeed 按 UnweightedGaitAmount lerp)/StrideBlendAmount，钳[~0,3]，蹲=Speed/(AnimatedCrouchSpeed·StrideBlend) 钳≤2；StrideBlend=曲线按 Speed 采样按 running amount lerp；JumpPlayRate=lerp(1.2,1.5,Speed/600) 起跳沿生效；SprintAccelerationAmount 仅冲刺前 0.5s；设置默认 AnimatedWalk/Run/Sprint/CrouchSpeed=150/350/600/(crouch 见 AlsCrouchingSettings) cm/s、PivotActivationSpeedThreshold=200。

- [x] AP1 Stance 维度接入动画图（下蹲动画链路）：base_locomotion 子图增加 crouching 分支（idle=A_Als_Crouch_Pose、walk=A_Als_Crouch_Walk_Forward 起步先正向）与 stance_weight 混合；根图 pin 合同扩容 `stance_weight` F32；驱动系统 AlsStanceBlend 组件平滑跟随 Stance（ALS StandToCrouch/CrouchToStand clip 时长≈0.8s 等价 crossfade，AP5 换真 clip）；下蹲时 speed 上限已由 motion.rs 钳 1.50。验收：manual 下蹲可见蹲姿 idle/蹲走，起立恢复；workspace 测试全绿；截图 docs/ap1-shots/。
- [x] AP2 Gait 离散化与步频修正（跑步起伏）：废弃连续 speed BlendSpace 的 walk↔run 半程混合——ALS WalkRunBlendAmount 为离散值；新增 als-core gait 动画状态机（Walk/Run/Sprint 三态离散选择+进入退出 crossfade 包络 0.25s Cubic 等价）+ ALS 步频公式 play_rate=(speed/animated_gait_speed)/stride_blend_amount 钳[0,3]（stride 曲线先用常量近似 walk1.35/run1.2，后续 M0.5 增补条目导 CF_Als_StrideBlendAmount_*）+ speed EMA 平滑；图侧改为 walk/run 双链按 gait_weight 离散混合。验收：匀速跑无纵向起伏、步频与位移同步无滑步；locomotion_gate 复跑 PASS。
- [x] AP3 六向移动对齐：port CalculateMovementDirection(70°/5°) 至 als-core movement_direction.rs（VelocityDirection/Sprint 恒 Forward），六态选择+F/B/L/R 主方向带±对角带划分与 hysteresis；图侧 per-direction BlendSpace1D×stance 或 BlendSpace2D(speed×angle) 挂接已导出的 12 个 directional clips（WalkRun 6+Crouch 6）；转换 crossfade 0.7s Cubic 对齐。验收：WASD 各向播对应方向 clip、横移不再播前向动画。
- [x] AP4 Jump/Fall/Land 重构：起跳沿选 Jump_Walk|Run_Left|Right 四变体（速度阈值 walk/run 分界+左右脚交替起步近似 ALS FootLock 相位）、JumpPlayRate=lerp(1.2,1.5,speed/600)、Jump_Loop 循环、Fall 图 Flail 按 VelocityWorldSpace 速度映射接入、落地 Land_*_Additive 叠加继续移动（Land Movement 语义）/静止全身 Land clip 二分；fall_fast 权重语义对照 Fall↔Fall_Fast blend 保留。验收：air_gate 扩展断言（jump start clip 生效、additive land 移动连续性）PASS。
- [x] AP5 Stop/Stance/DynamicTransition 真 clip 化：stop 由 DistanceMatch(A_Als_Stop_L/R) 升级 ALS 语义 Plant/Lock Foot（A_Als_Walk_* 显式帧 21/24/6/7 + distance match）；stance 切换换真 clip A_Als_StandToCrouch/A_Als_CrouchToStand（Grounded SM 直连状态，非覆盖层）；dynamic transition 选择器(transitions.rs 已有 dormant)接入 actions 层。验收：急停脚部 planting 可见、蹲起播转换 clip。
- [x] AP6 TurnInPlace/RotateInPlace/Pivot 接线：turn_in_place/rotate_in_place 检测器输出驱动 RotateInPlace/TurnInPlace clips（Standing: A_Als_Turn_90|180_L/R + Crouch_Rotate_90_L/R 已导出；pivot 用 dynamic transition 等价）。验收：原地大幅转视角触发转身动画且根运动回正。
- [x] AP7 差异矩阵收口与 Gate：全链路差异矩阵（本节头架构事实 × 当前实现）逐项标注 已对齐/刻意差异(附录A)/待办 三类；刻意差异仅限 ECS 架构等价替换（SM→system+权重包络、SyncGroup→Absolute 同步策略、Layering 曲线体系简化版、ModifyCurve→分析式后处理）；Gate=12 门禁复跑 PASS+workspace 全绿+manual 冒烟+实机三症状复核截图 docs/。

### M0.5 增补（AP7 遗留项）

- [x] StrideBlend 真曲线接入：导入并采样 `CF_Als_StrideBlendAmount_Walk` 与 `CF_Als_StrideBlendAmount_Run`，替换常量近似；`als-core` 曲线/步频单测通过。
- [x] Crouch RotateInPlace 图侧挂接：将 `Crouch_Rotate_90_L/R` 接入 actions 图及组合根 pin，沿用既有检测器和 turn gate 验证。
- [x] Flail 真速度映射接入：实时提取 `AB_Als_Locomotion` 的 Fall 图 `TwoWayBlend_2`，其 B 输入为 `A_Als_Flail`，`VerticalVelocityWorldSpace` 经 `[-500,-3000] cm/s` 反向区间映射为权重，并以 5/s 插值；Bevy 以 y-up 米制映射为下落速度 5–30 m/s，离开 Air 状态立即退出。`air_gate` 实测高坠阶段 `vy=-33.81 m/s`、状态/pin 权重 0.66，最终 verdict PASS。

依赖关系：AP1 → AP2 → AP3 → AP4 →（AP5、AP6 可并行）→ AP7；AP1–AP6 均保持 RG 组合根结构与既有 pin 合同兼容（新增 pin 只增不改名）。

---
