# ALS 移植差异审计 — 2026-10-11

审计目标：记录当前实现中经本地 ALS-Refactored / UE source 与实际 target 消费链确认的全部 P2 及以上差异。目标基线：`bb3dccf3066a217b750670918e30f248ef2c5b98`。审计仍在进行。

本报告的源路径相对于 ALS 插件的 `Source/` 或注明的 UE `Engine/Source/`；target 路径相对于本仓库。机器本地路径只使用 gitignored 环境配置，不进入记录。

## 判定口径

- P0：普遍阻断使用或产生严重不可恢复后果。
- P1：核心功能在常用、受支持路径中明显失效，需优先处理。
- P2：受支持场景、配置或生命周期下产生可观察的行为/姿态/物理错误，值得修复。
- 仅实现形式、编辑器外观、无消费者的代码差异、明确接受的差异和缺乏源证据的症状不计入。

每项必须包含 source 和 target 的具体位置、触发条件、实际影响、最小反例、现有验证的漏检边界。已修历史问题仅用于去重；待办未勾选不等于存在缺陷。本轮只记录，不实施修复。

## 覆盖台账

| 范围 | 状态 | 本轮边界 |
|---|---|---|
| Character、CMC、输入与标签状态 | 已审计，5项P2 | 本地原始 C++、当前 ECS 调度、初始化/状态/输入/旋转/速度/蹲起/基座/碰撞/时钟的实际消费者；网络与纯 Editor 排除；P2-01已撤销 |
| Roll、Mantle、GetUp、root motion | 已审计，新增3项P2 | 启动/抢占/退出、蒙太奇设置、slot/曲线/notify输入、root、动作时钟与目标基座 |
| Ragdoll、物理适配 | 已审计，新增3项P2 | 21body/22shape/20constraint/49禁碰pair，进入/跟随/退出、PA配置/驱动/碰撞/查询/重建/CCD/DOF/sleep/缩放 |
| AnimationInstance、状态输入与Feet | 已审计，扩展P2-05/09 | fresh AIS与原始逐帧/相关性回调；Head/Spine/姿态曲线、Grounded/Standing/Crouching/Air/Feet/Turn；19个最小测试 |
| 图/FSM/同步/曲线、资产转换与 Rig | 进行中 | 源递归依赖到已安装图及实际求值消费者 |
| 通知、脚步/贴花、Overlay 物件 | 待审计 | 通知时间/权重/结束与最终姿态消费 |
| Camera、Playground、材质、HUD/UI、PlayerStart | 待审计 | 本地源及资产配置到真实入口 |

## 已确认 P2 及以上

### P2-02 — DesiredRotationMode=Aiming 被降为 ViewDirection

- Source：`ALS/Private/AlsCharacter.cpp:675` 的 `RefreshRotationMode` 用 `bDesiredAiming || DesiredRotationMode == Aiming`；源公开的 `SetDesiredRotationMode` 允许直接选择 Aiming，无需持续按 Aim。
- Target：`src/als/state/aim.rs:55` 只以 `aim_held` 计算意图，`:57` 把存储的 Aiming 改为默认自由模式。`AlsLastFreeRotationMode` 确实承担 desired rotation：`src/als_camera/mod.rs:427` 用它实现源相机的 desired-mode 分支，调试 HUD 也将其展示为 DesiredRotationMode。
- 反例：Grounded/Standing、MaxAllowedGait=Running、desired=Aiming、`aim_held=false`。实际为 ViewDirection，源为 Aiming；当前生产 `AlsAimPlugin` 断言失败。
- 影响：程序设置/初始配置的瞄准模式无法保留，身体朝向及相机模式分歧。`last_free_mode_never_stores_aiming` 测试固化了比源更窄的状态集合。
- 排除：普通 `AlsStateRequest.rotation_mode` 被刷新覆盖的现象不单独计数；source 的实际模式 setter 同样受 desired-mode 刷新支配，不能仅凭这一现象定缺陷。

### P2-03 — 普通 CMC 运动没有消费角色 CustomTimeDilation

