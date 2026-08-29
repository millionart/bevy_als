# bevy_als — 执行清单

> 本文件是唯一权威执行清单；执行纪律见 `agents.md`（仅规则），启动方式见 `goal.md`。
> 完成一项即勾选并一次提交：实现 → 自测 → 勾选 `[x]` → `git commit -m "<前缀><编号>: <条目摘要>"`（勾选与实现同一提交）。
> 基座模块提交前缀沿用 `M<编号>`；缺陷修复清单用 `BF<编号>`。
> 已归档清单：[archivedchecklist.md](docs/archive/archivedchecklist.md)

## 背景与基础事实（已核实，持续有效）

- 本机 bevy 源码：`0.19.1`（release-0.19.1）。`bevy_animation_graph` 0.11.0 与 `bevy-tnua` 均基于 Bevy 0.19，兼容。
- bevy_animation_graph 已有节点：ClipNode、ChainNode、BlendNode（含骨骼 mask/Difference/Additive）、BlendSpace1D/2D、布尔运算节点等；图资产 RON 序列化、支持编辑器热重载；Pose 以 `BoneId` 路径表达；RootMotionMode 提取已可用（M8.8）。曲线/ramp/距离匹配等由 als-ext 经公开扩展点补齐。
- bevy-tnua 已有 basis：Walk（coyote time）、Jump（高度/悬挂控制）、Crouch、Dash、Climb、Knockback、WallSlide；物理后端 avian3d 0.12.1 / rapier3d 0.17.0。
- ALS-Refactored 关键结构：4 模块（ALS / ALSCamera / ALSExtras / ALSEditor）；状态体系用 GameplayTags（非枚举）；自定义 AnimNode_CurvesBlend；ControlRig 输入结构 `AlsControlRigInput`；IK 相关骨骼 `foot_l/foot_r`、`ik_foot_l/ik_foot_r`、`VB foot_l/VB foot_r`；关键曲线名 `LayerHandLeft/Right`、`HandLeftIk/HandRightIk`、`FootLeftIk/FootLeftLock/FootRightIk/...`；动画资产分组：Grounded / Air / Actions / Transitions / TurnInPlace / RotateInPlace / Overlays / View / Base。
- 本地环境路径、引擎版本等隐私信息见 `docs/env.md`（已 gitignore）。

## 模块依赖顺序

```
M0 环境与资产管线 ─► M1 ECS核心状态模型 ─► M2 曲线基础设施 ─► M3 tnua运动基座
─► M4 动画图基座 ─► M5 Locomotion状态机动画 ─► M6 Overlay/Aim ─► M7 FootIK/CR等价物
─► M8 特殊动作(跳跃翻滚) ─► M9 Ragdoll ─► M10 相机 ─► M11 调试与总验收
```

## BF102 — 2026-08-29 默认生产链原子审计清单

来源是已校验的 2026-08-29 58 项静态审计包（`findings.json` SHA-256 `40be20020dc11ad7b39a145aee3d8a948b703ea0ddd1dac56b34fe3b1327ff1d`）。其 Bevy 基线为 `ea05bbe`，早于已归档的 [BF101](docs/archive/2026-08-29/BF101-2026-08-29.md)；M02、M03、M09 因而只作当前状态回归核验。审计为固定源码与公式／控制流取证，不是完整 UE／Bevy 实机回放；条件性资产/底座项不得冒充默认场景故障。

本段仅登记原子范围、依赖和验收条件，不授权源码提取、资产查询、实现或验收。每项成为当前执行项时必须重新从 ALS-Refactored 提取当次事实；确认缺陷后立即建立独立修复子项，并在其 Gate 通过前保持原审计项未勾选。旧的泛化与分组 BF102 段已分别归档至 [pre-reconciliation](docs/archive/2026-08-29/BF102-pre-reconciliation-2026-08-29.md) 和 [coarse-58-map](docs/archive/2026-08-29/BF102-coarse-58-map-2026-08-29.md)。

```
BF102.01–.03 BF101 回归
 ├► BF102.04–.09 默认执行链/槽位
 ├► BF102.10–.14 坐标、状态、投影
 ├► BF102.15–.23 实例所有权与移动基座
 └► BF102.24–.30 动画底座
      ├► BF102.31–.39 空中与时间
      ├► BF102.40–.45 Foot IK
      ├► BF102.46–.51 特殊动作
      └► BF102.52–.57 相机
BF102.58 网络范围 ─► BF102.Gate
```

