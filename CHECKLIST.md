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

## 附录 A：与 ALS 的刻意差异（非缺陷）

| ALS | bevy_als | 理由 |
|---|---|---|
| AnimBP 状态机 | 源合同生成 RON/FSM，由公开节点与 ECS 输入接线执行 | 保留源状态、转换及求值生命周期 |
| GameplayTags | `bevy_gameplay_tag` 的真实层级 Tag | 保留 UE Tag identity 与 exact/hierarchical query 语义 |
| ControlRig | 离线编译源 RigVM 程序，在源 ControlRig 节点中逐实例执行 | 保留 Construction/Forward Solve、当前 Pose/Curve 及图内求值位置 |
| Root Motion | 图内 `RootMotionMode` 提取 + `RootMotionOutput` 驱动位移（M8.8；数据经 M8.6 回填） | fork 已支持提取，M0.5 导出缺根轨道由 M8.6 离线回填，不再用曲线等价物 |
| Curves（UBT 资产） | als-ext CurveAsset + 采样节点 | 见 M2 |
| w_als_hud Overlay 菜单世界锚定头顶骨骼 | 菜单停靠 HUD 面板左侧（BF6b 用户决策 2026-08-25） | 解除 UI 对骨骼/相机投影的耦合 |
| W_Als_Hud 使用 Roboto 字体 | Bevy 默认字体（BF6b 用户决策 2026-08-25） | 字体非功能项，不移植 |
| L_Als_Playground TextRender 使用 RobotoDistanceField 与描边材质 | Bevy 默认字体布局/atlas 生成的 `Mesh3d` 世界字形四边形（BF-Scene 用户决策 2026-08-26） | 不带入字体资产；保留文字内容、颜色、世界锚点与固定朝向，不做相机 billboard |
| S_Als_UI_Click 菜单点击音 | 不复刻（BF6b 用户决策 2026-08-25） | WAV 解码需 bevy `wav` feature（hound 依赖），用户否决新增依赖 |
| UE Shift+数字仍触发玩法Action | Shift+数字用于Debug时仅抑制对应数字binding的玩法动作 | BF9用户决策（2026-08-25；4018824），手柄和非数字binding保持源行为 |

## 附录 B：风险与后置项

- **资产格式决策（2026-08-21）**：Bevy 官方推荐 glTF；M0.5/M0.6 管线定为 UE → glTF（Interchange 导出）→ bevy_animation_graph 自带的 glTF 导入器（GraphClip/Skeleton）。**不引入 bevy_ufbx**：其 v0.19.0（bevy 0.19 兼容）虽支持蒙皮数据，但动画曲线未接入 Bevy 动画系统、无社区验证，且 FBX 中转对 ALS 曲线/虚骨/单位换算风险更大；仅当未来需要直接加载 FBX 时再评估。

- bevy_animation_graph additive/mask 语义若与 ALS layering 不完全一致，在 M6 前验证并决定扩展方式（优先 als-ext 扩展点）。（已于 M6.1 验证通过，记录备查）
- Distance matching / motion matching 类高级特性仅实现 Stop 所需子集（M8.3）。
- ALSExtras（AI 控制器示例）不在范围内。
- 音频（Metasound）、Niagara 特效仅保留挂钩点。ALSCamera 完整移植已纳入 BF116.01。

## BF144 — ALS 运动系统持续 source-first 差异审计

用户授权（2026-10-13）：持续审计 ALS 运动系统本身，及时登记遗漏移植差异；周边内容暂不展开，截图关闭，性能验收继续暂停。本轮审计不修改生产实现或原始资产。未覆盖边界、未解释差异和确认缺陷均不得计作“无差异”；不以历史通过或有限测试宣称绝对等价。

范围按本地 ALS 源闭包推进：Character 状态/输入/旋转 → CMC 地面/空中/碰撞/基座/蹲跳 → Roll/Mantle → Ragdoll/GetUp → 运动动画输入/生命周期/通知 → 核心 AnimBP/Rig 与实际消费。排除 Camera、场景、材质、UI、Overlay 道具表现、音效/特效和网络同步。发现差异先追到实际消费者并保留最小反例，登记不代表授权修复或复开相邻已完成项。