- Source：UE `Runtime/Engine/Classes/GameFramework/Actor.h:4877` 的 `FActorComponentTickFunction::ExecuteTickHelper` 将组件 Tick Delta 乘 owner `CustomTimeDilation`；CMC 的 Walking/Falling 都消费该 Delta。
- Target：`src/als/character/movement/dynamics.rs:441` 和 `src/als/character/movement/motion.rs:3447` 直接使用全局 `Time`；后者将 `time.delta_secs()` 传入 Falling 事务。`AlsCameraCustomTimeDilation` 已被角色旋转、动画和已修复动作使用，但普通移动未接入。
- 反例：Air、水平速度 3.75 m/s、50 ms 帧、角色倍率 0.5。源水平位移应为 0.09375 m，实际为 0.1875 m；竖向速度也按完整 50 ms 的重力推进。当前真实 ECS/物理消费者断言失败，倍率 1 的无落地运动对照通过。
- 影响：角色慢放/加速时，普通位移与自身动画、旋转和动作时钟脱节。此前 owner-clock Gate 覆盖 GetUp/Roll/Mantle，未覆盖普通 CMC；本项不复开这些已完成动作。

### P2-04 — Falling 首个子步落地后丢失剩余 Walking 时间

- Source：UE `Runtime/Engine/Private/Components/CharacterMovementComponent.cpp:5050` 在合法落点将子步剩余时间加入 `remainingTime`；`:6373` 的 `ProcessLanded` 最终调用 `StartNewPhysics(remainingTime, Iterations)`，继续当前帧的地面运动。
- Target：`src/als/character/movement/motion.rs:2697` 在发现可走地面后直接 `break`，返回接口不携带剩余时间，也没有切入 Walking。之后 `locomotion_state_sync_system` 只更新模式/速度。
- 反例：胶囊中心高 1 m、下落速度 3 m/s、水平速度与前向输入对应 3.75 m/s、200 ms 帧、50 ms CMC 子步、平地。源在落地后继续行走，水平位移约 0.75 m；实际仅 0.10516852 m，帧末已 Grounded、水平速度仍 3.75 m/s。相同输入在高空无落地时为 0.75 m，正对照通过。下落速度低于源 Roll/Ragdoll 阈值，排除了落地动作切换。
- 影响：卡顿帧落地明显少走距离；空中末速度、中点、墙面尾段和“最终进入 Grounded”测试均不能检出跨模式后的时间丢失。

### P2-05 — 普通角色/动画输入把局部旋转当世界旋转

- Source：`ALS/Private/AlsCharacter.cpp:1680` 等旋转函数使用 `GetActorRotation`/`SetActorRotation`；`ALS/Private/AlsAnimationInstance.cpp:730` 从 actor world transform 读取旋转，`:826` 将世界速度反旋到角色局部空间。
- Target：`src/als/character/movement/motion.rs:415` 从局部 `Transform.rotation` 得到当前 yaw，`:577–579` 又把这个旋转同时写给局部 Transform 和世界 `avian3d::Rotation`。`src/als/character/animation_instances/actual_motion.rs:90`、`ab_als_provider.rs:319` 和 `locomotion_layer.rs:209` 同样把局部旋转作为世界姿态输入。
- 反例一：父实体 yaw=90°、角色 local identity、沿角色世界前方以 2 m/s 运动。局部速度应为 `(forward=2,right=0)`，实际 `(0,-2)`；无父实体、相同世界方向的对照通过。
- 反例二：真实移动插件中给静止角色挂平移+90°父实体，并保持其原世界姿态。更新后渲染 world yaw 约 0°，物理 Rotation 为 −90°。位置本次保持一致，因此不据此声称位置一定出错。
- 影响：挂接/分组角色的朝向、方向动画、Lean 和物理旋转不同步。此前父空间 Gate 只覆盖 Roll/Ragdoll/Camera 等消费者，没有覆盖普通 locomotion/provider。
- BF139.04追加：`head_layer.rs:72–118,166` 也读取局部Transform推导world yaw/pitch；完整角色在物理后固定local identity、父级yaw90时，实际Head/Spine共享输入 `locomotion_state.rotation_yaw=0`，源应90，第三个消费者反例FAIL。该父空间根因继续归并本项。

### P2-06 — 普通台阶跨越忽略组件 CanCharacterStepUp 许可