### BF101 基线回归

- [x] **BF102.01 — M02：视角相对台阶方向回归核验（P1）**
  - 依赖：已通过 BF101.Gate。
  - 验收：默认生产入口在四个 view yaw 下的台阶探针方向与 locomotion 世界方向一致；只对仍复现的缺口创建修复子项。
  - 当次 ALS 事实：`AAlsCharacterExample::Input_OnMove` 以当前 View yaw 求世界前／右向量并调用 `AddMovementInput`；`UAlsCharacterMovementComponent::MoveSmooth` 在刷新 grounded 配置后委托 UE 原生 `Super::MoveSmooth`。
  - 证据：`cargo test -p als-core --jobs 2 rotation_cap_tests::step_assist_uses_default_motion_plugin_with_physics_queries -- --ignored --nocapture --test-threads=1` 通过；`AlsMotionPlugin` 生产调度下 yaw 0／+90／180／-90° 的方向点积均为 1，上抬均为 0.305 m，Position／Transform 同步误差均为 0。

- [x] **BF102.02 — M03：逻辑 stance 与物理胶囊同步回归核验（P1）**
  - 依赖：已通过 BF101.Gate。
  - 验收：低顶取消蹲伏时同时记录逻辑请求、实际 collider、Tnua headroom 和最终碰撞状态；区分 BF101 后保留的请求语义与真实不同步。
  - 当次 ALS 事实：`ApplyDesiredStance` 依据 `DesiredStance` 调用 `Crouch`／`UnCrouch`；只有原生动作成功进入 `OnStartCrouch`／`OnEndCrouch` 后才调用 `SetStance` 更新实际姿态。
  - [x] **BF102.02.a — M03：分离 desired 与 actual stance（P1）**
    - 依赖：BF102.02 当次 ALS 事实与低顶复现。
    - 验收：受阻站起时 desired 保持 Standing、actual `Stance`/胶囊/动画/Tnua 均保持 Crouching；净空恢复后原子切换为 Standing 并保留请求的可追溯性。
    - 证据：`cargo test -p als-core --jobs 2 rotation_cap_tests -- --ignored --nocapture --test-threads=1` 通过。低顶时 `desired=Standing`、`actual=Crouching`、物理 crouched=true、Tnua headroom=0.560 m、碰撞重叠=false、同步误差=0；移除低顶后 actual=Standing、crouched=false、headroom=0.900 m、同步误差=0。
  - 回归：`cargo test --workspace --jobs 2` 通过；als-core 196 通过／2 专用 Gate 已单独通过，als-ext 159、als-camera 5、bevy_als 110 均通过。

- [x] **BF102.03 — M09：方向相关速度配置回归核验（P3）**
  - 依赖：已通过 BF101.Gate。
  - 验收：默认关闭保持原速度；启用后前进/中间/后退阈值、gait、Tnua 和最终动画输入使用同一当前配置。
  - 当次 ALS 事实：`FAlsMovementGaitSettings` 默认关闭方向相关速度；`RefreshGroundedMovementSettings` 以速度相对 view 的绝对角、`VelocityAngleToSpeedInterpolationRange` 100°–125°插值前后 walk/run，并用结果映射 gait amount。
  - 证据：`cargo test -p als-core --jobs 2 direction_dependent_speed -- --nocapture`、`refresh_gait_uses_direction_dependent_thresholds` 与 `max_allowed_speed_consumes_direction_dependent_thresholds` 均通过。默认关闭前后阈值相同；启用后前进=(1.75,3.75)、中间=(1.375,2.875)、后退=(1.0,2.0) m/s，后退 gait amount=1.2、dynamics/Tnua run speed=2.0 m/s。
  - 最终动画链证据：`cargo test --jobs 2 direction_dependent_gait_amount_reaches_animation_layer -- --nocapture` 通过；生产 `GaitLayer` 接收核心的后退 gait amount=1.2，写入最终动画 running amount=0.2、sprint amount=0。

### 默认执行链与槽位