- [x] BF144.01 本轮基线、源码闭包和覆盖入口：核对原始 ALS 插件/本地 UE/当前生产代码的身份，重新提取角色和运动设置，列出按源责任分组的审计入口与既有 Gate 漏检边界。Gate：fresh source 身份、native reader/watchdog 报告、范围表和后续独立审计项可核查。开始：2026-10-13 18:49 +08:00；预估结束：2026-10-13 19:13 +08:00；实际结束：2026-10-13 18:54:50 +08:00；偏差：−18分10秒；结果：基线 Gate PASS，语义审计按下表分项继续。估时依据：BF140 的 19 个 native 资产及 C++ 快照、BF140.03 的 20 分钟闭包审计记录；本项仅取角色/配置及清点源码，不运行完整图审计或构建。
  - 18:53 活动摘要：生产基线 `5554874b`，工作树干净；已读当前执行规则、源归属、BF140/BF141 复审记录和原生 reader 的 watchdog 入口。当前没有已确认新增差异；下一步核对原始插件身份、源函数责任与现有消费者，按规模拆分后续审计。截图/性能均未启动。
  - 18:54 source/static 闭环：原始插件 `b754d6f0`；112 个 ALS C++/header 与4个 UE Character/CMC/Actor/Capsule 文件已快照/hash，160 个 target Rust 文件仅做身份登记，不能视为已审计。fresh B_Als_Character、CS_Als_Default、MS_Als_Normal 均与 reader 实际加载的 Build 资产逐字节一致，3个 reader report PASS；`pwsh tools/run_ue_source_reader.ps1 -SelfTest` 的正常完成、停滞终止/子进程清理及机器报告全部 PASS。证据：`target/bf144/inventory.json`、`target/bf144/source/`。原始插件另有 Bow 资产工作区改动，属于本轮排除的道具，不据此扩展范围。复用既有 watchdog，未新增生产抽象/状态/分支，设计消融不适用。

| 顺序 / 源责任 | 本轮 target 核对入口 | 覆盖状态与旧 Gate 边界 |
|---|---|---|
| Character 输入、Gait、RotationMode、Grounded/InAir rotation | `state/{aim,gait,transition}`、`character/movement/motion`、`actual_motion` | BF144.02完成首轮source/focused审计，确认D01–D04；关联蹲跳/落地回调和动作生命周期留对应责任批次 |
| CMC 时钟、速度、地面/空中、碰撞、蹲跳、基座 | `character/movement/{motion,dynamics,control_scheme,source_physics_timing}`、`tnua_ext` | BF144.03完成速度/时钟审计，确认D05–D08；地面查询、步阶、蹲跳与基座待后续独立批次 |
| Roll、Mantle、RMS、动作切换 | `character/actions/{roll,roll_drive,mantle}`、`root_motion_sources` | 待逐项审计；角色蓝图 SelectMantlingSettings 属此闭包 |
| Ragdoll、GetUp、PA 运动消费者 | `character/{pa_als,actions/ragdoll_link,actions/get_up}`、`avian_ext` 的实际调用 | 待逐项审计；不开展无源链路的通用物理审查 |
| AnimationInstance 输入、生命周期、Feet/Turn/Transition、通知 | `character/animation_instances`、`state`、`notifies` | 待逐项审计；实际动画选择包含角色 Overlay 标签，但排除道具表现 |
| 核心 AnimBP/FSM、曲线、同步、Root Motion、Rig | 已生成核心图、实际节点与 `control_rig` | 待重新取证及核对；历史155子图合同只作起点，不能自证算法等价 |
| 剩余源函数与跨阶段边界 | 以上 source-target 链路的覆盖缺口 | 前述审计后再独立收口；未解释项保留，截图与性能继续暂停 |