- Source：UE `Runtime/Engine/Private/Components/CharacterMovementComponent.cpp:7525` 的 `CanStepUp` 检查组件 `CanCharacterStepUp` 与 actor `CanBeBaseForCharacter`；`StepUp` 必须通过该检查。ALS Mantle 也使用同一个原生组件许可。
- Target：`src/als/character/movement/motion.rs:3380` 的低台阶事务只检查碰撞/高度/空间，没有读取已经由 `AlsMantleTargetPermission.allows_step_up` 表达的组件许可；该许可目前只被 `mantle.rs:569` 消费。
- 反例：20 cm 静态台阶，角色持续前进 60 个 1/60 s 帧，分别设置许可 true/false。两次均跨过台阶并到达 z=−3.4291668；源 false 应在台阶正面被阻挡。true 正对照通过，false 断言失败。
- 影响：明确禁止角色跨越的碰撞体仍能通过普通走路登上。现有许可测试只触发 Mantle，未经过普通 StepUp。

### P2-07 — Mantle 的 root 速度混用了 world 与 local 位置

- Source：`ALS/Private/RootMotionSources/AlsRootMotionSource_Mantling.cpp::PrepareRootMotion` 用 `(TargetActorTransform.Location - Character.GetActorLocation()) / DeltaTime`。
- Target：`src/als/root_motion_sources/mantle_drive.rs:727` 减去局部 `Transform.translation`；`:729` 与移动目标回写 `:795` 也直接将世界变换写入局部字段。
- 反例：完整加载角色，保持相同世界起点与 High Mantle 目标，仅增加平移 `(5,0,0)` 的父实体。普通角色横向速度约 `−4.09e−8` m/s，有父级后为 `299.99997` m/s，断言 FAIL。动作、root clip 和当前源形状均相同。
- 影响：挂接角色产生错误 root 速度，并影响后续运动/物理移交。已有 Mantle owner-clock 测试使用无父实体的 Transform，没有约束世界空间速度。

### P2-08 — Roll/Mantle 重新处理原始输入，丢失源输入有效性

- Source：`AlsCharacter_Actions.cpp:34` 和 `:176` 读取已由 Character 消费、归一化的 `LocomotionState.bHasInput/InputYawAngle`；有效性阈值为平方 `1e−8`。
- Target：`roll.rs:57`、`roll_drive.rs:1931` 又使用平方 `1e−6`。Mantle 则由 `motion.rs` 把原始输入先变成单位方向，再交 `mantle.rs:469` 判断。
- 反例：实际角色输入 `(0.0002,0)` 已 `has_input=true`，源应向右 Roll；实际目标 yaw=0，源 −90°。输入 `(0.01,0)` 正对照正确。另以 `(0.00001,0)` 验证已 `has_input=false` 且无水平速度，实际 Mantle trace 为 `(0.76604444,0,−0.6427875)`，源应 `(0,0,−1)`，相差50°。
- 影响：模拟量边界的动作方向和攀爬候选选择错误。普通四向/无输入测试绕开了源 safe-normal 边界和真实已消费状态。

### P2-09 — Roll/GetUp/Transition 的自然混出时序不同

- Source：UE `Animation/AnimInstance.cpp:573–578` 先调用 `Montage_UpdateWeight`，再调用 `Montage_Advance`。`Animation/AnimMontage.cpp:2626–2636` 在本次 Advance 首次进入窗口时才执行 Stop，默认负 TriggerTime 使用当前剩余播放时间；Stop 只 `Blend.Update(0)`，不会在该帧额外推进混出。
- Target：`roll.rs:48` 与 `get_up.rs:112–130` 根据推进后的 elapsed、固定默认 BlendOut 直接计算权重，没有首次进入窗口时的剩余时间状态。
- 反例：100 ms帧、实际加载的 Roll（序列1.5 s、播放1.3、BlendOut0.3）到elapsed0.9时源weight=1，实际0.93627644。通过真实ragdoll退出启动 GetUp（序列1.5 s、RateScale1.2、BlendOut0.3），elapsed1.0时源weight=1，实际0.92592555。两个实际slot断言FAIL，进入窗口前weight1对照PASS。
- 影响：动作尾段pose/curve/notify权重与源错位，后续混出还使用不同预算。原有测试验证曲线形状或中断fade，没有验证源 UpdateWeight/Advance/首次自然Stop的顺序。Mantle使用自定义TriggerTime=0的末点停止，不按本项默认自然窗口直接推断缺陷。
- BF139.04追加：source `AlsAnimationInstance.cpp:1780` 的Transition使用TriggerTime=0，`AnimMontage.cpp:2626–2636` 仍在Advance首次触及末点才Stop，当前weight保持1。目标 `stop_link.rs:252–279` 当帧直接用elapsed越界量计算混出；实际StandingDynamic的duration1.533333/rate1.5，在末点前20 ms推进100 ms后weight=0.64799964、stop_elapsed=0.08000004，源该帧应1/0，实际角色消费者FAIL。Turn明确使用惯性混出，不把不同源合同算作此项缺陷。

