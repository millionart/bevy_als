# ALS 移植差异审计 — 2026-10-11

审计目标：记录当前实现中经本地 ALS-Refactored / UE source 与实际 target 消费链确认的全部 P2 及以上差异。目标基线：`bb3dccf3066a217b750670918e30f248ef2c5b98`。8批审计于2026-10-11 15:00 +08:00完成，确认18项P2、未确认P0/P1；均已实时登记至CHECKLIST。用户随后授权修复，当前状态以CHECKLIST及各条修复记录为准。P2-01已撤销，不计入18项。

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
| 图/FSM/同步/曲线、资产转换与 Rig | 已审计，新增P2-13/14 | 15root/23AnimBP/236子图/374产物完整重转与source闭包；Node运行时、127动画、Rig/骨骼/坐标与当帧pose消费者 |
| 通知、脚步/贴花、Overlay 物件行为 | 已审计，扩展P2-05/08 | 137动画/蒙太奇、177通知、60标记重建一致；Notify队列、脚步与贴花参数/附着生命周期、Overlay绑定/清除/恢复；静态mesh/socket/material留资产批 |
| Camera、CameraShake | 已审计，新增P2-15 | 本地组件/Shake/调度与10个原生数据来源，实际view/碰撞/时钟/Camera消费者 |
| Playground、材质/Overlay静态数据、HUD/UI、PlayerStart | 已审计，新增P2-16–19 | 34 Overlay/34环境/5 EngineSky依赖；20环境mesh碰撞模式/15凸包、150材质参数/4纹理、22文字、HUD/菜单、3运动物件、PlayerStart与实际装配消费者 |

## 已确认 P2 及以上

### P2-02 — DesiredRotationMode=Aiming 被降为 ViewDirection

修复完成（2026-10-11 15:58 +08:00）：保留持久Aiming并与Aim按钮独立合并，删除错误排除和单用途默认helper，沿用既有组件；无组件时的既有初始化边界保留。fresh Character/CS/Head及原始C++、34项模式/input/gait/Head驱动回归、两个独立消融与同target恢复通过；当前345规格/15root完整闭包、510图编辑器和资产边界通过。真实startup→Head 7窗口功能验收通过，专门覆盖未按按钮的desired Aiming，图/姿态/故障为0；性能仍按既有延期边界保留。证据在本地 `target/bf139/fix02/` 与 `target/animation_visual/bf139_p202/`。以下保留原审计反例。

- Source：`ALS/Private/AlsCharacter.cpp:675` 的 `RefreshRotationMode` 用 `bDesiredAiming || DesiredRotationMode == Aiming`；源公开的 `SetDesiredRotationMode` 允许直接选择 Aiming，无需持续按 Aim。
- Target：`src/als/state/aim.rs:55` 只以 `aim_held` 计算意图，`:57` 把存储的 Aiming 改为默认自由模式。`AlsLastFreeRotationMode` 确实承担 desired rotation：`src/als_camera/mod.rs:427` 用它实现源相机的 desired-mode 分支，调试 HUD 也将其展示为 DesiredRotationMode。
- 反例：Grounded/Standing、MaxAllowedGait=Running、desired=Aiming、`aim_held=false`。实际为 ViewDirection，源为 Aiming；当前生产 `AlsAimPlugin` 断言失败。
- 影响：程序设置/初始配置的瞄准模式无法保留，身体朝向及相机模式分歧。`last_free_mode_never_stores_aiming` 测试固化了比源更窄的状态集合。
- 排除：普通 `AlsStateRequest.rotation_mode` 被刷新覆盖的现象不单独计数；source 的实际模式 setter 同样受 desired-mode 刷新支配，不能仅凭这一现象定缺陷。

### P2-03 — 普通 CMC 运动没有消费角色 CustomTimeDilation

修复完成（2026-10-11 17:35 +08:00）：Walking/Falling事务消费owner delta，物理桥与基座保留世界时钟；完成速度从角色位移恢复，并先扣实际物理帧的基座携带量。沿用既有倍率组件和运行数据，四次消融均触发对应失败，源码字节恢复后定向回归及同启动器生产库的24组合矩阵、跳跃/落地/倍率恢复检查通过。当前345规格/15root闭包及510图编辑器通过，真实startup/clock/grounded/moving_base功能Gate全部通过；clock实际覆盖0/.5/1/2倍率，Roll/Mantle移交通过，图/故障/时序异常均0。超时复盘和拆分结果见CHECKLIST，性能延期与截图关闭边界保留。证据位于本地 `target/bf139/fix03/` 和 `target/animation_visual/bf139_p203/`。以下保留原审计反例。

- Source：UE `Runtime/Engine/Classes/GameFramework/Actor.h:4877` 的 `FActorComponentTickFunction::ExecuteTickHelper` 将组件 Tick Delta 乘 owner `CustomTimeDilation`；CMC 的 Walking/Falling 都消费该 Delta。
- Target：`src/als/character/movement/dynamics.rs:441` 和 `src/als/character/movement/motion.rs:3447` 直接使用全局 `Time`；后者将 `time.delta_secs()` 传入 Falling 事务。`AlsCameraCustomTimeDilation` 已被角色旋转、动画和已修复动作使用，但普通移动未接入。
- 反例：Air、水平速度 3.75 m/s、50 ms 帧、角色倍率 0.5。源水平位移应为 0.09375 m，实际为 0.1875 m；竖向速度也按完整 50 ms 的重力推进。当前真实 ECS/物理消费者断言失败，倍率 1 的无落地运动对照通过。
- 影响：角色慢放/加速时，普通位移与自身动画、旋转和动作时钟脱节。此前 owner-clock Gate 覆盖 GetUp/Roll/Mantle，未覆盖普通 CMC；本项不复开这些已完成动作。