- [x] BF144.02 Character 输入、状态与旋转语义：以 fresh Character/CMC、角色/运动配置核对初始化、输入、步态、旋转模式与地空旋转的有效分支；沿实际消费者确认候选，并用独立最小反例与对照记录遗漏。Gate：逐源分支覆盖记录、生产库身份与 focused 结果、确认/排除候选登记。开始：2026-10-13 18:55 +08:00；预估结束：2026-10-13 19:40 +08:00；实际结束：2026-10-13 19:19:06 +08:00；偏差：−20分54秒；结果：审计记录Gate PASS，确认4项未修复差异，不能视为移植等价。估时依据：Character 约1960行与既有 motion/state 消费者、BF140 的15项基础运动反例和 BF141.03/.04 focused 回归；将 CMC 积分/碰撞另列后续项以控制本项规模。
  - 18:55 活动摘要：基础配置/输入/状态函数已定位；候选是离开 Grounded 后保留的 MaxAllowedGait 与 RotationMode 消费条件，尚未确认缺陷。下一步建立 source 分支对照与真实角色反例，不修改生产代码。
  - 19:00 source/focused 闭环：fresh `RefreshGait`/`RefreshRotationMode` 与 target aim→root/provider 链已核对，确认 BF144-D01。生产启动器同 target 构建 PASS；独立链接当前优化库，Grounded/Air 72组为0差异，Custom/None 72组有18处语义失败，完整角色 sprint→mantle 复现错误 Aiming。另有 stationary yaw curve 最小失败，但尚未证明实际动画触发，保留候选不计缺陷。初始 `cargo build --lib` 使用不同依赖特征而意外扩大重编，已核对 PID/target 后终止本轮子进程并改回启动器的 example 构建；终止报告保留，不作通过。证据：`target/bf144/02/initial/`、`launcher_library_identity.watchdog.json`；截图/性能未启动。
  - 19:05 曲线候选范围：fresh `ApplyRotationYawSpeedAnimationCurve` 证明回写 actor yaw 后必须刷新 target；target `standing_layer:2390` 直接把最终 `RotationYawSpeed` 交给 `motion:477`。仅沿这条边追加 AB_Als_Standing 取证和实际 Turn→VelocityDirection 组合夹具。首个完整角色夹具未产生曲线，诊断发现无渲染夹具进入了源“不可见只更新Montage”策略，属于前置失败，不计缺陷；改为标记实际角色 mesh 可见后再验证。未启动公共 Playground Gate。
  - 19:08 source/focused 闭环：fresh `PreRegisterAllComponents` 与 rotation state 初始化链确认 BF144-D02；4个新建角色的零度对照通过，3个非零朝向反例失败。fresh AB_Als_Standing reader PASS；实际 Turn 已产生31.67度/秒曲线。首个消费采样位于 Standing/Crouching producer 前，存在同帧写入歧义，不能据此确认曲线问题；已将采样移到 producer 后、rotation consumer 前继续核对。
  - 19:10 source/focused 闭环：消费采样顺序已闭合，实际 ViewDirection Turn→VelocityDirection 输入切换在 producer 后捕获47.6275度/秒曲线；source 单步应到0.0380855rad，target实际0.0365703rad，确认 BF144-D03。独立固定90度/秒的最小消费者同样失败；AB_Als_Standing 源记录有效，没有改图/曲线/转换器。下一步完成 Character 剩余分支对照和本项归纳，再进入 CMC。
  - 19:16 source/focused 闭环：独立degree-space瞄准公式共5040组，36处差异全部集中在 zero owner delta 的精确半转边界；实际运动系统倍率1/.001对照到+110°，倍率0却到−110°，fresh UE Actor Tick 证明0倍率仍调用Character Tick，确认 BF144-D04。fresh 两条 RotationInterpolationSpeed 曲线已读取，Normal/Responsive 锚点与当前目标常量一致。D01另由真实 ragdoll 输入触发，错误Aiming已读回动画玩家 `SourceAnimationInputs`，不只存在于状态镜像。
  - 19:19 本项归纳：已核对PreRegister/BeginPlay初始化、RefreshInput归一化/保留角度、Gait许可与actual阈值、RefreshRotationMode各地空/其他分支、Grounded moving/stationary与FirstPerson/Aiming、源InAirRotationMode=3保持分支、target/smooth target及限位生命周期；角色蓝图ParentClass实际指向的 `AlsCharacterExample` 仅追运动输入分支，不展开相机/UI。状态请求的desired gait/stance与actual分离符合源结构；物理蹲起、跳跃、落地动作分别留CMC/动作批次。6个fresh native来源与Build原资产一致，两种旋转曲线的线性插值由fresh UE RichCurve构造默认确认。当前库SHA-256为`f45addc2…ce719ee`，160个生产源码hash未变；32项focused最终24PASS/8预期语义FAIL，失败归并D01–D04，旧23项全部PASS。证据 `target/bf144/02/final/`、`02/summary.json`；无实现修改、无新生产设计可消融、未做截图或性能验收。