- [x] **BF102.04 — A01：默认 PostLocomotion 槽位接通翻滚驱动（P1）**
  - 依赖：BF102.01。
  - 验收：默认 avatar 入口中翻滚请求可追到每角色 slot 姿态、权重、曲线、根运动和最终骨骼。
  - 当次 ALS 事实：`StartRollingImplementation` 以 `Montage_Play` 启动滚翻；当前 `AM_Als_Roll` 二进制资产含 `PostLocomotion` 槽位标识。
  - [x] **BF102.04.a — A01：实现每角色 Roll PostLocomotion slot 调度（P1）**
    - 依赖：BF102.04 当次 ALS Montage／槽位事实与默认入口运行反例。
    - 验收：滚翻 clip 以每角色独立时间采样，向 `als_root_post_locomotion_slot_pose`／weight 写入姿态与曲线；同一 clip 的 root motion 驱动角色；默认 Gate 记录槽位权重、最终 pose 及累计位移。
    - 证据：完整 `AlsPlayableCharacterPlugin` 入口运行 `roll_gate`（12 s safety timeout）输出 `completed=true`。两次滚翻均记录距离约 2.75 m、方向点积 1、slot weight 1、slot root delta 约 0.0732 m、slot pose=true、roll curve=1、Roll state=true、final pose=true，最终 Pose 指纹变化分别约 7.08／7.87。
  - 回归：`cargo test -p bevy_als --lib --jobs 2` 通过（111 tests）。

- [x] **BF102.05 — A02：一次性动画转换事件跨时钟可靠消费（P1）**
  - 依赖：BF102.04。
  - 验收：30/60/144/240 FPS × fixed dt 下的每个 transition 都有一次实际 FSM 消费记录，不能被后续渲染帧覆盖。
  - 当次 ALS 事实：`PlayTransitionAnimation` 在工作线程仅写入 queued transition；`NativePostUpdateAnimation` 在可执行动画的阶段调用 `PlayQueuedTransitionAnimation`，成功后才清空 queued sequence。
  - [x] **BF102.05.a — A02：按图求值确认消费 locomotion transition（P1）**
    - 依赖：BF102.05 当次 ALS 队列语义与当前跨时钟反例。
    - 验收：每 avatar 的 outer/jump transition 仅在 `AnimationGraphSet::PrePhysics` 后确认出队；同一待消费 index 在任意渲染帧数下保持可见，但 DirectTransition 只发出一次。
    - 证据：30/60/144/240 Hz 渲染 × 30/60/120 Hz fixed 的 12 组 `BF102.05` 探针均为 `deliveries=1`；`InterruptibleFsmNode` 实测消费后状态为 `HlTransition(Direct(...))`、进入时间 0.25、`deliveries=1`。
  - 回归：`cargo test -p bevy_als --lib --jobs 2` 通过（112 tests）；`cargo test -p als-ext --jobs 2` 通过（161 tests）。实际默认入口的 FSM 可观测性会在 BF102.07 单独继续增强。

- [x] **BF102.06 — A03：头部 ViewBlock/PoseAiming 曲线桥接（P1）**
  - 依赖：BF102.04。
  - 验收：默认主图的实际曲线在正确时间点驱动每角色 `AlsHeadCurveState` 与最终头部权重。
  - 当次 ALS 事实：`RefreshView` 读取 `ViewBlock` 与 `PoseAiming`，并以 `(1 - ViewBlock) × (1 - PoseAiming)` 计算 HeadBlendAmount。
  - 证据：`head_curve_gate` 在完整 `AlsPlayableCharacterPlugin` 的 Rifle+Aiming 默认图中记录 `PoseAiming=1`、`ViewBlock=0`、`als_head_blend_amount=0`，与桥接组件和 ALS 公式一致；`cargo test -p bevy_als --lib --jobs 2` 通过（113 tests）。

- [ ] **BF102.07 — Q01：遥测改为实际 FSM 状态证据（P2）**
  - 依赖：BF102.05。
  - 验收：探针同时记录请求输入、FSM state id、状态进入时间和最终 pose，禁止将 writer 回显当作已消费状态。