### P2-04 — Falling 首个子步落地后丢失剩余 Walking 时间

修复完成（2026-10-11 21:39 +08:00）：碰撞子步退款后以共用迭代预算进入Walking，复用既有地面算法并保留同帧模式转换事件。落地回调、自动Roll和Ragdoll优先级均按源执行；已有CMC结果阻止新蒙太奇重复消费本帧移动。首次Ragdoll激活只对齐当前骨骼world pose，None模式清零actor速度及未消费motor，保留骨骼物理速度，修复退出时13.24 m/s反向弹射。40组落地、24组角色时钟、同帧回调、真实默认资产与开关控制通过；14个有效消融边界和合并实验、14项相关/4项接管/7项恢复控制、正常生产库矩阵及345规格/15root闭包通过。真实startup、grounded、0.2秒hitch moving_base、最终Ragdoll功能验收通过，最终PA位置/旋转误差0，恢复与输入解除全部覆盖，图/故障/时序异常均0。原失败报告、构造默认与资产有效值的纠正、超时拆分记录保留；性能延期、截图关闭及旧固定帧腿平面观察保持。证据在本地 `target/bf139/fix04/` 及 `target/animation_visual/bf139_p204*`。以下保留原审计反例。

- Source：UE `Runtime/Engine/Private/Components/CharacterMovementComponent.cpp:5050` 在合法落点将子步剩余时间加入 `remainingTime`；`:6373` 的 `ProcessLanded` 最终调用 `StartNewPhysics(remainingTime, Iterations)`，继续当前帧的地面运动。
- Target：`src/als/character/movement/motion.rs:2697` 在发现可走地面后直接 `break`，返回接口不携带剩余时间，也没有切入 Walking。之后 `locomotion_state_sync_system` 只更新模式/速度。
- 反例：胶囊中心高 1 m、下落速度 3 m/s、水平速度与前向输入对应 3.75 m/s、200 ms 帧、50 ms CMC 子步、平地。源在落地后继续行走，水平位移约 0.75 m；实际仅 0.10516852 m，帧末已 Grounded、水平速度仍 3.75 m/s。相同输入在高空无落地时为 0.75 m，正对照通过。下落速度低于源 Roll/Ragdoll 阈值，排除了落地动作切换。
- 影响：卡顿帧落地明显少走距离；空中末速度、中点、墙面尾段和“最终进入 Grounded”测试均不能检出跨模式后的时间丢失。

### P2-05 — 普通角色/动画输入把局部旋转当世界旋转

修复完成（2026-10-11 23:32 +08:00）：七个actor旋转consumer读取当帧world，普通移动保留world Pitch/Roll并在局部写回处乘父级旋转逆；provider及Air预测同时读取当前mesh world scale，补闭父级缩放的缓存缺口。8组消融/13个失败边界及45/27/4项恢复控制通过，最终正常生产库的父级Grounded/Air物理管线、原反例/对照、345规格/15root闭包通过。真实startup、带父级Head 7窗口、Grounded功能Gate均通过，图/故障/时序异常均0；Head守卫的affine四元数误报由独立反例修复，保留错输入与缺父级拒绝。fresh坐标基更正原Head夹具期望：target父级+90°对应source yaw−90°，旧局部0°仍为缺陷；原错误符号与失败报告保留。未修改图、转换器、原始资产或新增生产状态，性能与截图延期保持；证据位于本地 `target/bf139/fix05/` 和 `target/animation_visual/bf139_p205_guard/`。以下保留原审计反例。

- Source：`ALS/Private/AlsCharacter.cpp:1680` 等旋转函数使用 `GetActorRotation`/`SetActorRotation`；`ALS/Private/AlsAnimationInstance.cpp:730` 从 actor world transform 读取旋转，`:826` 将世界速度反旋到角色局部空间。
- Target：`src/als/character/movement/motion.rs:415` 从局部 `Transform.rotation` 得到当前 yaw，`:577–579` 又把这个旋转同时写给局部 Transform 和世界 `avian3d::Rotation`。`src/als/character/animation_instances/actual_motion.rs:90`、`ab_als_provider.rs:319` 和 `locomotion_layer.rs:209` 同样把局部旋转作为世界姿态输入。
- 反例一：父实体 yaw=90°、角色 local identity、沿角色世界前方以 2 m/s 运动。局部速度应为 `(forward=2,right=0)`，实际 `(0,-2)`；无父实体、相同世界方向的对照通过。
- 反例二：真实移动插件中给静止角色挂平移+90°父实体，并保持其原世界姿态。更新后渲染 world yaw 约 0°，物理 Rotation 为 −90°。位置本次保持一致，因此不据此声称位置一定出错。
- 影响：挂接/分组角色的朝向、方向动画、Lean 和物理旋转不同步。此前父空间 Gate 只覆盖 Roll/Ragdoll/Camera 等消费者，没有覆盖普通 locomotion/provider。
- BF139.04追加：`head_layer.rs:72–118,166` 也读取局部Transform推导world yaw/pitch；完整角色在物理后固定local identity、父级yaw90时，实际Head/Spine共享输入 `locomotion_state.rotation_yaw=0`，源应90，第三个消费者反例FAIL。该父空间根因继续归并本项。
- BF139.06追加：`stop_link.rs:398–421,869–903` 与 `transition_notify.rs:334–350` 的QuickStop调用链也传局部body。无输入、90°父级、目标yaw0时，源world actor合同选择右侧/2.375，实际左侧/1.75；生产选择函数断言FAIL、无父级对照PASS。