### P2-10 — 初始非等比缩放退回未缩放的布娃娃物理配置

- Source：UE `SkeletalMeshComponentPhysics.cpp:1000–1084` 从含组件世界缩放的 BoneTransform 初始化刚体；`ChaosInterfaceUtils.cpp:139,148,176` 按形状调用 `GetFinalScaled`，`BodySetup.cpp:2006` 的球半径乘绝对最小轴。初始化与 `UpdateRigidBodyScaling:3517` 对运行中非等比变更的拒绝必须区分，本项只证明初始创建。
- Target：`src/avian_ext/ragdoll_physics_profile.rs:1055–1064` 的 `ragdoll_positive_uniform_scale` 对任何非等比输入返回1；实际profile的形状、质量、关节frame统一按该值配置。
- 反例：完整加载角色以 `(2,2,2)` 初始缩放时applied=2、root球半径0.2 m、pelvis质量37.70501 kg；以 `(2,3,4)` 创建时applied=1、球半径0.1 m、pelvis质量7.9265018 kg。源同一球的绝对最小轴仍2，应为0.2 m；等比对照PASS，非等比FAIL。root球作为尺寸探针本身禁碰，受影响的动态pelvis同样退回未缩放质量/形状，不能把禁碰球单独冒充碰撞症状。
- 影响：初始非等比角色的渲染姿态与物理体尺寸、质量及约束frame不一致。旧测试只覆盖正等比缩放及detach后等比变化，没有覆盖初始非等比创建。

### P2-11 — PA 的正数睡眠阈值与计数丢失

- Source：fresh native PA物理材质为线速度1 cm/s、角速度0.05、SleepCounterThreshold=4、body multiplier=1；UE `Chaos/Island/IslandManager.cpp:242–249` 读取全部字段，按速度及计数决定睡眠；孤立刚体另有阈值缩放，不会把0.03 m/s变为可睡眠速度。
- Target：`tools/import/gen_ragdoll.py:64–75` 只生成 `sleeping_disabled`；`src/avian_ext/ragdoll_physics_profile.rs:1192` 只切换禁睡标记。正数配置落回Avian默认 `SleepThreshold{linear:0.15,angular:0.15}` 和TimeToSleep=0.5 s，原阈值/倍率/计数无消费者。
- 反例：使用实际生产profile插件、原PA的pelvis配置，去除外力/阻尼与约束以隔离睡眠判定。0.2 m/s对照80帧后仍运动、位移0.26333317 m；0.03 m/s在位移0.015000002 m处睡眠，实际阈值0.15，源应保持运动，原生断言FAIL。
- 影响：布娃娃仍在缓慢移动/旋转时提前冻结，静止后的睡眠延迟也不等价。BF135只验证零阈值禁睡，没有验证有效正阈值及计数；本轮不修改其既有已完成边界。

### P2-12 — Ragdoll 退出使用上一帧地面结果

- Source：`ALS/Private/AlsCharacter_Actions.cpp:1082–1187` 的StopRagdolling当场调用RagdollTraceGround，再决定Walking/GetUp或Falling/继承骨盆速度。
- Target：`src/als/character/actions/ragdoll_link.rs:604–655` 从缓存 `AlsRagdollGroundFollow` 读取grounded和velocity；`:788–817` 再使用同一旧target，退出后不重新查询本帧支撑。
- 反例：完整角色在平地进入ragdoll并稳定90帧，确认grounded=true；删除唯一支撑体后立即请求退出。实际最终LocomotionState=Air而Action=GettingUp，源当前查询应直接Air且无GetUp，实际消费者断言FAIL。
- 影响：地面被移除/移走同帧退出时在空中播放起身并阻塞输入，且可能用过时位置/速度恢复。旧退出Gate只比较固定grounded布尔分支，没有验证查询到退出之间的场景生命周期变化。

## 未定级观察

BF138 的固定 `0.046875` 秒 UphillJump 记录存在 pre-rig 腿平面异常，但旧报告缺少当帧完整图输入与混合源骨骼。它是待核实症状，不能据此认定源/目标差异或复开相邻图。

## 撤销记录