- [ ] **BF102.08 — A05：overlay 运行输入去硬编码（P2）**
  - 依赖：BF102.04、BF102.06。
  - 验收：SprintAcceleration 和 AllowTransitions 由实际 standing/曲线状态提供，并在各 overlay 消费者处量测。

- [ ] **BF102.09 — A06：slot 姿态与曲线同权重混合（P2）**
  - 依赖：BF102.04、BF102.08。
  - 验收：槽位动作注入 FootLock、ViewBlock 等曲线时，最终曲线按同一角色、同一时间、同一权重混合。

### 坐标、状态、旋转与投影

- [ ] **BF102.10 — M01：统一 yaw 正负号与翻滚/头部约定（P1）**
  - 依赖：BF102.04。
  - 验收：非零朝向、横向翻滚和头部相对 yaw 的身体朝向、位移与局部角均满足同一数值约定。

- [ ] **BF102.11 — M04：旋转半衰期/曲线成为实际角色旋转律（P1）**
  - 依赖：BF102.05、BF102.10。
  - 验收：运动、瞄准和原地转向的最终 Transform yaw 与当前曲线/半衰期配置一致。

- [ ] **BF102.12 — M05：VelocityDirection 使用实际速度方向（P2）**
  - 依赖：BF102.10、BF102.11。
  - 验收：刹车、反向输入和碰撞滑动时，身体目标来自实际速度而非新输入方向。

- [ ] **BF102.13 — M08：输入 override 可强制解除瞄准（P2）**
  - 依赖：BF102.05、BF102.12。
  - 验收：重放、AI 和脚本 override 与真实按键冲突时，最终 aim/rotation state 等于明确的 override 合同。

- [ ] **BF102.14 — C01：UE 水平 FOV 转换为 Bevy 垂直 FOV（P1）**
  - 依赖：BF102.10。
  - 验收：多宽高比下投影矩阵的水平/垂直 FOV 与 UE 契约一致，16:9 水平 90° 的垂直值约为 58.7155°。

### 每角色所有权、移动基座与临时物理状态

- [ ] **BF102.15 — A04：动作和动画实例按角色归属（P1）**
  - 依赖：BF102.04。
  - 验收：两个 ALS 角色及无关动画实例同时存在时，roll/mantle/ragdoll/root motion 不使用 `single()` 或未归属的 `next()`。

- [ ] **BF102.16 — F08：禁用 Tnua 实体不终止其他实体电机（P1）**
  - 依赖：BF102.15。
  - 验收：Enabled/Disabled/Enabled 三实体探针证明 A、C 均更新，Disabled 只跳过自身。

- [ ] **BF102.17 — M06：地面制动扣除支撑面速度（P1）**
  - 依赖：BF102.15、BF102.02。
  - 验收：移动/旋转平台上记录相对速度、世界速度和制动输出，角色制动不抵消平台自身运动。

- [ ] **BF102.18 — M07：落地短暂 BrakingFrictionFactor 语义（P2）**
  - 依赖：BF102.17、BF102.31。
  - 验收：落地后的临时制动窗口、曲线与恢复时机有可复查速度轨迹。

- [ ] **BF102.19 — I04：Foot Lock 使用 movement-base 相对坐标（P1）**
  - 依赖：BF102.17。
  - 验收：移动/旋转平台上锁点按基座换算，角色与脚锁互不产生世界空间漂移。

- [ ] **BF102.20 — X01：Mantle 保留目标身份和相对变换（P1）**
  - 依赖：BF102.15、BF102.17。
  - 验收：移动、旋转、销毁攀爬目标时，每角色目标身份、相对变换和失效恢复均可追踪。

- [ ] **BF102.21 — X02：Mantle 临时碰撞状态拥有所有权并恢复原值（P1）**
  - 依赖：BF102.20。
  - 验收：攀爬、布娃娃和自定义碰撞禁用并存时，结束任一动作不会删除其他动作的状态或丢失原层。

- [ ] **BF102.22 — C06：相机 movement-base 生产状态接线（P2）**
  - 依赖：BF102.17、BF102.20。
  - 验收：移动/旋转平台时相机消费者收到与角色相同归属的基座状态。

- [ ] **BF102.23 — F09：Tnua 重力与每刚体 GravityScale 一致（P2）**
  - 依赖：BF102.17。
  - 验收：非 1 GravityScale 和自定义重力下，传感、Tnua 轨迹和角色物理使用同一实际重力。