### P2-06 — 普通台阶跨越忽略组件 CanCharacterStepUp 许可

修复完成（2026-10-12 00:32:54 +08:00）：普通StepUp在初始命中与更高落点读取已有许可，child显式设置优先、所属刚体兜底，并覆盖落地后的walking剩余预算。fresh完整CMC闭包纠正了原20cm最终位置oracle：源FindFloor/AdjustFloorHeight独立刷新地面，不能由最终登上直接证明StepUp未拒绝；原始报告保留，该断言撤回。修正40cm反例先红，16组许可/所有权/帧预算矩阵、独立正面/落点四组合、3个判定消融、精确字节同target恢复与Mantle/partial-step/运动时钟/落地回归通过；额外高度输出消融后信号不变，已删除。当前345规格/15root完整闭包及声明式边界通过；真实startup、三座原楼梯上/下六路线功能通过，图/fault/temporal/primary_failure均无。证据位于本地 `target/bf139/fix06/` 与 `target/animation_visual/bf139_p206/`，性能延期保持。以下保留原审计记录。

- Source：UE `Runtime/Engine/Private/Components/CharacterMovementComponent.cpp:7525` 的 `CanStepUp` 检查组件 `CanCharacterStepUp` 与 actor `CanBeBaseForCharacter`；`StepUp` 必须通过该检查。ALS Mantle 也使用同一个原生组件许可。
- Target：`src/als/character/movement/motion.rs:3380` 的低台阶事务只检查碰撞/高度/空间，没有读取已经由 `AlsMantleTargetPermission.allows_step_up` 表达的组件许可；该许可目前只被 `mantle.rs:569` 消费。
- 反例：20 cm 静态台阶，角色持续前进 60 个 1/60 s 帧，分别设置许可 true/false。两次均跨过台阶并到达 z=−3.4291668；源 false 应在台阶正面被阻挡。true 正对照通过，false 断言失败。
- 影响：明确禁止角色跨越的碰撞体仍能通过普通走路登上。现有许可测试只触发 Mantle，未经过普通 StepUp。

### P2-07 — Mantle 的 root 速度混用了 world 与 local 位置

修复完成（2026-10-12 01:43:03 +08:00）：Mantle root速度使用当前actor世界位置，root与当前移动基座的世界目标经parent逆affine/逆quat写入local，物理Position/Rotation保持world。fresh原始RootMotionSource/Character_Actions/Walking移交及High/Low原生蒙太奇闭包、24组父级/owner/完成组合、移动基座和退出惯性、六个独立消融与精确字节同target恢复14控制通过。正常构建、5运动矩阵和原完整角色反例、345规格/15root闭包/声明式边界通过；真实startup与带活动父级Mantle的原Base功能通过，53项全覆盖、92帧world/物理一致，图/fault/temporal/primary_failure均无。速度断言使用实际消费的world位置复算，原阈值保留；没有新增生产状态或更改时钟/资产/root算法。证据位于本地 `target/bf139/fix07/` 与 `target/animation_visual/bf139_p207/`，性能延期保持。以下保留原审计记录。

- Source：`ALS/Private/RootMotionSources/AlsRootMotionSource_Mantling.cpp::PrepareRootMotion` 用 `(TargetActorTransform.Location - Character.GetActorLocation()) / DeltaTime`。
- Target：`src/als/root_motion_sources/mantle_drive.rs:727` 减去局部 `Transform.translation`；`:729` 与移动目标回写 `:795` 也直接将世界变换写入局部字段。
- 反例：完整加载角色，保持相同世界起点与 High Mantle 目标，仅增加平移 `(5,0,0)` 的父实体。普通角色横向速度约 `−4.09e−8` m/s，有父级后为 `299.99997` m/s，断言 FAIL。动作、root clip 和当前源形状均相同。
- 影响：挂接角色产生错误 root 速度，并影响后续运动/物理移交。已有 Mantle owner-clock 测试使用无父实体的 Transform，没有约束世界空间速度。

### P2-08 — Roll/Mantle 重新处理原始输入，丢失源输入有效性

修复完成（2026-10-12 02:58 +08:00）：Roll/Mantle和两个QuickStop入口直接消费已有has_input及world yaw；Mantle速度分支也使用已消费的速度有效性/yaw，删除重复raw归一化和阈值。fresh原始Character/Actions/AnimationInstance、UE GetSafeNormal与CS/AIS闭包通过，原3反例及矛盾raw/消费快照、真实完整帧、生成FSM正例、阈值两侧/保留yaw、父空间/时钟/生命周期等49项控制通过。五个独立consumer消融均语义失败，源码精确恢复后同target重编49项重新通过；正式入口构建及受影响例程、原3反例/5运动矩阵、345规格/15root完整闭包、510编辑器与声明式边界通过。真实startup和原Base功能验收通过，53项覆盖齐全，graph/fault/temporal为空、primary_failure=null。没有新增生产状态、适配层或阈值，输入生产者未改；性能和截图延期保持。证据位于本地 `target/bf139/fix08/` 与 `target/animation_visual/bf139_p208/`。以下保留原审计记录。