P2-01 Sprint 于2026-10-11 12:01 +08:00撤销。工程插件链接实际指向原始仓库的 Build 副本；原始 `ALSExtras/Private/AlsCharacterExample.cpp:75–76,127` 绑定 Triggered/Canceled 并按输入bool选择 Sprinting/Running，与目标一致。Build的Started/toggle属于本地调试修改，不能作为原始 `b754d6f0` 的移植合同。原错误断言与阶段报告保留作审计失误证据，不再计入已确认差异；编号不复用。146个原始C++/header完整比较，仅此cpp/header存在语义改动，AnimationInstance仅换行不同；已使用的6个uasset与原始资产字节一致，其他已登记差异的源文件不受影响。后续C++一律从原始插件根读取。

## 审计验证与限制

本轮不修改生产代码、转换规则或原始资产；无需以全构建替代逐项 source 证据。只有与具体差异有关的最小验证会执行。没有新增设计、抽象或运行时分支，设计消融暂不适用。

BF139.01 使用当前生产库链接独立 Rust 反例，保存于本地 `target/bf139/phase1/`（probe源码、stdout/stderr、watchdog JSON）。9个测试为7个预期语义失败和2个通过对照；期望失败归并为6项差异，不能解读为移植验收通过。原生库未修改；最初缺少Windows native link-search和一个Position字段访问错误均已纠正，不计入语义证据。reader watchdog `-SelfTest` 的正常完成/无进展终止/子进程清理/报告用例全部通过。

本地 ALS 插件版本身份：`b754d6f0f2bb03741d301f8fb88077ebfe561e17`；本轮源 C++ SHA-256：

| 文件 | SHA-256 |
|---|---|
| AlsCharacterExample.cpp（原始） | `ba558540dd414d57bd61bb37c57cf3db9d6a1b6a15088463616588845fc1afa4` |
| AlsCharacter.cpp | `779192e94d0a1ac13ccf95cfa9db540cc4f6230a2dc3ab9113848f8d983ace84` |
| AlsCharacterMovementComponent.cpp | `e37d349bc41a5de208cbdd1799c32741cf84aa5bddfe5580906c50f2a6360dda` |
| UE Actor.h | `2ee1effb7ab4ac4cf65cf6caa90a27cea45361feefaae0cb673f7265e00bc5c9` |
| UE CharacterMovementComponent.cpp | `fcf827d7c5adaa73a4f272a716a53b03f66fc7352f3eacc1be4d51cc9b3e15a8` |
| AlsCharacter_Actions.cpp | `3dbd91f26154fd99297a7c7bb293993d938a86b8937d7071760cabc8517aa317` |
| AlsRootMotionSource_Mantling.cpp | `624964d860117f3794d249a2b8d25e5d0c610b7dd852751e052e60bb19b7d55e` |

BF139.02 的实际角色反例在资产就绪后执行；根图异步加载期间的临时 OutputMissing 不作为本次动作缺陷。fresh native导出 Roll/GetUp Front/Mantle High 已完成，生成数据的播放率/混入/混出/Trigger字段已核对。累计14测试为12预期语义FAIL和2对照PASS；本地源码与原始报告存于 `target/bf139/phase2/`。源 `PhysCustom` 的 MoveUpdatedComponent 明确不扫掠，不能把攀爬中不扫掠定为移植缺陷。

BF139.03 fresh原生PA导出37.93秒通过；使用当前转换函数在本地重建完整physics profile，与安装文件对象逐字段完全一致（21body/20constraint/49pair），故以上为转换字段遗漏或消费者差异，不是旧产物未更新。Ragdoll查询使用带shape mode过滤的公开wrapper，原父空间退出、初始9次限速、驱动当前pose与实际碰撞仲裁已有实现，不重复登记。非默认且转换器明确拒绝的angular projection/capped angular drive等不冒充当前PA缺陷。

更正Sprint源期望后，当前完整最小反例集17测试10.05秒完成：3PASS/14预期语义FAIL，归并11项有效P2，均未修复；源码/日志/JSON保留于本地 `target/bf139/phase3/`，watchdog未超时。

BF139.04的fresh AIS_Als_Default导出41.86秒通过、原始与安装副本资产字节相同；设置及源AnimationInstance函数闭包核对完毕。当前19个最小测试11.82秒完成：3PASS/16预期语义FAIL，两个新增失败归并P2-05/09，11项总数不变；证据存本地 `target/bf139/phase4/`。没有把仅用于诊断的旧DynamicTransition尺度和无当前图consumer的view_pitch_norm计作P2。