### 动画底座与条件资源语义

- [ ] **BF102.24 — F01：InertializationNode 端点收敛与惯性语义（P1）**
  - 依赖：BF102.05。
  - 验收：过渡结束时无残留源姿态；源姿态继续求值与 UE 惯性混合差异有明确处理方案。

- [ ] **BF102.25 — F02：InertializedLinkedAnimLayer 复位/请求转发（P1）**
  - 依赖：BF102.09、BF102.24。
  - 验收：overlay 实现切换时 activation、复位、请求与消费标志与实际链接层输出一致。

- [ ] **BF102.26 — F03：浮点曲线后向 Cycle 外推周期（P2）**
  - 依赖：BF102.06。
  - 验收：超出末帧的 Cycle 采样返回正确周期值；无对应资产时明确保持条件性结论。

- [ ] **BF102.27 — F04：浮点曲线 cubic/tangent 载入语义（P2）**
  - 依赖：BF102.06、BF102.26。
  - 验收：未预烘焙 cubic 曲线保留切线/插值语义，或给出受控烘焙及数值误差 Gate。

- [ ] **BF102.28 — F05：CycleWithOffset/Oscillate 外推语义（P2）**
  - 依赖：BF102.26。
  - 验收：三类外推模式分别可采样、可区分，并按资产触发条件验证。

- [ ] **BF102.29 — F06：根运动旋转 weighted sum 不重复加权（P2）**
  - 依赖：BF102.09、BF102.24。
  - 验收：含 root rotation 的 normalized blendspace 以相等权重混合时保持正确旋转量。

- [ ] **BF102.30 — F07：Morph 权重线性混合保留起始值（P2）**
  - 依赖：BF102.24。
  - 验收：含 morph targets 的模型/动画在 alpha=0、alpha=1 和中间值均保持正确起点、终点和插值。

### 空中、落地与动作时间

- [ ] **BF102.31 — L01：重落地使用预碰撞竖直速度（P1）**
  - 依赖：BF102.05、BF102.02。
  - 验收：高落差着地记录预/后碰撞速度与 heavy landing 权重，动画输入使用预碰撞值。

- [ ] **BF102.32 — L02：主路径地面预测使用目标胶囊 sweep（P1）**
  - 依赖：BF102.17、BF102.23。
  - 验收：台阶边缘、斜落、窄台和缩放角色下，预测形状、命中、法线和输出曲线可复查。

- [ ] **BF102.33 — L03：起跳播放率在成功起跳时锁存（P2）**
  - 依赖：BF102.05。
  - 验收：空中水平速度变化不改写当前跳跃播放率；新起跳重新建立锁存值。

- [ ] **BF102.34 — L04：跳跃曲线时间积分播放率（P1）**
  - 依赖：BF102.33、BF102.26。
  - 验收：变速跳跃的曲线时间等于播放率积分而非 elapsed×当前速率，并与姿态相位一致。

- [ ] **BF102.35 — L05：jump_latched 基于成功起跳事件（P2）**
  - 依赖：BF102.05、BF102.33。
  - 验收：被拒绝起跳后走下边缘不会产生错误的已起跳状态；成功事件可追溯到动画和物理两侧。

- [ ] **BF102.36 — L06：落地滚翻使用确定性的撞击速度记录（P2）**
  - 依赖：BF102.31、BF102.04。
  - 验收：不同渲染 FPS 下自动落地滚翻阈值与选择一致，不依赖逐渲染帧 EMA 偶然值。

- [ ] **BF102.37 — L07：lean 使用当前曲线的加减速度归一化（P2）**
  - 依赖：BF102.17、BF102.18。
  - 验收：不同 gait 区间的起停/加速均以当前曲线值归一化，最终 lean 与数值探针一致。

- [ ] **BF102.38 — L08：播放率缩放来自实际动画网格世界缩放（P3）**
  - 依赖：BF102.06、BF102.15。
  - 验收：独立缩放 avatar 或缩放父级时，动画速度使用实际网格世界缩放而非替代来源。

- [ ] **BF102.39 — L09：原地转向移除额外视角速度衰减包络（P2）**
  - 依赖：BF102.11。
  - 验收：快速转动相机后停止时，触发阈值/延迟/播放率只受源合同定义的量控制。