落地消费者有限复开并补闭（2026-10-12 03:58:05 +08:00）：P209扩大回归后发现落地Roll仍读取Walking后的当前速度；fresh Character.cpp:522–525证实源读取已消费的bHasVelocity/VelocityYawAngle，独立链接原P208正式库复现源−90°、目标0.14544414 rad，证明旧Gate漏掉该消费者。现落地入口复用已有速度快照和1 cm/s判定，移除live速度及重复向量阈值；缓存yaw/live速度矛盾、阈值两侧、原完整角色反例及补齐前置条件的三个旧夹具通过。两项消融均语义FAIL，精确字节恢复同target重编55项通过；正式库四原反例/五运动矩阵、345规格/完整15root闭包及声明式边界通过。真实startup/Base功能通过，Base53项覆盖齐全，graph/fault/temporal为空；没有修改生产状态、CMC预算、时钟或图。完整重转的374图产物零diff，未复开其他已完成模块。证据位于本地 `target/bf139/fix08_landing/` 与 `target/animation_visual/bf139_p208_landing/`，原Gate与脚本guard误判记录保留。

- Source：`AlsCharacter_Actions.cpp:34` 和 `:176` 读取已由 Character 消费、归一化的 `LocomotionState.bHasInput/InputYawAngle`；有效性阈值为平方 `1e−8`。
- Target：`roll.rs:57`、`roll_drive.rs:1931` 又使用平方 `1e−6`。Mantle 则由 `motion.rs` 把原始输入先变成单位方向，再交 `mantle.rs:469` 判断。
- 反例：实际角色输入 `(0.0002,0)` 已 `has_input=true`，源应向右 Roll；实际目标 yaw=0，源 −90°。输入 `(0.01,0)` 正对照正确。另以 `(0.00001,0)` 验证已 `has_input=false` 且无水平速度，实际 Mantle trace 为 `(0.76604444,0,−0.6427875)`，源应 `(0,0,−1)`，相差50°。
- 影响：模拟量边界的动作方向和攀爬候选选择错误。普通四向/无输入测试绕开了源 safe-normal 边界和真实已消费状态。
- BF139.06追加：source `PlayQuickStopAnimation:1590` 也读已消费bHasInput/InputYaw；目标 `stop_link.rs:407` 再以f32::EPSILON过滤原始输入。实际movement确认0.0002右输入has_input=true/input_yaw=−90；stored target yaw0时，当前函数错选左侧/1.75，源应右侧/2.375，0.01对照PASS。fresh AIS确认QuickStop播放率范围1.75–3，继续归并此根因。

### P2-09 — Roll/GetUp/Transition 的自然混出时序不同

修复完成（2026-10-12 04:35:10 +08:00）：三类动作均按源先更新权重、再Advance并首次Stop，触发帧保留当前weight，后续使用实际剩余预算；Transition自定义TriggerTime=0采用配置预算且不消费当帧越界量。Roll用一个可空起点替换无用cooldown，GetUp/Transition复用已有停止状态；真实slot/曲线/通知读取同一weight。fresh GetUp原生action End marker为1.166567，生成器不再误用Duration=1.166667，仅两处目标设置改变；全部15root/39来源完整重转，374图产物零新增/删除/修改。六项消融均语义FAIL，精确字节恢复同target重编83项相关控制通过，正式库原3完整角色反例/五运动矩阵、345规格/15root合同、510编辑器与声明式边界通过。真实startup/Base/GetUp/TransitionNotify全部功能通过，运动场景53/13/3项覆盖齐全，graph/fault/temporal为空、primary_failure=null。P208遗漏的落地方向已独立补闭并提交；保留初次夹具oracle、脚本guard和超时复盘记录，没有阈值补偿或相邻Turn修改。证据位于本地 `target/bf139/fix09/` 与 `target/animation_visual/bf139_p209/`；完整性能和截图延期保持。以下保留原审计记录。

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

- 修复闭环（2026-10-12 05:50 +08:00）：初始world scale保留三轴，球取最小轴、盒逐轴、胶囊按横向最大轴与轴向总高重建；中心和两侧joint frame按各轴缩放。质量/CoM/惯性从最终形状及现有mass exponent重算，nudge与inertia scale由公开离线生成器输出目标字段；仅完全等比时使用原标量公式。独立source oracle验证pelvis约122.2897 kg、双shape spine_03及旋转hand_l，禁碰root球仅作尺寸探针。五矩阵含(2,2.00005,2)、非零modifier、运行中拒绝与新root清理通过；九项消融均语义FAIL，精确字节恢复后55项回归PASS。正式库原完整角色反例/五矩阵、345规格/15root静态闭包和startup/Ragdoll/GetUp功能Gate通过；full15 AnimBP/39source/374产物重转零diff。原始UE资产与既有Ragdoll/bone map未改，profile仅新增42个目标metadata字段；性能与截图延期保持。

### P2-11 — PA 的正数睡眠阈值与计数丢失

修复完成（2026-10-12 07:38:42 +08:00）：public PhysicsAsset生成器从fresh PA/SKM与有效物理材质生成21组目标阈值/计数，原始导出保持；完整15root/39源/374图产物重转零diff。运行时单一身体组件消费源严格大于计数、孤立127饱和/计数保留、连接组全员条件与最大计数，并通过公开Avian接口睡眠/拆分；连接指标从质心delta换算为身体原点delta，再按源0.3平滑及四元数导数处理。48组阈值/计数/帧长/子步矩阵、五倍率、刷新/唤醒/断开/native成员和偏心旋转反例、12有效消融及字节恢复后63回归通过；旧惯量夹具显式禁睡以保留原断言，生产惯量未改。正式target_game构建及原0.03 m/s反例、五运动矩阵、345规格/完整闭包/声明式/510编辑器通过；原Playground startup诊断、Ragdoll11项、GetUp13项功能验收通过，graph/fault/temporal均空。首次startup混入Ragdoll/退出动作的失败原报告与更改诊断路径后的无动作通过均保留，不冒称其原因已由输入日志回溯证明；性能和截图延期。证据位于本地 `target/bf139/fix11/` 与 `target/animation_visual/bf139_p211/`。以下保留原审计反例。