- [x] BF144.03 CMC 速度、加减速和时钟：沿fresh CMC CalcVelocity/RefreshGroundedMovementSettings及其实际UE基类算法，核对运动配置选择、输入模拟量、加速/转向摩擦/制动、地空切换与owner/world时钟；接触/地面查询/步阶另列下批。Gate：逐源合同与target消费者闭环、最小公式/实际运动反例及对照、确认/排除登记。开始：2026-10-13 19:20 +08:00；预估结束：2026-10-13 20:10 +08:00；实际结束：2026-10-13 19:52:12 +08:00；偏差：−17分48秒；结果：审计记录Gate PASS，新增3项P2与1项P3，未修复。估时依据：ALS CMC约1040行、UE CalcVelocity/ApplyVelocityBraking/NewFallVelocity实际闭包与现有dynamics/source_physics_timing消费者；BF140基础运动批次及本次BF144.02实耗24分钟，增加曲线取证和积分边界后独立估时。
  - 19:20 活动摘要：本次B_Als_Character明确引用MS_Als_Normal，其VelocityDirection/Standing的加减速曲线引用Normal；target `grounded_dynamics_profile` 按VelocityDirection选择Responsive。先核对曲线实值、选择调用链和生产运动反例；此时仍是候选，尚未确认新缺陷。截图与性能保持暂停。
  - 19:23 source/focused 闭环：fresh两条CV曲线证明Normal起步为20/15/5（m/s²、m/s²、摩擦），Responsive为25/20/10；当前grounded motor真实消费错误profile，6种stance×rotation组合仅Standing/VelocityDirection失败，确认D05。源码排除“每子步应重新采样”的猜测：ALS PhysWalking在while之前调用RefreshGroundedMovementSettings一次。下一步核对UE CalcVelocity/Braking公式和实际owner/world时钟，WorldSettings仅作为角色delta输入依赖取证，不扩展场景表现。
  - 19:30 source/focused 闭环：fresh原生读取限定为Playground的WorldSettings对象，确认世界delta上下限0.0005/0.4与倍率1；UE WorldSettings→Actor→CMC链和当前入口DefaultPlugins时钟对比确认D06，实际角色0.3/0.5秒长帧只积分0.25秒。排除air analog候选：本地UE5.8 GetMinAnalogSpeed明确包含Falling；原夹具“Falling返回0”的oracle错误已纠正并保留为排除记录，不能计缺陷。截图、场景表现与性能测试均未展开。
  - 19:36 source/focused 闭环：D05再经完整默认角色按冲刺输入确认，切到VelocityDirection后的速度0.75而非source0.666667m/s。fresh UE ControlledCharacterMove→ScaleInputAcceleration→ALS PhysWalking顺序证明输入Acceleration先于本轮curve refresh，target却用新值，独立于D05的FirstPerson/ViewDirection反例前12帧PASS、第13帧FAIL，确认D07。新CMC focused共5项，air analog对照PASS，4个语义失败归并D05–D07；未重新运行或改写旧审计结论。
  - 19:49 source/focused 闭环：fresh PhysicsSettings CDO确认默认重力−980cm/s²、terminal4000cm/s、同步无substep且独立物理cap约1/30；WorldSettings无gravity override，实际target重力却−9.81m/s²，默认0.1秒落体velocity−0.981而source−0.98，源重力显式对照PASS，登记较小差异D08/P3。world0.1秒×owner0/.5/1/2的velocity/位移/CMC delta对照全PASS。当前CMC focused7项2PASS/5语义FAIL归并D05–D08；完整结果见`target/bf144/03/final/`，没有把失败报告当验收通过。
  - 19:52 本项归纳：已核对grounded profile/连续曲线、输入缩放时点、CalcVelocity制动/转向摩擦/overspeed恢复/再加速、独立braking半尾子步、CMC末步预算、AirControl/boost/MinAnalog、无接触自由落体/terminal与World/owner/physics三层时钟。AI requested movement/RVO不在本项目范围；着地剩余时间与几何接触留后续碰撞批次。fresh4个新增native report与两条CV/原关卡包身份检查PASS，PhysicsSettings采用实际CDO，160份生产源码及已验证库hash未变；`03/summary.json`仅验收审计记录完整性，不宣称运动等价。无生产修改、无设计消融对象、截图0、性能继续暂停。