### Foot IK、骨盆与动态过渡

- [ ] **BF102.40 — I01：脚局部旋转使用更新后的小腿父旋转（P1）**
  - 依赖：BF102.10。
  - 验收：腿 IK 修正后，脚世界朝向与由更新后父骨骼换算的局部旋转一致。

- [ ] **BF102.41 — I02：骨盆偏移先于最终双腿 IK 求解（P1）**
  - 依赖：BF102.40。
  - 验收：骨盆下移非零时，最终脚世界位置仍命中目标，且无事后整体下拉误差。

- [ ] **BF102.42 — I03：脚旋转完整消费地面法线和三维脚锁旋转（P1）**
  - 依赖：BF102.06、BF102.41。
  - 验收：斜坡和转身锁脚时，脚旋转、地面法线、锁定旋转和最终骨骼均可追踪。

- [ ] **BF102.43 — I05：传送和骨骼恢复重初始化 Foot Lock（P2）**
  - 依赖：BF102.15、BF102.42。
  - 验收：传送、动画重新有效、组件重装后锁点与插值状态不保留过期数据。

- [ ] **BF102.44 — I06：动态过渡检测水平脚锁漂移（P1）**
  - 依赖：BF102.19、BF102.42。
  - 验收：平地原地转身和水平位移触发条件同时检测高度与水平误差，资源选择/重入受控。

- [ ] **BF102.45 — I07：脚高/骨盆跟随使用源弹簧状态响应（P2）**
  - 依赖：BF102.32、BF102.41。
  - 验收：阶梯高度突变和不同帧率下的足端/骨盆响应符合可复查的弹簧状态轨迹。

### 攀爬、翻滚与布娃娃动作闭环

- [ ] **BF102.46 — X03：Mantle 轨迹使用根运动变换合成（P1）**
  - 依赖：BF102.04、BF102.20、BF102.29。
  - 验收：实际攀爬的姿态、根运动、目标相对变换和角色位移使用同一相位，先验证 slot 接线。

- [ ] **BF102.47 — X04：Mantle 结束不引入固定目标锁定收尾（P2）**
  - 依赖：BF102.46。
  - 验收：攀爬结束时目标锁定/恢复时机来自源合同，不能固定附加约 0.35 秒。

- [ ] **BF102.48 — X05：根运动适配器保留侧向位移与旋转（P2）**
  - 依赖：BF102.29、BF102.46。
  - 验收：非纯前向 root-motion clip 的平移和旋转分量均提交给正确角色控制器。

- [ ] **BF102.49 — X06：Mantle 权限、自动起始时间与缩放语义（P3）**
  - 依赖：BF102.20、BF102.38。
  - 验收：自定义权限、缩放角色和自动 start-time 配置分别有条件性结论和可复查的行为。

- [ ] **BF102.50 — X07：布娃娃退出按空中/落地分流并保留速度（P1）**
  - 依赖：BF102.15、BF102.31。
  - 验收：空中布娃娃恢复时保留退出速度并进入正确 locomotion 分支，落地恢复不误走空中分支。

- [ ] **BF102.51 — X08：布娃娃速度相关关节驱动（P2）**
  - 依赖：BF102.16、BF102.50。
  - 验收：布娃娃运动中关节驱动随源合同定义的速度变化，启动行为单独量测。

### 相机 lag、时间、模式与碰撞恢复

- [ ] **BF102.52 — C02：轴独立相机 lag 做 UE→Bevy 坐标映射（P2）**
  - 依赖：BF102.14。
  - 验收：各轴 LocationLag 不相同时，位置响应按映射后的轴和时间常数一致。

- [ ] **BF102.53 — C03：冲刺镜头震动使用冲刺动画实际时间（P2）**
  - 依赖：BF102.33、BF102.34。
  - 验收：任意时刻起跑和变播放率时，震动曲线时间与冲刺动画相位一致。

- [ ] **BF102.54 — C04：ignore_time_dilation 覆盖相机曲线过渡（P2）**
  - 依赖：BF102.53。
  - 验收：慢动作/暂停附近的模式切换中，所有声明去缩放的相机计算使用同一未缩放时间源。