- Source：fresh native PA物理材质为线速度1 cm/s、角速度0.05、SleepCounterThreshold=4、body multiplier=1；UE `Chaos/Island/IslandManager.cpp:242–249` 读取全部字段，按速度及计数决定睡眠；孤立刚体另有阈值缩放，不会把0.03 m/s变为可睡眠速度。
- Target：`tools/import/gen_ragdoll.py:64–75` 只生成 `sleeping_disabled`；`src/avian_ext/ragdoll_physics_profile.rs:1192` 只切换禁睡标记。正数配置落回Avian默认 `SleepThreshold{linear:0.15,angular:0.15}` 和TimeToSleep=0.5 s，原阈值/倍率/计数无消费者。
- 反例：使用实际生产profile插件、原PA的pelvis配置，去除外力/阻尼与约束以隔离睡眠判定。0.2 m/s对照80帧后仍运动、位移0.26333317 m；0.03 m/s在位移0.015000002 m处睡眠，实际阈值0.15，源应保持运动，原生断言FAIL。
- 影响：布娃娃仍在缓慢移动/旋转时提前冻结，静止后的睡眠延迟也不等价。BF135只验证零阈值禁睡，没有验证有效正阈值及计数；本轮不修改其既有已完成边界。

### P2-12 — Ragdoll 退出使用上一帧地面结果

修复完成（2026-10-12 08:28:25 +08:00）：退出请求在状态转换前调用公开Avian移动包围盒刷新，再按源当前sphere sweep重算grounded/target；保留源最后记录的位置/速度与零目标fallback、原碰撞响应和自身排除，退出路由与actor realign消费同一结果。完整角色支撑删除/移走/移入/新增与自身可查询边界、8种位置/速度/过滤矩阵、3个既有路由状态通过；去掉重查、AABB刷新、自身排除分别稳定复现原错误起身、移入漏判和自身支撑误判。移除新helper后结果相同，最终无新抽象/组件/状态；源码精确恢复、同target重编37回归、正式库原完整角色反例/五运动矩阵、345规格/15root静态闭包/声明式/510编辑器通过，fresh15root/39源/374产物零diff。原Playground startup、Ragdoll11项、GetUp13项功能验收通过，graph/fault/temporal均空，性能和截图延期。证据位于本地 `target/bf139/fix12/` 与 `target/animation_visual/bf139_p212/`。以下保留原审计反例。

- Source：`ALS/Private/AlsCharacter_Actions.cpp:1082–1187` 的StopRagdolling当场调用RagdollTraceGround，再决定Walking/GetUp或Falling/继承骨盆速度。
- Target：`src/als/character/actions/ragdoll_link.rs:604–655` 从缓存 `AlsRagdollGroundFollow` 读取grounded和velocity；`:788–817` 再使用同一旧target，退出后不重新查询本帧支撑。
- 反例：完整角色在平地进入ragdoll并稳定90帧，确认grounded=true；删除唯一支撑体后立即请求退出。实际最终LocomotionState=Air而Action=GettingUp，源当前查询应直接Air且无GetUp，实际消费者断言FAIL。
- 影响：地面被移除/移走同帧退出时在空中播放起身并阻塞输入，且可能用过时位置/速度恢复。旧退出Gate只比较固定grounded布尔分支，没有验证查询到退出之间的场景生命周期变化。

### P2-13 — Slot 的 SourceWeight 没有进入底层状态机更新上下文

修复完成（2026-10-12 09:40:07 +08:00）：SourceSlot统一计算更新权重，普通Slot传递剩余非加法权重，加法保留1；SourceWeight严格大于1e-5才相关，always-update按源最小2e-5更新。直接Source求值复用既有machine-weight scope，现有缓存分支收集器也读取同一权重，保留SaveCachedPose最高请求语义及Getter完成缓冲，不改变Pose/Curve混合。14组parent/Slot模式/边界/嵌套/共享缓存与真实条件选态、五种语义消融、字节恢复同target95回归、正式库两个原反例/五运动矩阵、345规格/15root静态闭包/声明式/510编辑器通过，fresh15root/39源/374产物零diff。原Playground startup、grounded（含Roll）16项、GetUp13项功能通过，graph/fault/temporal均空，性能和截图延期。夹具按当前核心query自动推进帧的事实纠正重复推进，先建立完成权重再验证条件，未修改原Getter。证据位于本地 `target/bf139/fix13/` 与 `target/animation_visual/bf139_p213/`。以下保留原审计反例。

- Source：UE `AnimGraphRuntime/Private/AnimNodes/AnimNode_Slot.cpp:86–92` 用SourceWeight创建FractionalWeight更新源pose；`Engine/Private/Animation/AnimNode_StateMachine.cpp:382` 记录Context.GetFinalBlendWeight，`AnimInstanceProxy.cpp:2487` 的MachineWeight getter读该记录。
- Target：`src/animation_graph_ext/source_slot.rs:130` 直接读取Source，未使用已有fractional machine weight scope；最终pose/context曲线乘权重不能补回更新阶段记录。
- 反例：实际生产SourceSlotNode与单状态InterruptibleFsmNode组成最小图，连续3帧。slot weight=0时machine=1对照PASS；slot weight=0.75、layer weight=1时machine仍1，源应0.25，原生断言FAIL。
- 真实consumer：当次原生AB_Als_Standing的Movement Details States两个转换有 `Machine Weight (Standing States)` getter；安装T8/T9条件图比较该值与1，因而这不是仅遥测数值不同。会改变蒙太奇混合期间的状态选择与后续姿态。
- 漏检：已有Slot测试验证pose/curve权重和是否更新Source；machine scope测试覆盖普通/加法blend，缺少Slot包裹真实FSM的组合。当前只登记，不修改通用Slot、转换器或已生成图。
- 完整角色追加：开始Roll首帧PostLocomotion slot=0.07407408，真实Locomotion States machine权重1，源应0.9259259，当前安装图消费者FAIL。fresh根图的PostLocomotion直接包裹AB_Als_Locomotion；UE LinkedAnimGraph `:88–106` 仅WithOtherProxy，保留权重。初次寻找未活跃Grounded子machine的夹具前置失败不计入语义证据。

