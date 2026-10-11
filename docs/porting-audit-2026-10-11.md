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
| Character、CMC、输入与标签状态 | 已审计，6项P2 | 本地 C++、当前 ECS 调度、初始化/状态/输入/旋转/速度/蹲起/基座/碰撞/时钟的实际消费者；网络与纯 Editor 排除 |
| Roll、Mantle、GetUp、root motion | 已审计，新增3项P2 | 启动/抢占/退出、蒙太奇设置、slot/曲线/notify输入、root、动作时钟与目标基座 |
| Ragdoll、物理适配 | 进行中 | PA/约束/碰撞与查询 |
| AnimationInstance、图/FSM/同步/曲线、资产转换与 Rig | 待审计 | 源递归依赖到已安装图及实际求值消费者 |
| 通知、脚步/贴花、Overlay 物件 | 待审计 | 通知时间/权重/结束与最终姿态消费 |
| Camera、Playground、材质、HUD/UI、PlayerStart | 待审计 | 本地源及资产配置到真实入口 |

## 已确认 P2 及以上

### P2-01 — Sprint 的事件消费仍是按住语义，源已是按下切换

- Source：`ALSExtras/Private/AlsCharacterExample.cpp:110` 绑定 `ETriggerEvent::Started`，`:161` 的 `Input_OnSprint` 在 Running/Sprinting 间切换。fresh `IA_Als_Sprint` native export 的 Hold=0.1、OneShot=true、Released 仍在；触发器存在不等于 C++ 消费 Triggered/Released。
- Target：`src/als_extras/core/input.rs:433` 消费 FIRE/CANCEL 并保存 bool；`src/als_extras/core/input_request.rs:66` 按 bool 上升/下降沿设置 Sprinting/Running。
- 反例：Running → 按 Shift 12 个 20 ms 帧 → 松开 3 帧。实际目标为 Running，源应保留 Sprinting，下一次 Started 才切回。原生输入插件和当前生产库的断言失败；按住期间 Sprinting 的正对照通过。
- 影响：冲刺按键操作、松手后的速度和动画选择与当前本地源不一致。现有测试把“释放清除 Sprint intent”当期望，只核验资产触发器，漏掉当前 C++ 事件绑定。

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

### P2-09 — Roll/GetUp 的自然混出提前且未保留实际剩余预算

- Source：UE `Animation/AnimInstance.cpp:573–578` 先调用 `Montage_UpdateWeight`，再调用 `Montage_Advance`。`Animation/AnimMontage.cpp:2626–2636` 在本次 Advance 首次进入窗口时才执行 Stop，默认负 TriggerTime 使用当前剩余播放时间；Stop 只 `Blend.Update(0)`，不会在该帧额外推进混出。
- Target：`roll.rs:48` 与 `get_up.rs:112–130` 根据推进后的 elapsed、固定默认 BlendOut 直接计算权重，没有首次进入窗口时的剩余时间状态。
- 反例：100 ms帧、实际加载的 Roll（序列1.5 s、播放1.3、BlendOut0.3）到elapsed0.9时源weight=1，实际0.93627644。通过真实ragdoll退出启动 GetUp（序列1.5 s、RateScale1.2、BlendOut0.3），elapsed1.0时源weight=1，实际0.92592555。两个实际slot断言FAIL，进入窗口前weight1对照PASS。
- 影响：动作尾段pose/curve/notify权重与源错位，后续混出还使用不同预算。原有测试验证曲线形状或中断fade，没有验证源 UpdateWeight/Advance/首次自然Stop的顺序。Mantle使用自定义TriggerTime=0的末点停止，不按本项默认自然窗口直接推断缺陷。

## 未定级观察

BF138 的固定 `0.046875` 秒 UphillJump 记录存在 pre-rig 腿平面异常，但旧报告缺少当帧完整图输入与混合源骨骼。它是待核实症状，不能据此认定源/目标差异或复开相邻图。

## 审计验证与限制

本轮不修改生产代码、转换规则或原始资产；无需以全构建替代逐项 source 证据。只有与具体差异有关的最小验证会执行。没有新增设计、抽象或运行时分支，设计消融暂不适用。

BF139.01 使用当前生产库链接独立 Rust 反例，保存于本地 `target/bf139/phase1/`（probe源码、stdout/stderr、watchdog JSON）。9个测试为7个预期语义失败和2个通过对照；期望失败归并为6项差异，不能解读为移植验收通过。原生库未修改；最初缺少Windows native link-search和一个Position字段访问错误均已纠正，不计入语义证据。reader watchdog `-SelfTest` 的正常完成/无进展终止/子进程清理/报告用例全部通过。

本地 ALS 插件版本身份：`b754d6f0f2bb03741d301f8fb88077ebfe561e17`；本轮源 C++ SHA-256：

| 文件 | SHA-256 |
|---|---|
| AlsCharacterExample.cpp | `bcfacb62ffc48eb77937c4c9a74ac68d49d8b5a9d687374530c0e72f406cb833` |
| AlsCharacter.cpp | `779192e94d0a1ac13ccf95cfa9db540cc4f6230a2dc3ab9113848f8d983ace84` |
| AlsCharacterMovementComponent.cpp | `e37d349bc41a5de208cbdd1799c32741cf84aa5bddfe5580906c50f2a6360dda` |
| UE Actor.h | `2ee1effb7ab4ac4cf65cf6caa90a27cea45361feefaae0cb673f7265e00bc5c9` |
| UE CharacterMovementComponent.cpp | `fcf827d7c5adaa73a4f272a716a53b03f66fc7352f3eacc1be4d51cc9b3e15a8` |
| AlsCharacter_Actions.cpp | `3dbd91f26154fd99297a7c7bb293993d938a86b8937d7071760cabc8517aa317` |
| AlsRootMotionSource_Mantling.cpp | `624964d860117f3794d249a2b8d25e5d0c610b7dd852751e052e60bb19b7d55e` |

BF139.02 的实际角色反例在资产就绪后执行；根图异步加载期间的临时 OutputMissing 不作为本次动作缺陷。fresh native导出 Roll/GetUp Front/Mantle High 已完成，生成数据的播放率/混入/混出/Trigger字段已核对。累计14测试为12预期语义FAIL和2对照PASS；本地源码与原始报告存于 `target/bf139/phase2/`。源 `PhysCustom` 的 MoveUpdatedComponent 明确不扫掠，不能把攀爬中不扫掠定为移植缺陷。
