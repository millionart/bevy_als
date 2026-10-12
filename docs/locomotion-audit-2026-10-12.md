# ALS 运动系统复审 — 2026-10-12

目标基线：`5952afcd`。本轮限定 ALS 角色运动、动作、布娃娃与其动画消费者；场景/材质/UI/相机/示例物件不在范围内。截图关闭，性能验收暂停。原始 ALS 插件基线：`b754d6f0f2bb03741d301f8fb88077ebfe561e17`。

复审于 2026-10-12 15:48:46 +08:00 完成，确认 **6 项 P2**，未确认新的 P0/P1。审计阶段 23 个有效最小测试中，12 个通过对照、11 个语义失败对应这 6 项；核心动画源闭包和 Rig 产物审计通过。用户随后授权修复，当前修复状态以各项记录及 CHECKLIST 为准，原始反例保留。

只记录经本次本地 source、当前实际消费者和最小反例确认的行为差异；既有修复、刻意差异、无消费者字段和仅有症状的观察不重复算缺陷。独立测试/取证产物保留在 gitignored `target/bf140/`；审计不修改生产实现。

## 覆盖与结论

| 范围 | 状态 | 证据边界 |
|---|---|---|
| Character / CMC / 状态 | 已审计，4 项 P2 | 原始 Character、CMC、UE Character/CMC/Capsule/Actor 与当前运动/状态/时钟消费链；fresh B_Als_Character、CS_Als_Default、MS_Als_Normal |
| 动作 / Root Motion / Ragdoll | 已审计，新增 2 项 P2 | 原始 Actions/Mantling RMS、UE RootMotionSource/CMC 与 Roll/Mantle/GetUp/PA 消费者；fresh PA、Roll/High Mantle/Front+Back GetUp |
| 运动动画 / 图 / Rig 消费边界 | 已审计，未新增已确认 P2 | 9 个核心 AnimBP / 155 个源子图 / 256 个产物、fresh AIS 与 CR_Als；运动输入、Lean、Jump/Air、步幅/播放率、Turn/Rotate/Transition、Foot Lock 与 Rig 当帧消费 |

## 已确认差异

### P2-01 — 配置的跳跃按住时长没有使用角色时钟

修复完成（2026-10-12 16:37:17 +08:00）：直接把当帧 owner delta 传给现有 Jump input，由原 hold_elapsed 累积，并在活动期间刷新以支持途中变速。fresh Character/UE CMC/Actor/B_Als_Character、维护回归先红后绿、0/.5/1/2及途中暂停/变速矩阵通过；两个有效消融分别重现世界计时和时钟输入陈旧，精确字节恢复后同 target 6项回归通过。正式生产库原3个反例/对照、声明式合同及 target_game 构建通过；真实 startup/Base功能通过，Base53项覆盖齐全，graph/fault/temporal为空。截图0、性能仍延期，未新增通用时钟层或修正计时系统。开始16:00、预估16:55、实际16:37:17、偏差−17分43秒；证据在 `target/bf141/fix01/` 与 `target/animation_visual/bf141_fix01/`。以下保留原审计反例。

- Source：UE `Runtime/Engine/Private/Character.cpp:1609–1625` 的 `ClearJumpInput` 以 CMC 传入的 `DeltaSeconds` 累积 `JumpKeyHoldTime`；`CharacterMovementComponent.cpp:3027` 与 `Actor.h` 的组件 tick helper 共同确认该时间包含 owner `CustomTimeDilation`。`DoJump` 在按住窗口内持续保证最小起跳速度。
- Target：`src/als/character/movement/control_scheme.rs:71–94` 的 `hold_elapsed` 直接累积 Tnua `ctx.frame_duration`；Tnua controller 使用全局 `Time`。普通 Falling 已用角色倍率推进，按住窗口仍使用世界时间。
- 反例：`max_hold_time=0.2`、60 Hz、一直按住 Jump。倍率 0.5 的第 12 帧，角色只经过 0.1 秒，实际竖直速度为 4.0365 m/s，源应继续保持 4.11825 m/s；倍率 2 的第 6 帧，实际 3.873 m/s，源应已结束按住而下降到 3.546 m/s。倍率 1 的完整 20 帧对照通过。
- 影响：慢放时跳跃持续推力提前结束，加速时延后结束，轨迹与角色时钟不一致。默认 `JumpMaxHoldTime=0` 不触发；这是项目已明确支持且已有合同的非零按住配置。
- 漏检：`configured_source_jump_hold_maintains_launch_velocity_until_release` 覆盖帧长/速度/按住长度，未与角色倍率交叉；BF139-P2-03 的普通 CMC 时钟修复没有覆盖 Tnua Jump 的独立计时。

