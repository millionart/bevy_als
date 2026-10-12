# ALS 运动系统复审 — 2026-10-12

目标基线：`5952afcd`。本轮限定 ALS 角色运动、动作、布娃娃与其动画消费者；场景/材质/UI/相机/示例物件不在范围内。截图关闭，性能验收暂停。原始 ALS 插件基线：`b754d6f0f2bb03741d301f8fb88077ebfe561e17`。

只记录经本次本地 source、当前实际消费者和最小反例确认的行为差异；既有修复、刻意差异、无消费者字段和仅有症状的观察不重复算缺陷。独立测试/取证产物保留在 gitignored `target/bf140/`；审计不修改生产实现。

## 覆盖与结论

| 范围 | 状态 | 证据边界 |
|---|---|---|
| Character / CMC / 状态 | 已审计，4 项 P2 | 原始 Character、CMC、UE Character/CMC/Capsule/Actor 与当前运动/状态/时钟消费链；fresh B_Als_Character、CS_Als_Default、MS_Als_Normal |
| 动作 / Root Motion / Ragdoll | 待审计 | 尚无本轮结论 |
| 运动动画 / 图 / Rig 消费边界 | 待审计 | 尚无本轮结论 |

## 已确认差异

### P2-01 — 配置的跳跃按住时长没有使用角色时钟

- Source：UE `Runtime/Engine/Private/Character.cpp:1609–1625` 的 `ClearJumpInput` 以 CMC 传入的 `DeltaSeconds` 累积 `JumpKeyHoldTime`；`CharacterMovementComponent.cpp:3027` 与 `Actor.h` 的组件 tick helper 共同确认该时间包含 owner `CustomTimeDilation`。`DoJump` 在按住窗口内持续保证最小起跳速度。
- Target：`src/als/character/movement/control_scheme.rs:71–94` 的 `hold_elapsed` 直接累积 Tnua `ctx.frame_duration`；Tnua controller 使用全局 `Time`。普通 Falling 已用角色倍率推进，按住窗口仍使用世界时间。
- 反例：`max_hold_time=0.2`、60 Hz、一直按住 Jump。倍率 0.5 的第 12 帧，角色只经过 0.1 秒，实际竖直速度为 4.0365 m/s，源应继续保持 4.11825 m/s；倍率 2 的第 6 帧，实际 3.873 m/s，源应已结束按住而下降到 3.546 m/s。倍率 1 的完整 20 帧对照通过。
- 影响：慢放时跳跃持续推力提前结束，加速时延后结束，轨迹与角色时钟不一致。默认 `JumpMaxHoldTime=0` 不触发；这是项目已明确支持且已有合同的非零按住配置。
- 漏检：`configured_source_jump_hold_maintains_launch_velocity_until_release` 覆盖帧长/速度/按住长度，未与角色倍率交叉；BF139-P2-03 的普通 CMC 时钟修复没有覆盖 Tnua Jump 的独立计时。

### P2-02 — 角色父级缩放未进入普通地面尺寸计算

- Source：`AlsCharacterMovementComponent.cpp:644` 调用 `GetScaledCapsuleSize`；UE `CapsuleComponent.h:202–211` 从 component world scale 计算半径和半高。
- Target：`src/als/character/movement/motion.rs:3714–3715,3960–3964,4046–4047` 使用角色局部 `Transform.scale` 构造步阶/FindFloor/落地合同，而实际 Avian 胶囊包含父级缩放。
- 反例：同一平地，世界胶囊半径 0.6 m、半高 1.8 m，分别由角色自身 scale=2 和父实体 scale=2 得到。两者真实碰撞尺寸一致；稳定运行 10 帧后，前者 `Grounded`，父级缩放者为 `Air`，物理中心分别 1.8227272 m 和 1.8215 m。
- 影响：处于正常地面接触的角色会进入空中运动/动画分支，普通地面跳跃入口也依赖该状态。问题在地面查询尺寸，不是碰撞体没有缩放。
- 漏检：旧父空间 Gate 检查旋转/位置，缩放 Gate 使用角色自身缩放；没有等世界尺寸、不同父级分解的反例。父级平移下的蹲起及视觉/物理位置对照本轮通过，不把全部父空间路径概括为错误。