### 已确认差异（仅登记，未实施修复）

- [ ] BF144-D01 / P2 — 动作期间的旋转模式丢失保留的冲刺优先级。
  - Source：fresh `AlsCharacter.cpp:675–758` 的 `bSprinting` 只读 CMC MaxAllowedGait；`942–958` 非 Grounded 时不刷新该值，Custom/None 按“Grounded and other locomotion modes”分支继续保留冲刺优先级。fresh CS_Als_Default 确认 SprintHasPriorityOverAiming/RotateToVelocityWhenSprinting 均为 True。
  - Target：`src/als/state/aim.rs:69–71` 额外要求 Grounded+Standing 才认定 sprinting；`character/animation_instances/ab_als_root.rs:184` 与 `ab_als_provider.rs:343` 把错误模式传入动画图和 Turn/Rotate 许可。
  - 反例：实际加载角色前进、冲刺、瞄准，Grounded 下 VelocityDirection 正常；进入活动 High Mantle 的第0帧，Custom/MaxAllowedGait=Sprinting 保持，却变为 Aiming，source 应仍为 VelocityDirection。`bf144_sprint_to_mantle_preserves_source_rotation_mode` 的资产/动作前置通过后语义 FAIL；72组地空对照全通过，72组 Custom/None 有18处不符。影响翻越/布娃娃期间状态及动画输入，尚不宣称所有18组都已走真实动作路径。
  - 漏检：`desired_aiming_intent_survives_button_release_and_source_mode_branches` 只枚举 Grounded/Air；旧 Custom 测试验证 actor 不旋转，却没有调用并核对 `RefreshRotationMode`。最小复现源/日志保留在 `target/bf144/probe.rs`、`target/bf144/02/initial/`。本项未修改实现、未复开相邻已完成项。

- [ ] BF144-D02 / P2 — VelocityDirection 首次更新没有从出生朝向初始化目标。
  - Source：fresh `AlsCharacter.cpp:92–116` 的 PreRegisterAllComponents 先以 actor yaw 调用 SetTargetYawAngle，并初始化 InputYawAngle/VelocityYawAngle；静止且 blocked 的 VelocityDirection 分支继续使用这个初始目标。
  - Target：`src/als/character/movement/motion.rs:2313–2325` 默认 target_yaw=0、blocked=true；`489–502` 首次只初始化 smooth_target_yaw，随后仍选择零 target。`AlsCharacterBundle` 没有其他目标初始化消费者。
  - 反例：用实际运动插件新建静止角色，出生模式 VelocityDirection，yaw分别0°/45°/−90°/170°。零度对照通过；其余三组首帧实际 yaw为43.542°/−88.542°/168.542°，target均为0，source应保持出生角度。`bf144_spawn_velocity_direction_initializes_target_from_actor` 语义FAIL，证据 `target/bf144/02/spawn/`。该触发依赖初始速度朝向配置，当前默认 ViewDirection 出生不受此分支影响。
  - 漏检：已有旋转测试大多从 identity 出生或手动填入 initialized=true 的 response；父空间测试验证 world/physics 一致，未核对第一次静止 VelocityDirection 的目标。仅登记，不改实现。