### P2-14 — 非同步 SequencePlayer 继承全局root查询时间

- Source：UE `Actor.h:4877` 的组件tick乘owner CustomTimeDilation；`Engine/Private/Animation/AnimNode_SequencePlayer.cpp:100–110` 从UpdateContext获取Delta。ALS `RefreshRagdollingOnGameThread` 仅以骨盆速度计算Flail播放率，没有改用全局时间的例外。
- Target：`src/animation_graph_ext/post_physics_animation.rs:190–202` 向run_animation_player传全局Time；`source_sequence_player.rs:350–362,488` 在无synchronization时用context_time_update。`animation.delta_seconds` 只供部分FSM/sync/辅助链消费，没有统一控制root查询。
- 反例：完整角色进入ragdoll12帧后给实际body共同5 m/s速度，owner CustomTimeDilation=0。确认player的animation.delta_seconds=0；活跃 `A_Als_Flail` 的clock.delta_seconds=0.016666668，position从0.01987938到0.028212715，按0.5播放率仍推进0.008333秒，原生断言FAIL。
- 影响：冻结/减速角色的flail pose与关节驱动目标继续按世界帧更新，和角色的其他动画时钟不同。旧时钟测试覆盖FSM、同步玩家和动作montage，没有覆盖root下无同步的序列；Rifle的初始候选未满足活跃player前置条件，未当作缺陷证据。后续修复应找全同一消费者，不只特判Flail。

修复验收（2026-10-12 10:29:57 +08:00）：root查询改用现有animation.delta_seconds，普通播放器保留全局时间。root/linked八组受控矩阵与实际Ragdoll六倍率、pose/通知/暂停验证通过；三个必要分支消融失败、精确字节恢复同target 35项回归通过。三相关资产fresh source、39源身份/374完整产物、345规格/15root静态闭包及声明式通过。公共startup/Ragdoll/GetUp功能通过，截图/性能延期。旧反例保留；开始09:47、预估10:37、偏差−7分02秒。

### P2-15 — 世界暂停时零owner倍率相机仍更新

- Source：UE `Components/SkeletalMeshComponent.cpp:440` 明确 `PrimaryComponentTick.bTickEvenWhenPaused=false`；ALS Camera构造只修改start/tick group，未改变该标志。`TickTaskManager.cpp:1626` 暂停期间只调度允许暂停tick的组件，所以IgnoreTimeDilation的real-delta回退不应在全局暂停时被调用。
- Target：`src/als_camera/camera_component.rs:146–250` 在Update内不判断world pause；owner CustomTimeDilation=0使 `camera_delta_seconds_from_times` 的分母为0，回退Time<Real>并持续更新图、lag及view。
- 反例：完整角色真实Camera组件，开启第一人称混合后暂停Time<Virtual>，推进3个真实帧。owner=1时elapsed delta=0、FirstPersonOverride保持0.07407408，对照PASS；owner=0时elapsed delta=0.049999997、混合升到0.7407408、view移动1.9981213 m，原生断言FAIL。
- 影响：暂停界面/冻结角色的相机仍平移和切换视点。旧测试分别覆盖时间倍率和暂停，没有组合world pause与零owner倍率。修复应保留世界运行中IgnoreTimeDilation对零owner倍率的合法real-time行为；当前只记录。

修复验收（2026-10-12 11:03:15 +08:00）：fresh Character Camera CDO与engine scheduler确认暂停不tick；动画与rig各用一条Bevy原生not(paused)条件，保持原缓存view/有效性，并保留运行时owner0忽略倍率的real-time回退。六组合暂停/恢复、真实pose/curve/lag/view矩阵通过，两条件消融失败、两份源码字节恢复后38项camera回归通过。当前Camera8源subgraph/345规格/15root完整静态闭包与公共startup/Camera11项通过，截图/性能延期。开始10:31、预估11:16、偏差−12分45秒；旧反例保留。

### P2-16 — PlayerStart named lookup 使用大小写敏感字符串

- Source：UE `GameModeBase.cpp:1156–1160` 先构造FName，再与PlayerStartTag比较；FName身份忽略大小写。
- Target：`src/als_extras/core/player_start_types.rs:111` 和 `game_mode.rs:232–233` 对Option<&str>做精确比较，named selection与同帧reservation均不保留该身份语义。
- 反例：两个可用出生点DoorA和Other、random_ticket=1。请求DoorA正确选首个；请求doora却选Other，当前生产选择函数断言FAIL。
- 影响：URL/脚本传入的标签仅大小写不同时会把玩家送到错误出生点。旧用例只验证精确拼写；GameplayTag的历史FName修复未覆盖独立PlayerStartTag字符串，不重复计入历史问题。