- [ ] **BF102.55 — C05：第一人称 profile 使用 DesiredRotationMode（P2）**
  - 依赖：BF102.13。
  - 验收：VelocityDirection 与第一人称互切时，profile 选择跟随 desired 而非滞后的实际 rotation mode。

- [ ] **BF102.56 — C07：相机起点穿透恢复聚合多重 MTD（P2）**
  - 依赖：BF102.52。
  - 验收：墙角和多个重叠阻挡物下，起点恢复使用全部相关 MTD 而非首个 sweep 命中。

- [ ] **BF102.57 — C08：第一人称头部输入使用未缩放 dt（P2）**
  - 依赖：BF102.06、BF102.54。
  - 验收：第一人称慢动作下头部参数和曲线输入按声明的未缩放时间更新。

### 范围与最终 Gate

- [ ] **BF102.58 — Q02：网络范围与本地确定性边界（P2）**
  - 依赖：BF102.15、BF102.16、BF102.50。
  - 验收：网络缺失不被伪装成单机画面缺陷，A04/F08 等本地多实体错误不被推迟；扩展网络范围时另建清单。

- [ ] **BF102.Gate — 58 项原子审计总验收**
  - 依赖：BF102.01–BF102.58。
  - 验收：58 个 ID 各有“当前已修复／确认待修复／条件性不适用或待证”三态记录；每个确认缺陷有独立修复提交和真实运行 verdict；未执行项不得宣称完成。

## 附录 A：与 ALS 的刻意差异（非缺陷）

| ALS | bevy_als | 理由 |
|---|---|---|
| AnimBP 状态机 | 显式 system 路由 + animation graph RON | ECS 数据流，无隐藏图执行 |
| GameplayTags | 普通 Rust 枚举组件 | 编译期检查 |
| ControlRig | 分析式 IK systems | 无运行时 rig 编译 |
| Root Motion | 图内 `RootMotionMode` 提取 + `RootMotionOutput` 驱动位移（M8.8；数据经 M8.6 回填） | fork 已支持提取，M0.5 导出缺根轨道由 M8.6 离线回填，不再用曲线等价物 |
| Curves（UBT 资产） | als-ext CurveAsset + 采样节点 | 见 M2 |
| w_als_hud Overlay 菜单世界锚定头顶骨骼 | 菜单停靠 HUD 面板左侧（BF6b 用户决策 2026-08-25） | 解除 UI 对骨骼/相机投影的耦合 |
| W_Als_Hud 使用 Roboto 字体 | Bevy 默认字体（BF6b 用户决策 2026-08-25） | 字体非功能项，不移植 |
| L_Als_Playground TextRender 使用 RobotoDistanceField 与描边材质 | Bevy 默认字体布局/atlas 生成的 `Mesh3d` 世界字形四边形（BF-Scene 用户决策 2026-08-26） | 不带入字体资产；保留文字内容、颜色、世界锚点与固定朝向，不做相机 billboard |
| S_Als_UI_Click 菜单点击音 | 不复刻（BF6b 用户决策 2026-08-25） | WAV 解码需 bevy `wav` feature（hound 依赖），用户否决新增依赖 |

## 附录 B：风险与后置项

- **资产格式决策（2026-08-21）**：Bevy 官方推荐 glTF；M0.5/M0.6 管线定为 UE → glTF（Interchange 导出）→ bevy_animation_graph 自带的 glTF 导入器（GraphClip/Skeleton）。**不引入 bevy_ufbx**：其 v0.19.0（bevy 0.19 兼容）虽支持蒙皮数据，但动画曲线未接入 Bevy 动画系统、无社区验证，且 FBX 中转对 ALS 曲线/虚骨/单位换算风险更大；仅当未来需要直接加载 FBX 时再评估。

- bevy_animation_graph additive/mask 语义若与 ALS layering 不完全一致，在 M6 前验证并决定扩展方式（优先 als-ext 扩展点）。（已于 M6.1 验证通过，记录备查）
- Distance matching / motion matching 类高级特性仅实现 Stop 所需子集（M8.3）。
- ALSExtras（AI 控制器示例）不在范围内。
- 音频（Metasound）、Niagara 特效、ALSCamera 动画实例仅保留挂钩点。