- [ ] BF144-D03 / P2 — 转身曲线在切换到 VelocityDirection 后被旧目标抵消。
  - Source：fresh `AlsCharacter.cpp:1529–1573,1812–1823` 先应用 RotationYawSpeed，再由 RefreshTargetYawAngleUsingActorRotation 同步 target/smooth target；静止 blocked VelocityDirection 随后使用更新后的目标。fresh AB_Als_Standing 的实际 Turn 曲线经现有 Standing/Crouching producer 进入消费者。
  - Target：`src/als/character/movement/motion.rs:477–497` 只更新局部 current_yaw/smooth_target_yaw，没有同步 target_yaw；blocked VelocityDirection 又取旧 target，后续插值抵消曲线转向。
  - 反例：实际加载角色和动画，静止 ViewDirection 朝120°触发 Turn；按真实 rotation_mode_pressed 切换为 VelocityDirection。捕获位于 `AlsCrouchingLayerSet` 后、`AlsRotationResponseSet` 前，当帧曲线47.6275度/秒、dt=1/60；source yaw应0.0380855rad，target为0.0365703rad。`bf144_turn_to_velocity_rotation_uses_actual_curve` 的可见mesh、Turn曲线、静止和切换前置均通过后语义FAIL；固定90度/秒消费者反例也失败，记录在 `target/bf144/02/turn/`。
  - 漏检：`stationary_turn_curve_starts_at_actor_yaw_and_retains_its_target` 仅测试 ViewDirection，`velocity_direction_retains_rotation_target_after_velocity_stops` 不包含曲线；未组合模式切换与尚未消退的 Turn 曲线。未将初始无曲线夹具和旧采样时点计入有效反例，未修改生产实现。

- [ ] BF144-D04 / P2 — 冻结角色的瞄准限位在精确半转处选到相反一侧。
  - Source：fresh `AlsCharacter.cpp:1718–1787` 先保留 counterclockwise remap 的符号，secondary 在0 delta只做0量插值，再执行primary clamp；UE `Actor.cpp:372–379,1997–2003` 证明 CustomTimeDilation=0仍传0调用Character Tick，并非全局暂停。
  - Target：`src/als/character/movement/motion.rs:244–246,261–267` 用通用 damper_yaw 代替该段标量算法；0 delta仍执行末尾 `wrap_yaw_radians`，将 +π 变成−π，随后primary限位反向。
  - 反例：静止瞄准、已应用70°限位、view与body相差180°。实际运动插件在 owner倍率1/.001时到+110°（对照PASS），倍率0时到−110°（source仍应+110°）；`bf144_frozen_owner_aiming_half_turn_keeps_source_side` 语义FAIL。独立source-degree公式5040组中36处同类边界FAIL，其余5004组无差异。证据：`target/bf144/02/frozen/`。
  - 漏检：旧瞄准测试覆盖reset/primary/secondary和普通帧长，暂停回归在稳定角度暂停；没有覆盖0 owner delta×半转×已收紧限位。仅登记，不改公式或放宽阈值。

- [ ] BF144-D05 / P2 — Standing/VelocityDirection 错选 Responsive 加减速曲线。
  - Source：fresh B_Als_Character→MS_Als_Normal，三种站姿rotation的 `AccelerationAndDecelerationAndGroundFrictionCurve` 均引用Normal，蹲姿才引用Responsive；VelocityDirection仅 `RotationInterpolationSpeedCurve` 引用Responsive。fresh两条CV资产确认起步Normal=20/15/5、Responsive=25/20/10（前两项已从cm换算m）。
  - Target：`src/als/settings/params.rs:78–99` 把VelocityDirection当作加减速Responsive条件；`character/movement/dynamics.rs:543–565` 的实际motor以及stop预测、落地剩余积分共享这个选择。
  - 反例：真实运动插件的站姿VelocityDirection首帧无障碍前进速度0.4166667m/s，source Normal应0.3333333m/s；实际加速/制动/摩擦为25/20/10而非20/15/5。三rotation×两stance的另外5组都通过。`bf144_cmc_grounded_dynamics_uses_source_stance_mapping` 语义FAIL，证据 `target/bf144/03/profile/`。
  - 漏检：`defaults_match_als_source_values` 和 `grounded_dynamics_samples_match_the_live_source_contract` 把错误VelocityDirection→Responsive映射写进oracle；验证了曲线数值而没有核对MS_Als_Normal中加减速与旋转曲线的独立引用。仅登记，不改源码或旧测试。