修复验收（2026-10-12 11:40:45 +08:00）：fresh Engine.GameModeBase/FName及实际GameMode继承确认名称身份；局部PlayerStart比较函数统一named与reserved三处，保留None/有效空基名数字别名、数字边界、空请求与未知名回退。26名称边界/4种实际reserved组合通过，五消融失败、两份源码字节恢复后19项回归通过。正式target_game原反例及原Playground startup功能通过；坐标/fit/动画图未改，截图/性能延期。开始11:04、预估11:44、偏差−3分15秒；旧反例保留。

### P2-17 — PlayerStart 的局部变换直接用于出生

- Source：fresh B_Als_GameMode继承Engine.GameModeBase，无Find/Spawn覆盖；UE `GameModeBase.cpp:1214–1222` 取StartSpot.GetActorLocation/world yaw，再构造单位缩放的FTransform。
- Target：`src/als_extras/core/player_start_types.rs:98–101,354–406` 从局部Transform拟合出生点，只丢pitch/roll却保留scale；`game_mode.rs:248` 将fit.transform直接交角色factory。
- 反例：在完整加载角色/物理场景中安装实际PlayerStart与GameMode系统并请求admission。marker scale2生成的pawn scale=(2,2,2)；parent x20、marker local x−5时生成pawn x−5，源世界位置应15且缩放应1，实际出生反例FAIL。
- 影响：关卡分组/缩放出生点会使玩家出生在错误地点或错误大小，且占用探测与最终pawn尺寸不一致。旧测试仅无父级unit scale。初次夹具在App::finish后add_plugins失败已纠正，不把配置错误当作语义失败。

### P2-18 — 天空静态渐变替代了源构造和材质链

- Source：fresh World确认SkySphere及其mesh可见，ColorsDeterminedBySunPosition=true、DirectionalLight pitch=−45。fresh EngineSky `BP_Sky_Sphere.UserConstructionScript→RefreshMaterial` 把pitch从0..−90映射为0..1，使用SunHeight采样Clamped Linear Color曲线，再设置MID；直接使用Actor颜色是另一分支。
- fresh `C_Sky_Zenith_Color` 的RGB最后key位于0.470069，PostInfinity=Constant、颜色调整为默认值；time0.5的结果为(0.0228,0.048994,0.12)，与fresh世界MID相符。fresh `M_Sky_Panning_Clouds2` 的Zenith Color在75对象输出闭包内，存在真实材质消费者。
- Target：`assets/ALSExtras/Levels/L_Als_Playground/playground_scene.ron:221` 保存Actor编辑默认值；`src/als_extras/levels/playground.rs:1939–1970` 直接用这些值生成天空顶点，未执行源颜色构造分支。
- 反例：完整加载角色后调用实际 `playground_setup_system`，取得SkySphere真实Mesh顶点颜色。顶点Y=500处RGB=(0.034046054,0.10924697,0.29500008)，蓝通道为源有效参数的约2.46倍，0.91秒语义断言FAIL；并非截图观感或未使用字段差异。
- 漏检：旧场景验证只确认编辑属性与配置匹配、光照强度/方向和对象存在，没有追踪Blueprint构造后的有效MID输入。修复Gate应覆盖太阳角度/自动颜色开关、曲线和最终材质消费者，不能从保存的MID缓存硬编码单个默认场景。
- 同一静态替代的另一边界：fresh材质的Time×CloudSpeed连接Panner_4(SpeedX=0.001)→TextureSample_7→最终输出；当前CloudSpeed=2、CloudOpacity=1，fresh `T_Sky_Clouds_M` 2048×1024/RGBA逐像素导出确认R范围0..255。源云UV每秒变化0.002；实际setup生成的SkySphere为unlit StandardMaterial、base_color_texture=None，仅有依赖顶点高度的一次性渐变，云层及其滚动未移植。它与错误颜色同属此手写SkySphere替代源材质链，归并本项；修复不能止于颜色常量。

### P2-19 — 环境碰撞体使用了 Avian 默认物理材质

- Source：fresh World的`S_Arch_01.StaticMeshComponent0.PhysMaterialOverride=None`，fresh Arch BodySetup.PhysMaterial=None，源材质无覆盖。UE `Runtime/Engine/Private/PhysicsEngine/BodyInstance.cpp:3178–3244` 按body/component/setup/material优先级解析，最终回退`DefaultPhysicalMaterial`；当次原生PA记录确认该引擎资产Friction=0.7、Restitution=0.3、Combine=Average。
- Target：`src/als_extras/levels/playground_collision.rs:169–170`、`playground.rs:90`、`manual_playground.rs:451–452` 给环境安装body/collider而不安装物理材质，也无源默认材质资源映射。Avian `physics_material.rs:152,320` 的默认为0.5/0，narrow phase `system_param.rs:610–627` 在形状和body未赋材质时消费该默认。
- 反例：完整角色环境运行实际`playground_setup_system`和`PlaygroundCollisionPlugin`，等待源Arch mesh加载并生成真正Collider；实际摩擦0.5/弹性0，调用生产combine与ragdoll的0.7/0.3组合为0.6/0.15，源应0.7/0.3。0.92秒语义断言FAIL，没有把手工空白地板当作环境装配证据。
- 影响与漏检：ragdoll接触场景时的滑动阻力和反弹系数错误。旧PA Gate只确认身体自身材质；静态场景Gate检查几何/查询响应，没有覆盖场景与PA交界。修复Gate需覆盖各环境碰撞类型、运动平台、landscape、材质覆盖优先级及真实接触结果。

## 未定级观察

BF138 的固定 `0.046875` 秒 UphillJump 记录存在 pre-rig 腿平面异常，但旧报告缺少当帧完整图输入与混合源骨骼。它是待核实症状，不能据此认定源/目标差异或复开相邻图。