### P2-02 — 角色父级缩放未进入普通地面尺寸计算

修复完成（2026-10-12 17:50:45 +08:00）：CMC/FindFloor/落地读取当帧世界尺寸，蹲起读取 world shape scale 和世界净空；普通 CMC、状态同步、蹲起及基座携带按 inverse-parent 写回局部 Transform，Physics Position 保留世界语义。父级.5悬空、2倍台阶/低顶棚反例、缓存Global未覆盖的current-transform边界先红后绿；维护夹具补齐真实入口的最终传播。7个消费者消融均语义FAIL，精确字节恢复后同target10项回归通过；正式库原反例、声明式合同、正式构建通过。真实startup和三座原楼梯上下六路线功能通过，2275帧确认父级2/局部.5、世界角色尺寸1，graph/fault/temporal为空。开始16:40、预估17:35、实际17:50:45、偏差+15分45秒；没有新增生产状态/适配层或修改图与原始资产。证据在 `target/bf141/fix02/` 与 `target/animation_visual/bf141_fix02/`；截图0、性能延期。以下保留原审计反例。

- Source：`AlsCharacterMovementComponent.cpp:644` 调用 `GetScaledCapsuleSize`；UE `CapsuleComponent.h:202–211` 从 component world scale 计算半径和半高。
- Target：`src/als/character/movement/motion.rs:3714–3715,3960–3964,4046–4047` 使用角色局部 `Transform.scale` 构造步阶/FindFloor/落地合同，而实际 Avian 胶囊包含父级缩放。
- 反例：同一平地，世界胶囊半径 0.6 m、半高 1.8 m，分别由角色自身 scale=2 和父实体 scale=2 得到。两者真实碰撞尺寸一致；稳定运行 10 帧后，前者 `Grounded`，父级缩放者为 `Air`，物理中心分别 1.8227272 m 和 1.8215 m。
- 影响：处于正常地面接触的角色会进入空中运动/动画分支，普通地面跳跃入口也依赖该状态。问题在地面查询尺寸，不是碰撞体没有缩放。
- 漏检：旧父空间 Gate 检查旋转/位置，缩放 Gate 使用角色自身缩放；没有等世界尺寸、不同父级分解的反例。父级平移下的蹲起及视觉/物理位置对照本轮通过，不把全部父空间路径概括为错误。

### P2-03 — 静止 VelocityDirection 旋转错误地随镜头转速加快

修复完成（2026-10-12 18:09:44 +08:00）：静止 VelocityDirection 的现有 half-life 直接返回，保留移动与 ViewDirection 的镜头转速倍率。两姿态×五档正负镜头变化的维护反例先红后绿，移除分支再次语义 FAIL；精确字节恢复、同 target 重编后11项运动回归及移动对照通过，正式优化构建中的反例亦通过。fresh Character/MS_Als_Normal、声明式合同、startup与standing_response原场景6项功能覆盖通过，graph/fault/temporal为空。开始18:00、预估18:35、实际18:09:44、偏差−25分16秒；只增加提前返回条件，没有新增生产状态/系统或修改图与资产。证据在 `target/bf141/fix03/` 与 `target/animation_visual/bf141_fix03/`；截图0、性能延期。以下保留原审计反例。