### P2-03 — 静止 VelocityDirection 旋转错误地随镜头转速加快

- Source：`AlsCharacter.cpp:1546–1576` 的静止 VelocityDirection 分支固定使用 0.1 秒 half-life；镜头转速倍率属于 `CalculateGroundedMovingRotationInterpolationHalfLife`，该分支不调用它。
- Target：`src/als/character/movement/motion.rs:314–327` 取出静止的 0.1 后，仍统一乘以镜头转速倍率。
- 反例：Grounded、速度 0、无输入，保留目标 yaw=1 rad，60 Hz。镜头不变时实际/源均约 0.109366 rad；镜头在本帧转动 1 rad 时，实际 actor yaw=0.292778 rad，源仍应为 0.109366 rad。
- 影响：停止后尚未完成的身体转向会被转动镜头意外加速；速度朝向模式的静止分支与 source 不符。
- 漏检：现有 responsiveness 测试只覆盖移动中的 ViewDirection；静止测试没有在同一源状态下改变镜头转速。

### P2-04 — Actual Gait 错用了方向限速阈值

- Source：`AlsCharacter.cpp:978–997` 的 `CalculateActualGait` 使用 `GaitSettings.GetMaxWalkSpeed/GetMaxRunSpeed`；`AlsMovementSettings.h:85–97` 在开启方向速度后取前后速度的较大值。CMC 的方向限速/GaitAmount 与 Character 的 ActualGait 是不同合同。
- Target：`src/als/state/gait.rs:321–329` 把当前方向插值得到的同一组阈值同时用于 GaitAmount 和 ActualGait。
- 反例：启用前后不同速度，Walk 前/后=1.75/1.0 m/s，Run 前/后=3.75/2.0 m/s。后退速度 1.2 m/s 时实际 `Running`，源应 `Walking`；允许 Sprint、后退速度 2.5 m/s 时实际 `Sprinting`，源应 `Running`。同速度的前进对照均通过。
- 影响：提前发布实际步态；`ab_als_provider.rs:93,398` 消费 Gait 选择方向和 Sprint 动画输入，因此不只是调试标签不同。默认 MS_Als_Normal 关闭方向速度，此问题在源与目标均公开支持的方向速度配置触发。
- 漏检：`refresh_gait_uses_direction_dependent_thresholds` 测试直接固化了错误期望。速度限制/连续 GaitAmount 的正确性不能证明 ActualGait 使用了正确边界。

## 验证与排除

BF140.01（15:00–15:12:25 +08:00）：当次 native reader 读取 B_Als_Character、CS_Als_Default、MS_Als_Normal，均与原始插件资产逐字节一致；保留 9 个 C++/header 源快照与 SHA-256。`target_game/play` 当前生产库 SHA-256 为 `5472f9f72358de503f6e823ad5712d270ccff3bb4d9e6fc698676d3dc7c7a4ea`。

独立链接该库的 15 个最小测试在 0.99 秒完成：9 个对照通过、6 个预期语义失败，归并为以上 4 项 P2；退出 101 是断言失败，不是验收通过。编译夹具的一次 Rust 借用生命周期错误已纠正，不计作生产缺陷。证据保存在 `target/bf140/phase1/`，源记录在 `target/bf140/source/`。watchdog 的正常完成、无进展终止、子进程清理及 JSON 报告自测全部通过。

本轮明确排除：普通 gait permission 已读取归一化消费输入；动画加速度在 provider 中补偿 owner delta；静止/上升/下降基座起跳的 signed vertical velocity 继承均通过真实运动系统对照；source 的 InAirRotationMode 原始值 3 落入 default 保持分支，与 target 一致。没有把原生枚举显示为 INVALID 单独列为缺陷。

未新增生产抽象、状态、分支或转换规则，设计消融不适用。此次是只读差异审计，未运行 Playground 修复验收，不将独立反例替代未来修复所需的真实入口 Gate。