- [ ] BF144-D06 / P2 — 角色世界时钟未移植源帧长上下限。
  - Source：fresh Playground `WorldSettings` 对象的MinUndilatedFrameTime=0.0005、MaxUndilatedFrameTime=0.4，三种TimeDilation均1；UE `WorldSettings.cpp:334–342` 对世界delta施加该范围，经LevelTick/Actor进入Character/CMC。独立物理上限1/30不替代角色世界delta。
  - Target：`src/als_extras/levels/manual_playground.rs:56–73` 使用DefaultPlugins且无世界delta配置；当前依赖 `bevy_time::Virtual` 默认上限0.25秒，未实现源最小帧长。`character/movement/motion.rs:3708` 再把该值用于普通CMC，所以后续完整帧适配无法恢复已被全局截去的时间。
  - 反例：实际加载角色在空中以1m/s水平速度运动，raw帧0.2秒得到delta/位移0.2（PASS）；raw0.3秒得到0.25而source应0.3；raw0.5秒得到0.25而source应0.4。`bf144_cmc_hitch_uses_source_world_delta_bounds` 同时读取Time<Virtual>、CMC simulation delta及实际Position，语义FAIL；机器记录 `target/bf144/03/world_clock/`。
  - 漏检：旧完整帧和hitch验收测试主要使用0.1/0.12/0.2秒，启动器还按target现有max_delta限制注入值，未覆盖250毫秒上方和source WorldSettings的单独合同。只记录世界时钟对运动的影响，不开展性能验收或场景修复。

- [ ] BF144-D07 / P2 — 输入Acceleration使用了本轮刚刷新的曲线，而非源调用顺序中的缓存值。
  - Source：fresh UE `CharacterMovementComponent.cpp:6442–6457,8134–8137` 在ControlledCharacterMove先用GetMaxAcceleration缩放输入并缓存Acceleration；ALS `GetMaxAcceleration:292–299` 读取之前的MaxAccelerationWalking；随后ALS `PhysWalking:333–335` 才RefreshGroundedMovementSettings。UE CalcVelocity直接使用缓存Acceleration，不用新的MaxAccel重算普通玩家输入。
  - Target：`src/als/character/movement/dynamics.rs:543–564` 先覆写dynamics，再把fresh dynamics交给积分；`grounded_velocity_step_with_braking_friction:175–176` 立即以fresh acceleration缩放输入，丢失源阶段边界。
  - 反例：第一人称、ViewDirection、默认Normal曲线持续冲刺，排除D05的错误profile。前12帧速度一致；第13帧起始4.000003m/s，缓存加速度20、本轮曲线16.972263m/s²。source应到4.333337m/s，target到4.282876m/s。`bf144_cmc_input_acceleration_is_scaled_before_curve_refresh` 前置和12帧对照通过后语义FAIL，证据 `target/bf144/03/acceleration_order/`。
  - 漏检：旧位移/子步矩阵把加速度曲线换成常数；其他oracle直接调用target的fresh dynamics helper，未跨越source输入缩放与PhysWalking刷新这两个阶段。仅记录，未添加缓存或修改实现。

- [ ] BF144-D08 / P3 — 默认重力继承了Avian的9.81，未采用源9.80m/s²。
  - Source：fresh `/Script/Engine.Default__PhysicsSettings` 的DefaultGravityZ=−980cm/s²；fresh Playground WorldSettings的bWorldGravitySet/bGlobalGravitySet均False，`WorldSettings.cpp:290–299` 读取该默认值。源CharacterMovement GravityScale=1。
  - Target：实际`Gravity`及`AlsEffectiveGravity`为−9.81m/s²。`manual_playground.rs:146–153` 仅在显式测试环境变量下覆盖重力，默认入口没有消费source gravity配置；运动系统沿用默认PhysicsPlugins资源。
  - 反例：完整加载角色0.1秒无输入自由落体，target竖直速度−0.981m/s，source应−0.98m/s；同一入口显式给予−9.8重力后对照PASS。`bf144_cmc_default_gravity_matches_source_world` 默认语义FAIL，差异约0.102%，不计为浮点舍入噪声。证据 `target/bf144/03/gravity/`。
  - 漏检：若干轨迹oracle直接写9.81或与当前Gravity资源比较，只验证积分自洽；其他夹具显式注入9.8，绕过了默认资源选择。仅登记，未修改世界或物理设置。