- Source：`AlsCharacter.cpp:1546–1576` 的静止 VelocityDirection 分支固定使用 0.1 秒 half-life；镜头转速倍率属于 `CalculateGroundedMovingRotationInterpolationHalfLife`，该分支不调用它。
- Target：`src/als/character/movement/motion.rs:314–327` 取出静止的 0.1 后，仍统一乘以镜头转速倍率。
- 反例：Grounded、速度 0、无输入，保留目标 yaw=1 rad，60 Hz。镜头不变时实际/源均约 0.109366 rad；镜头在本帧转动 1 rad 时，实际 actor yaw=0.292778 rad，源仍应为 0.109366 rad。
- 影响：停止后尚未完成的身体转向会被转动镜头意外加速；速度朝向模式的静止分支与 source 不符。
- 漏检：现有 responsiveness 测试只覆盖移动中的 ViewDirection；静止测试没有在同一源状态下改变镜头转速。

### P2-04 — Actual Gait 错用了方向限速阈值

修复完成（2026-10-12 18:29:54 +08:00）：现有公共 helper 在启用方向速度时取前后最大 walk/run，运行系统调用该 helper；GaitAmount 和物理限速继续使用当前方向插值。旧错误 oracle 及后退上限较高反例先红后绿，7组方向速度矩阵、开关边界、两个语义消融和同 target 字节恢复后的11项步态回归通过；正式库原4个反例/对照通过。fresh Character/MovementSettings/CMC/MS_Als_Normal、声明式合同、正式构建、startup与standing_direction功能覆盖通过，graph/fault/temporal为空。开始18:11、预估18:40、实际18:29:54、偏差−10分06秒；归档BF102.03仅ActualGait oracle有限复开后重新验收，其他历史证据保留。证据在 `target/bf141/fix04/` 与 `target/animation_visual/bf141_fix04/`；截图0、性能延期。以下保留原审计反例。

- Source：`AlsCharacter.cpp:978–997` 的 `CalculateActualGait` 使用 `GaitSettings.GetMaxWalkSpeed/GetMaxRunSpeed`；`AlsMovementSettings.h:85–97` 在开启方向速度后取前后速度的较大值。CMC 的方向限速/GaitAmount 与 Character 的 ActualGait 是不同合同。
- Target：`src/als/state/gait.rs:321–329` 把当前方向插值得到的同一组阈值同时用于 GaitAmount 和 ActualGait。
- 反例：启用前后不同速度，Walk 前/后=1.75/1.0 m/s，Run 前/后=3.75/2.0 m/s。后退速度 1.2 m/s 时实际 `Running`，源应 `Walking`；允许 Sprint、后退速度 2.5 m/s 时实际 `Sprinting`，源应 `Running`。同速度的前进对照均通过。
- 影响：提前发布实际步态；`ab_als_provider.rs:93,398` 消费 Gait 选择方向和 Sprint 动画输入，因此不只是调试标签不同。默认 MS_Als_Normal 关闭方向速度，此问题在源与目标均公开支持的方向速度配置触发。
- 漏检：`refresh_gait_uses_direction_dependent_thresholds` 测试直接固化了错误期望。速度限制/连续 GaitAmount 的正确性不能证明 ActualGait 使用了正确边界。

### P2-05 — Roll / Mantle 把物理桥速度直接发布为源运动速度

修复完成（2026-10-12 20:59）：Roll保留动态物理world桥，通过既有CMC通道恢复source速度；碰撞预算、基座与当帧producer所有权闭合，52 Roll/102运动控制、10项有效消融、正式生产库3反例及startup→grounded16覆盖全部通过。Mantle的Custom积分跳过普通物理，可直接发布owner速度；这收窄了原审计对world桥的推断，重复桥/恢复/额外清理经消融删除。18项Mantle回归、9组完整角色/24组parent矩阵、首次/中途冻结及移交、4项有效消融与精确恢复、正式生产库3反例/构建/声明式合同通过；startup→Base53覆盖和挂接后92条world/physics记录全部功能PASS。所有公共Gate的graph/fault/temporal为空、primary_failure=null，截图0、性能延期。首轮超时/拆分与证据见 `CHECKLIST.md` BF141.05，详细记录在 `target/bf141/fix05a/`、`target/bf141/fix05b/` 及对应公共报告。以下保留原审计反例。