## 撤销记录

P2-01 Sprint 于2026-10-11 12:01 +08:00撤销。工程插件链接实际指向原始仓库的 Build 副本；原始 `ALSExtras/Private/AlsCharacterExample.cpp:75–76,127` 绑定 Triggered/Canceled 并按输入bool选择 Sprinting/Running，与目标一致。Build的Started/toggle属于本地调试修改，不能作为原始 `b754d6f0` 的移植合同。原错误断言与阶段报告保留作审计失误证据，不再计入已确认差异；编号不复用。146个原始C++/header完整比较，仅此cpp/header存在语义改动，AnimationInstance仅换行不同；已使用的6个uasset与原始资产字节一致，其他已登记差异的源文件不受影响。后续C++一律从原始插件根读取。

## 审计验证与限制

本轮未修改生产代码、转换规则或原始资产，以source审阅、静态合同及实际生产消费者的最小反例验证差异。没有新增生产设计、抽象或运行时分支，设计消融不适用；现有工具的缺Alpha、漏shader通道、错误碰撞模式等反例正常拒绝错误输入。本轮不是修复验收，未用这些测试替代未来修复所需的可见Playground Gate。

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

BF139.05使用39个本轮原生记录重转全部15个安装根（23AnimBP/236子图），耗时1720.352秒；359RON+15manifest对安装文件逐byte比较新增/删除/修改均0。当前完整source闭包53.931秒全部通过（98pose图、14FSM、104条件图、907节点、737pose连接）；规格读取使用与当前源码/资产逐byte一致的345份已有规格，审计同时核对各graph_source，未冒称新编译。510文件编辑器/运行时资产边界通过。scoped-cache旧专用合同误收类型规格产生schema失败，未当作资产缺陷；本批使用完整原生生成闭包与当前缓存算法取证。

127动画glTF严格raw逐轨比较104通过、23提示差异；原阈值和报告保留。23项为12个源ForceRootLock=RefPose下的raw根旋转差异、3个FName大小写匹配问题、8个小量化误差（最高0.08835 mm/0.18716°），未达到P2。A_Als_Idle的11个虚骨在目标均有动画通道，未把缺少运行中虚骨计算器误当缺失动画。当前22个有效最小测试13.61秒为3PASS/19预期语义FAIL，归并13项未修P2；证据存本地 `target/bf139/phase5/` 与 `target/bf139/graphs/`。无生成器、生产源或原始资产修改。

BF139.06当次读取FES_Als_Default（47.423秒）与B_Als_Character（52.268秒），原始/安装副本字节一致。完整本地通知审计137动画/蒙太奇、177通知、60marker及9montage segments均有效，生成manifest与安装文件byte一致。源AttachOverlayObject三项SnapToTarget、Clear置空mesh/class、非Low攀爬和ragdoll开始清除/结束恢复与目标一致。两QuickStop生产选择函数反例0.09秒失败并归并P2-05/08；无新增独立P2，证据在 `target/bf139/phase6/`。静态mesh/socket/material数据与场景资产批合并核对，尚未计入完成。

BF139.07从10个本轮原生来源重建camera definition，生成及源自测通过，消费数据相同。仅 `graph` metadata有旧generated/前缀：实际Camera加载固定scene wrapper，字段只用于合同测试，因此未当作当前运行时P2，也未宣称整个文件byte一致。原生读取批次最初误读外层LASTEXITCODE，已用reader报告确认两个成功结果并只继续未读项。暂停×零owner倍率的完整角色反例1.74秒确认P2-15，普通owner暂停对照通过，证据在 `target/bf139/phase7_camera_pause/` 与 `camera_source/`；当前有效14项未修P2。

BF139.08从14:00开始，预估14:55、实际15:00:06结束，偏差+5分06秒，在允许误差内。34项Overlay递归闭包778.113秒、34项环境闭包1333.879秒完成；追加5个EngineSky来源，确认P2-16–19四项。13模式/8物件的mesh骨架/socket/material定义与self-test通过，主角79骨骼一致；169静态部件mesh/transform、180物件Visibility、150物件材质参数、22文字、HUD/菜单、2移动/1旋转平台及4个运动曲线key一致。开始将旋转平台误传2×2网格的夹具错误已按fresh源引用改用3×3，不计作资产差异。

20环境mesh的8Box/5Convex/7TriMesh模式与源一致，5mesh/15凸包重建逐byte一致。严格Box尺寸比较保留1e−4 cm原阈值及原失败：仅S_Crate的半尺寸向量因导出取整相差0.0001400643 cm（约0.0014 mm），不计为P2，未把完全一致作为结论。四张环境/Footprint纹理全部RGBA逐像素一致；Grid与Footprint的G/A单次采样、缺Alpha/漏shader Alpha负例通过。天空的错误颜色和云层缺失、环境接触默认材质是消费者差异，分别归P2-18/19。

最终当前生产库29个最小反例/对照17.69秒完成：3PASS、26预期语义FAIL，归并18项有效P2；逐条检查失败位置，无资源加载、夹具前置或无进展超时失败，非零101为预期断言结果。证据保存在本地 `target/bf139/final_probe/`、`scene_source/`、`overlay_source/` 及相应合同JSON。全部118个本轮唯一native资产中的110个插件资产核对原始/安装副本逐byte一致；首次身份汇总漏计AnimBP格式已补齐，最终以118/110为准。生产源码、资产、工具、Cargo相对基线零变化，本轮只提交CHECKLIST及本报告。所有已确认差异已登记；历史UphillJump观察仍未被源事实定级，未擅自复开或宣称已解决。