- Source：UE `CharacterMovementComponent.cpp:2156–2161,2941,2968` 以 owner CMC `DeltaSeconds` 计算动画根运动速度；`RootMotionSource.cpp:1608` 把同一 owner delta 传给 RMS。`AlsRootMotionSource_Mantling.cpp:122–124` 同样以该 delta 除世界位置差。`AlsCharacter.cpp::RefreshLocomotion` 消费 CMC Velocity。
- Target：`src/als/character/actions/roll_drive.rs:2237` 和 `src/als/root_motion_sources/mantle_drive.rs:735` 以世界 dt 得到物理桥速度；`motion.rs:2682–2689` / `actual_motion.rs:89–91` 将其直接发布为实际运动速度，没有恢复 source 的 owner-clock 语义。
- 完整角色反例：Roll 在倍率 0.5 时，实际发布前向速度 1.335909 m/s，源同一根轨道增量对应 2.671818 m/s；倍率 2 时为 12.119965 对 6.059982 m/s。Mantle 两种倍率也分别为源速度的 0.5 倍和 2 倍；倍率 1 对照通过。Mantle oracle 读取驱动实际消费的 actor world 起点再除 owner delta，排除了旧父空间问题。
- 影响：动作位移可以正确，但 `AlsActualMotion`、动画输入、后续惯性移交及读取角色速度的逻辑收到错误速度。世界时钟物理桥本身有作用，不能简单把它的除数改成 owner delta 而破坏位移；应在修复时核对物理桥与源速度的消费边界。
- 漏检：旧动作时钟 Gate 检查播放时间和位移；BF139-P2-07 的速度 oracle 明确以世界 0.05 秒相除，复制了此处错误语义。此次只登记新证据，不改写或撤销旧报告。

### P2-06 — Ragdoll 恢复清除了 actor 的 Pitch / Roll

- Source：`AlsCharacter_Actions.cpp:1101–1104` 从 `GetActorRotation()` 创建 NewActorRotation，只替换 Yaw 后调用 SetActorLocationAndRotation。
- Target：`src/als/character/actions/ragdoll_link.rs:934–938,986–989` 构造纯 yaw 四元数并替换整个 actor rotation。
- 完整角色反例：布娃娃已稳定，退出前 actor pitch=0.2 rad、roll=−0.1 rad；请求退出并实际进入 GettingUp 后，目标两者均为 0，源应保留。零倾斜对照通过。
- 影响：带倾斜的 actor / 挂接姿态在恢复时突然被扶正，胶囊和 mesh 恢复坐标也随之改变。默认直立角色不触发。
- 漏检：原父空间恢复测试只改变父级 yaw/位置/缩放，并直接把 `ragdoll_recovery_rotation` 的纯 yaw 结果作为期望，未覆盖源“只修改 Yaw”的边界。

## 验证与排除

BF140.01（15:00–15:12:25 +08:00）：当次 native reader 读取 B_Als_Character、CS_Als_Default、MS_Als_Normal，均与原始插件资产逐字节一致；保留 9 个 C++/header 源快照与 SHA-256。`target_game/play` 当前生产库 SHA-256 为 `5472f9f72358de503f6e823ad5712d270ccff3bb4d9e6fc698676d3dc7c7a4ea`。

独立链接该库的 15 个最小测试在 0.99 秒完成：9 个对照通过、6 个预期语义失败，归并为以上 4 项 P2；退出 101 是断言失败，不是验收通过。编译夹具的一次 Rust 借用生命周期错误已纠正，不计作生产缺陷。证据保存在 `target/bf140/phase1/`，源记录在 `target/bf140/source/`。watchdog 的正常完成、无进展终止、子进程清理及 JSON 报告自测全部通过。

本轮明确排除：普通 gait permission 已读取归一化消费输入；动画加速度在 provider 中补偿 owner delta；静止/上升/下降基座起跳的 signed vertical velocity 继承均通过真实运动系统对照；source 的 InAirRotationMode 原始值 3 落入 default 保持分支，与 target 一致。没有把原生枚举显示为 INVALID 单独列为缺陷。

未新增生产抽象、状态、分支或转换规则，设计消融不适用。此次是只读差异审计，未运行 Playground 修复验收，不将独立反例替代未来修复所需的真实入口 Gate。

BF140.02（15:14–15:26:28 +08:00）：fresh PA / Roll / High Mantle / GetUp Front+Back 与原始资产逐字节一致；原始 Actions、Mantling RMS 与 UE RootMotionSource 已保存当次快照。复用转换函数重建 PA 的 21 body / 20 constraint / 49 disabled pair，与安装 profile 完全一致；重建完整 Ragdoll/GetUp 设置也一致，PA 转换 self-test 通过。首次使用 Build 实目录查询项目 angular-velocity 配置被工具拒绝，已改用实际 uproject 的插件挂载路径，不计作缺陷。

累计 23 个最小反例/对照在 9.66 秒完成：12 PASS、11 预期语义 FAIL，归并 6 项 P2。新增 8 个实际加载角色测试覆盖两种 Root Motion 的 0.5/1/2 倍率与 Ragdoll 退出倾斜，资产就绪和动作前置条件均通过。证据在 `target/bf140/phase2/` 与 `action_facts.json`。Mantle 的 no-sweep Custom 移动、目标销毁/动作优先级、GetUp 输入解除、Ragdoll ground query/当前父空间与初始限速仍与已核 source 合同相符，未重复登记已修 BF139 问题。

BF140.03（15:28–15:48:46 +08:00）：当次重读 AB_Als 及其 8 个 linked AnimBP、CR_Als、AIS_Als_Default，共 11 个 native 资产；全部与原始插件资产逐字节一致。沿 AB_Als 的实际默认角色引用闭包，源合同审计覆盖 155 个子图、57 个 pose graph、9 个 FSM、77 个 condition graph、491 个源节点和 393 条 pose 连线。247 份当前运行时节点规格重新读取，256 个已安装 RON/FSM 与最后完整编译产物一致；source closure、该范围 editor validation、runtime asset boundary 及 closure self-test 全部通过。此处没有修改转换规则，未运行全库重转，也未将周边 Overlay 物件或 Camera 加入审计。

CR_Als 使用本次 native 记录重新编译合同和 target program，165 条指令、79 根骨骼、43 条曲线及完整 program 与安装文件完全一致；当次 AIS 确认站立/蹲姿共享 Walk stride 曲线、Turn 延迟区间 0→0.75、Lean/Rotate/FootLock 阈值与现有消费者相符。已保留 AnimationInstance 与两种动作 Notify 的当次 C++ 快照。证据为 `target/bf140/graph_closure.json`、`animation_facts.json`、`graph_source_flat/` 和相应 watchdog JSON。

排除候选：Air 时 provider 保留非零 max-braking 数据，但完整角色起跳的 14 帧没有观察到实际 Grounded refresh 消费；原观察及失败的“必须有消费者”夹具保存在 `target/bf140/observations/`，不计为语义失败或第 7 项缺陷。已从有效反例集移除，最终 23 项重新编译执行为 12 PASS / 11 预期 FAIL，9.89 秒完成，无加载、超时或夹具前置失败。旧固定步长 UphillJump 腿平面观察仍缺少本轮可确认的 source-target 根因，不据此复开已完成图。

最终边界：共 19 个 fresh native 资产、15 个 C++/header 快照；生产库 SHA-256 始终不变。六项均有独立触发条件和对照，尚未实施修复；本轮未进行截图、性能验收或完整 Playground 功能总验收。
