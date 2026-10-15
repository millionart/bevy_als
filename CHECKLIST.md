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
| 默认重力加速度9.80m/s² | 统一使用Bevy侧重力值（当前Bevy/Avian默认9.81m/s²） | 用户决策2026-10-13；审计运动算法时使用统一的Bevy重力输入，不按源默认数值回改 |
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

用户授权（2026-10-13）：持续审计 ALS 运动系统本身，及时登记遗漏移植差异；周边内容暂不展开，截图关闭，性能验收继续暂停。重力加速度统一采用Bevy的值，源默认9.80m/s²与当前Bevy/Avian默认9.81m/s²的差别是刻意差异，不按缺陷处理。本轮审计不修改生产实现或原始资产。未覆盖边界、未解释差异和确认缺陷均不得计作“无差异”；不以历史通过或有限测试宣称绝对等价。

范围按本地 ALS 源闭包推进：Character 状态/输入/旋转 → CMC 地面/空中/碰撞/基座/蹲跳 → Roll/Mantle → Ragdoll/GetUp → 运动动画输入/生命周期/通知 → 核心 AnimBP/Rig 与实际消费。排除 Camera、场景、材质、UI、Overlay 道具表现、音效/特效和网络同步。发现差异先追到实际消费者并保留最小反例，登记不代表授权修复或复开相邻已完成项。

- [x] BF144.01 本轮基线、源码闭包和覆盖入口：核对原始 ALS 插件/本地 UE/当前生产代码的身份，重新提取角色和运动设置，列出按源责任分组的审计入口与既有 Gate 漏检边界。Gate：fresh source 身份、native reader/watchdog 报告、范围表和后续独立审计项可核查。开始：2026-10-13 18:49 +08:00；预估结束：2026-10-13 19:13 +08:00；实际结束：2026-10-13 18:54:50 +08:00；偏差：−18分10秒；结果：基线 Gate PASS，语义审计按下表分项继续。估时依据：BF140 的 19 个 native 资产及 C++ 快照、BF140.03 的 20 分钟闭包审计记录；本项仅取角色/配置及清点源码，不运行完整图审计或构建。
  - 18:53 活动摘要：生产基线 `5554874b`，工作树干净；已读当前执行规则、源归属、BF140/BF141 复审记录和原生 reader 的 watchdog 入口。当前没有已确认新增差异；下一步核对原始插件身份、源函数责任与现有消费者，按规模拆分后续审计。截图/性能均未启动。
  - 18:54 source/static 闭环：原始插件 `b754d6f0`；112 个 ALS C++/header 与4个 UE Character/CMC/Actor/Capsule 文件已快照/hash，160 个 target Rust 文件仅做身份登记，不能视为已审计。fresh B_Als_Character、CS_Als_Default、MS_Als_Normal 均与 reader 实际加载的 Build 资产逐字节一致，3个 reader report PASS；`pwsh tools/run_ue_source_reader.ps1 -SelfTest` 的正常完成、停滞终止/子进程清理及机器报告全部 PASS。证据：`target/bf144/inventory.json`、`target/bf144/source/`。原始插件另有 Bow 资产工作区改动，属于本轮排除的道具，不据此扩展范围。复用既有 watchdog，未新增生产抽象/状态/分支，设计消融不适用。

| 顺序 / 源责任 | 本轮 target 核对入口 | 覆盖状态与旧 Gate 边界 |
|---|---|---|
| Character 输入、Gait、RotationMode、Grounded/InAir rotation | `state/{aim,gait,transition}`、`character/movement/motion`、`actual_motion` | BF144.02完成首轮source/focused审计，确认D01–D04；关联蹲跳/落地回调和动作生命周期留对应责任批次 |
| CMC 时钟、速度、地面/空中、碰撞、蹲跳、基座 | `character/movement/{motion,dynamics,control_scheme,source_physics_timing}`、`tnua_ext` | BF144.03–.13完成速度/时钟、默认floor/胶囊查询、walking/Air接触、落地回调、蹲起、Jump/apex、基座transform/速度继承、物理交互及选定剩余ledge/恢复首轮审计，确认D05–D07、D09–D42；重力D08已撤销；BF144.43将U01原生核验归并为D125，动作mode锁/Root Motion归下一闭包，未宣称通用UE模式全面等价 |
| Roll、Mantle、RMS、动作切换 | `character/actions/{roll,roll_drive,mantle}`、`root_motion_sources` | BF144.14–.19完成Roll及Mantle探测/许可/设置选择/生命周期/root轨道、warp变换、动态基座和RMS物理消费的首轮审计，确认D43–D60、D62–D64，D61撤销；精确查询数值及公共Playground验收尚未闭合 |
| Ragdoll、GetUp、PA 运动消费者 | `character/{pa_als,actions/ragdoll_link,actions/get_up}`、`avian_ext` 的实际调用 | BF144.20–.42及.44–.64完成许可、启动/退出、输入/锁定、GetUp交接/时钟/通知/清理/姿态/曲线/根运动、PA body/shape/质量/有效碰撞、关节frame/drive/求解、conditioning/projection、body积分/时钟/sleep、原生CCD关闭、法向接触/回弹、单点摩擦/历史锚点、接触采样/child预测bounds、多点回弹迭代、原生点序/支持点消费、质量加权接触位置、contact/joint耦合、primitive缓存、初速生成/交接/限速预算、pelvis跟随/探地/恢复朝向、进入挂接/退出快照、mesh恢复及零scale重建首轮审计，确认D65–D124、D126–D145；实际convex/trimesh的几何、速度摩擦及跨帧消费已由.45–.64完成当前输入闭包分类，U02关闭；不开展无源链路的通用物理审查 |
| AnimationInstance 输入、生命周期、Feet/Turn/Transition、通知 | `character/animation_instances`、`state`、`notifies` | BF144.65–.71完成可见性更新/历史时钟、初始化/实例替换、View/Head/Spine、Layering/Pose与Locomotion通用输入、Grounded/Standing/Crouching/InAir运动算法、地面预测查询及Feet输入/Foot Lock，确认D146–D152、D154–D158；.81–.92完成动态过渡资格/选择/队列、播放时钟、单/多实例混合、组停止、惯性请求、队列仲裁/落地参数、当前显式过渡/QuickStop以及Turn资格/选择/速率、时钟/自然停止和非加性混合、Roll运动点通知采样/资格/模式消费，新增D166–D179并扩展D118/D120/D121/D123。.93完成排队RootScale状态/中断/重入与实际根运动倍率消费，新增D180–D182；.94完成九个动作montage的native branching动作状态、标记/零步长、中断/跨实例保护及动作消费，新增D183/D184并扩展D121；.95完成六个Mantle的EarlyBlendOut条件/实例/停止权重与输入消费，新增D185；槽相关性与跨阶段派发、完整惯性求解器和外部参数边界仍待审计；实际动画选择包含角色Overlay标签，排除道具表现 |
| 核心 AnimBP/FSM、曲线、同步、Root Motion、Rig | 已生成核心图、实际节点与 `control_rig` | BF144.70沿InAir直接消费者确认Default Overlay缓存次序D153；.72–.79完成Foot Trace算法/primitive与mesh查询、骨盆和脚位置偏移/弹簧、足旋转/平滑/约束与加权写回、pole投影/状态/平滑，以及TwoBoneIK位置/旋转/部分权重写回，确认D159–D165；.80公共默认动作矩阵未自然触发D163–D165，未采样动作/历史和有效手臂IK仍未证明。完整图闭包及其他缓存副作用仍待重新取证核对，历史155子图合同只作起点，不能自证算法等价 |
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

- [x] BF144.04 CMC 地面查询、边缘与胶囊尺寸：沿fresh ALS ComputeFloorDist、UE FindFloor/ComputePerchResult/Capsule尺寸及target实际floor consumers核对sweep/line/retry、walkable/edge/perch、单位/世界缩放/父空间；Walking StepUp/Slide、蹲跳/基座分别留下一独立批次。Gate：source查询合同、实际几何最小反例与阴阳对照、漏检/排除记录。开始：2026-10-13 19:53 +08:00；预估结束：2026-10-13 20:48 +08:00；实际结束：2026-10-13 20:40:40 +08:00；偏差：−7分20秒；结果：审计记录Gate PASS，新增4项P2未修复差异。估时依据：ALS floor约170行、UE floor/perch/shape缩放递归闭包与target query约200行，加已有BF141父级尺度和BF134 floor/perch边界记录；几何组合比BF144.03多，单独限制为地面查询闭包。
  - 19:53 活动摘要：上一批速度/时钟审计已提交；当前累计7项P2，重力D08已按后续用户决定撤销。下一步重新定位源Capsule尺寸、sweep/line和perch分支，再沿真实floor调用链检查边界，不展开图/周边或性能。
  - 20:00 用户范围修订：默认重力统一使用Bevy值，D08从缺陷及有效失败反例移除，登记到附录A。原取证和当时分类记录保留，当前有效问题仅D01–D07；运动公式与时钟审计采用同一Bevy重力输入继续。本项地面查询审计不受该决定影响。
  - 20:15 source/focused 闭环：fresh CapsuleComponent确认正世界尺度的半高取竖轴、半径取水平两轴最小值，排除“应统一取三轴最小值”的猜测；B_Als_Character关闭flat-base查询。fresh AdjustFloorHeight的line guard与target数据丢失闭合，普通物理帧在floor consumer前采样仍为侧墙穿透，随后错误下移，确认D09。完整角色及最小运动夹具8组共6个对照PASS、2个语义FAIL。初始仅观察帧前位置的记录不足以区分物理阶段，未作最终证据；已保留实际消费者前采样。证据`target/bf144/04/line_final/`。下一步完成perch/边缘/正非均匀尺度合同，不展开步阶或图。
  - 20:22 source 闭包补充：IsWalkable读取ImpactNormal而非接触Normal；因此仅沿fresh UE CollisionConversions→Chaos Convex补查sweep面法线生成。正非均匀尺度继续区分CapsuleComponent查询尺寸与BodySetup/BodyInstance实际形体，不能把两套源语义混成一个缩放公式。凸体陡边查询已有候选反例，尚待完整角色消费者确认；未扩展场景资产或通用物理实现。
  - 20:24 source/focused 闭环：fresh Sweep hit转换无条件为正距离命中提取opposing face，Convex返回该面法线；完整角色在60°凸体陡边被target错误保留为Grounded，30°对照正确，确认D10。中心射线无命中、非穿透且impact在perch半径内，排除了line fallback与边缘支撑半径的歧义；最小查询4组为2PASS/2FAIL，完整角色2组为1PASS/1FAIL。证据`target/bf144/04/normal_query/`、`normal_loaded/`。
  - 20:31 source/focused 闭环：fresh Capsule→BodySetup/ChaosInterfaceUtils创建与BodyInstance缩放→PrimitiveComponent/WorldCollision实际movement sweep链确认物理胶囊保持圆截面并取水平大轴，区别于floor helper取小轴。target非均匀缩放生成椭圆近似凸体；完整角色沿未放大的水平轴撞墙比源允许位置多进入约0.30m，直接/父级缩放均复现，确认D11。6组实际角色中4个uniform对照PASS、2个nonuniform语义FAIL；证据`target/bf144/04/scale/`。本项累计新增3项P2，仍未改生产代码或运行公共Playground Gate。
  - 20:37 source/focused 闭环：14组perch半径/落差/地空/extra-height与16组平面坡度/查询尺寸对照全部PASS；fresh默认always-check=true、flat-base=false与0.15m perch阈值一致。沿floor consumer的owner时钟边界追加fresh PerformMovement→StartNewPhysics，确认D12：owner=0仍执行贴地高度修正，4组完整角色中仅未贴地的冻结输入FAIL。外部碰撞组件的CanStepUp/WalkableSlopeOverride及walking/landing的后续消费留下一碰撞批次，当前未把这些输入或未测几何计作等价。
  - 20:40 本项归纳：已核对默认floor sweep/retry/line、半径/坡度/有效perch/地空距离、源尺寸与实际形体差异、height adjustment及owner=0前置；6项focused测试为1PASS/5预期语义FAIL，内部54组中46PASS/8FAIL归并D09–D12。原116份source身份、11份新增UE依赖快照、B_Als_Character原始/Build资产与10个floor属性核对PASS；160份生产Rust及已编译库hash未变。`target/bf144/04/summary.json`只确认审计记录完整，不能作为运动等价或公共Playground验收。未修改生产设计，消融不适用；截图0、性能暂停。

- [x] BF144.05 CMC 步行碰撞、坡面与跨步：沿fresh ALS PhysWalking及UE MoveAlongFloor/ComputeGroundMovementDelta/StepUp/SlideAlongSurface/TwoWallAdjust，核对当前target前进、抬升、前向剩余位移、下扫、拒绝/回退和穿透恢复；只为这些算法读取实际源碰撞组件的CanStepUp/WalkableSlopeOverride输入。Air接触/着地、蹲跳和移动基座另列后续批次。Gate：source分支与实际消费者映射、独立几何反例/对照、差异及漏检记录。开始：2026-10-13 20:43 +08:00；预估结束：2026-10-13 21:38 +08:00；实际结束：2026-10-13 21:35:53 +08:00；偏差：−2分7秒；结果：审计记录Gate PASS，新增4项P2未修复差异。估时依据：已取fresh PhysWalking约300行，UE跨步/坡面/滑动递归闭包约600行，target grounded transaction约150行；BF144.04几何查询审计实耗47分40秒，本项将Air/蹲跳/基座拆出后独立估时。
  - 20:43 活动摘要：BF144.04已提交，当前累计11项P2，重力继续采用Bevy值。下一步核对源步行碰撞分支与target顺序；源组件属性只作为碰撞输入取证，不审计场景表现。截图关闭、性能暂停，生产实现保持未改。
  - 20:55 source/focused 闭环：fresh ComputeGroundMovementDelta/MoveAlongFloor先沿可行走坡面移动，再处理真正阻挡；target直接水平探测。20°坡/宽松顶面不能证明候选，在40°且平行顶面余隙1.2cm时，实际切向shape sweep无阻挡但90帧行进少0.9603m，完整角色与最小夹具一致，确认D13；12组中10PASS/2FAIL。fresh Playground native reader PASS且原始/Build关卡包一致，仅汇总碰撞输入：285个BodyInstance的WalkableSlope均Default，7个CMC保持水平速度；7个角色Capsule为ECB_No，普通场景碰撞组件为ECB_Yes。后者沿B_Als_Character→Character构造→CanStepUp继续核对角色间碰撞，未扩展AI或场景表现。
  - 21:04 source/focused 闭环：fresh Character构造和B_Als_Character都给Capsule设置ECB_No；target角色缺少对应权限组件且walking默认允许，确认D14。50ms帧的实际角色跨过uniform缩至0.2的被动角色；同几何普通prop可跨过，显式源No权限则停止，两个对照PASS。初始60Hz夹具连允许prop也未跨上，前置失败保留但不计有效反例。下一步沿fresh MovementComponent::SlideAlongSurface→TwoWallAdjust核对slide剩余位移是否真正sweep。
  - 21:20 source/focused 闭环：fresh基类SlideAlongSurface对每次剩余位移再sweep，target跨步被拒时却把投影直接交给多子步位置累加，确认D15。No权限及普通Yes权限但命中胶囊顶部两条拒绝路径都在0.1s/2子步撞双墙时失败，包含完整角色；24组中20PASS/4FAIL，单墙/单子步/可成功进入跨步路径的对照通过。初始“单墙全帧保持恒定切向速度”的oracle忽略了源子步碰撞后重新加速，已改为源几何不穿透合同；该错误oracle不计缺陷。另已由fresh ALS构造及CDO确认站立/蹲姿均允许离开边缘，未把UE基类默认蹲姿限制误记为差异。
  - 21:27 source/focused 闭环：穿透路径出现的实际结果不是预想的“floor丢失后立即切Air”，而是false-solid中心射线把实体底部出口当成walkable floor。沿fresh Chaos AABB→Raycasts确认源inside hit时间为0，ALS明确拒绝Time≤0的line fallback，确认D16；完整角色3组中穿透实体1FAIL、正常贴地/移除实体2PASS。仅记录已证实的floor有效性和消费分支差异，不宣称未测出的源最终脱离位置。
  - 21:35 本项归纳：已核对当前启用的ramp移动、StepUp抬升/前进/下扫/高度与权限拒绝、slide两墙、失去floor与穿透分支；组件坡度Default为恒等规则。0.18m/0.36m正常台阶、0.6m过高台阶和0.36m No权限对照PASS；原0.18m No权限预期“任何上升都被禁止”不成立，因为source FindFloor/AdjustFloorHeight不检查CanStepUp，不能充当阴性oracle，原输出保留。5项focused最终1PASS/4预期语义FAIL，46组中38PASS/8FAIL归并D13–D16；角色权限夹具从90帧缩至30帧后信号不变。fresh关卡292行运动相关属性及原始/Build身份、4份新增UE闭包快照、116份原source与160份生产Rust身份均PASS，库hash仍未变。证据`target/bf144/05/final/`、`05/summary.json`；无生产设计修改、无公共runtime Gate、截图0、性能暂停。下一批Air碰撞/着地，蹲跳/基座及动作消费者继续分项。

- [x] BF144.06 CMC 空中碰撞、偏转与落点资格：沿fresh UE PhysFalling/GetFallingLateralAcceleration/LimitAirControl/ComputeSlideVector/TwoWallAdjust/IsValidLandingSpot/ShouldCheckForValidLandingSpot，核对target airborne sweep、碰撞后速度、连续接触、胶囊下半球/边缘及实际世界坐标。无接触重力/时钟沿用BF144.03合同；ProcessLanded剩余时间与Character落地回调另列下一批，蹲跳/移动基座继续后置。Gate：source分支闭包、实际碰撞反例及对照、漏检/排除记录。开始：2026-10-13 21:43 +08:00；预估结束：2026-10-13 22:38 +08:00；实际结束：2026-10-13 22:11:55 +08:00；偏差：−26分05秒；结果：审计记录Gate PASS，新增5项P2与1项P3未修复差异。估时依据：fresh UE空中主循环及接触/落点闭包约650行，target falling sweep/transaction及资格consumer约300行；BF144.04/.05几何审计分别实耗47分40秒/52分53秒，本项把落地通知和剩余时间拆出后单独估时。
  - 21:43 活动摘要：BF144.05已提交，累计15项有效P2（D08仍为刻意重力差异）；当前只展开空中接触链。下一步核对源air-control在接触后的限制、slide/两墙及landing spot条件，并检查实际sweep使用的世界Transform；截图关闭、性能暂停、无生产改动。
  - 21:49 source/focused 闭环：fresh PhysFalling→LimitAirControl在陡坡接触后先去掉朝坡内的水平输入，target直接投影完整速度，确认D17。60°光滑坡面上的完整角色与最小运动夹具共8组，朝坡输入2FAIL，无输入/沿坡/离坡6PASS；目标下落速度−0.82923m/s而源应−0.87263m/s，碰撞阶段缓存和最终velocity一致。证据`target/bf144/06/air_control/`；继续核对实际世界姿态与连续接触/落点资格，不改生产实现。
  - 21:51 source/focused 闭环：fresh PhysFalling明确用UpdatedComponent的世界quat执行sweep；target仅把局部Transform的位置换成世界Position，保留局部rotation，确认D18。8组相同世界形体的直接/父级姿态对照中，2个60°父级反例FAIL，6个对照PASS；碰撞前缓存中心z=0.31142m，源形体界限0.81962m，后置solver仍未完全推出。证据`target/bf144/06/world_rotation/`；这是世界空间传参遗漏，与D11非均匀形体缩放独立。
  - 21:56 source/focused 闭环：fresh PhysFalling的两次接触/TwoWallAdjust之后还有ditch资格分支，target固定3次投影没有移植；两片60°陡坡夹角的实际双命中满足源两法线向上、互相dot<0且沿交线竖直位移为0，source当帧ProcessLanded而target停在Air，确认D19。完整/最小夹具各1FAIL，单坡2PASS；已沿SetPostLandedPhysics→OnMovementModeChanged确认零剩余时间不会立即重开PhysWalking，未把后续帧稳定性计作已验证。证据`target/bf144/06/ditch/`。
  - 21:59 source/focused 闭环：fresh UE默认`p.UseTargetVelocityOnImpact=1`及PhysFalling→MovementBaseUtility证明simulated body接触必须保留其法向速度；target只从slide位移重算velocity，确认D20。完整/最小角色撞到±1m/s移动动态壁的4组FAIL，静态、静止动态、同速运动学壁6组PASS；实际查询命中和consumer前body velocity均已捕获。证据`target/bf144/06/moving_body/`；移动基座的携带/离地继承仍留后续批次，本项仅为空中直接impact。
  - 22:02 source/focused 闭环：fresh SafeMoveUpdatedComponent默认不忽略初始blocking overlap，先MTD恢复再重试原位移；target空中只有cast/slide，后置solver未完成同帧脱离，确认D21。0.2m侧壁初始穿透的完整/最小夹具各2FAIL，墙外向内/向外4PASS；源膨胀胶囊在MTD+pullback目标处无阻挡，CDO允许5m最大恢复，排除恢复路径受阻。`06/air_inputs.json`同时确认Walking默认、air设置和impact physics参数，D20移动body用普通AddForce路径不立即改变接触速度；截图/性能未启动。
  - 22:05 source/focused 闭环：普通平地从0.5–10cm间隙落下，10组接触/无接触资格对照全部PASS，排除“仅靠近floor便提前着地”的猜测。fresh PhysFalling尾部另有厘米单位的微小水平速度清零，target缺失，完整角色0/0.1/0.2/0.4/1mm/s的5组中2FAIL，登记低优先级D22/P3；这是极小残余漂移，不与重力政策混淆。剩余核对静态墙/顶面/连续墙对照，然后收口本批；root-motion override和跳跃apex仍按动作/跳跃责任留后续项。
  - 22:11 本项归纳：已核对普通air输入/积分后的接触顺序、无输入及输入限制、static/simulated impact、初始穿透、slide/两墙/ditch、普通flat landing及源微小速度清零；墙、顶面、锐/钝角双墙8组对照PASS。最终8项测试2PASS/6预期语义FAIL，61组45PASS/16FAIL归并D17–D22；去掉输入、旋转父级、第二坡面、body运动或初始重叠时对应失败消失。116份原source、2份额外UE依赖、18个native运动属性与原始/Build资产、160份生产Rust及库hash核对PASS。证据`target/bf144/06/final/`、`06/summary.json`；只验收审计记录，无生产设计修改、无公共runtime Gate、截图0、性能暂停。罕见同面重撞nudge/virtual-ditch尚无独立触发证据，native继承CanSwim=True的适用边界留最终源缺口批次，均不计为已等价；落地剩余时间/回调、跳跃apex、root motion与基座沿既定责任批次继续。

- [x] BF144.07 落地剩余时间、回调与状态提交顺序：沿fresh ProcessLanded/SetPostLandedPhysics/OnMovementModeChanged及ALS NotifyLocomotionModeChanged，核对普通/滚翻/布娃娃落地仲裁、剩余Walking预算、保留输入/速度、制动timer和旋转阻塞。蹲起物理、Jump/apex、基座继承与Roll/Ragdoll内部动作实现各留对应批次；这里只追落地直接调用边界。Gate：source阶段/值/时钟合同、完整角色反例与对照、漏检/排除记录。开始：2026-10-13 22:14 +08:00；预估结束：2026-10-13 22:59 +08:00；实际结束：2026-10-13 22:39:41 +08:00；偏差：−19分19秒；结果：审计记录Gate PASS，新增1项P2，保留1项未闭合查询边界U01。估时依据：fresh CMC落地与模式切换约180行、ALS回调约65行、target falling transaction/landing friction/动作入口约300行；BF140.01同类只读复审实耗12分25秒、本次BF144.06完整几何审计实耗28分55秒，增加回调与timer跨阶段组合后独立估时。
  - 22:14 活动摘要：BF144.06已提交，累计20项P2和1项P3未修复差异；重力D08仍为刻意差异。本批已定位fresh落地回调在剩余StartNewPhysics前执行的顺序，下一步核对target同帧快照、实际状态与计时消费者；生产代码未改，截图/性能继续暂停。
  - 22:22 source/focused 闭环：fresh MovementComponent默认先于owner Tick，ALS落地回调读取上一轮Character已保存的LocomotionState.Velocity；target CMC仲裁也读该快照，但真正启动Roll却读本帧pre-impact recorder，确认D23。完整角色4组中快照/当前速度一致的2组PASS，3→8m/s与8→3m/s的2组都选错滚翻；fresh PerformMovement支持当前帧外力/launch在Character刷新前改变CMC速度，反例不是要求两份速度永远相同。证据`target/bf144/07/retained_velocity/`；继续落地剩余Walking与timer合同。
  - 22:34 验证修正与边界：源制动曲线在gait 1→2并非常数，首版remaining-time oracle误用了起步值，已按fresh Normal曲线修正；错误版本保留在`07/remaining_initial/`，不计缺陷。修正后4m平地的6组全部PASS，100m平地仍6FAIL，直接shape cast已比解析接触提前约0.6–0.9cm；这不是已证实的剩余时间算法缺陷。仅沿已有movement sweep→WorldCollision/GeometryQueries与target Avian/Parry查询依赖核对容差，不展开场景或补偿。角色倍率0/.5/1/2的落地timer在世界0.5秒严格大于边界到期均PASS。
  - 22:38 source/static 闭环：fresh B_Als_Character确认bTickBeforeOwner=True，CS_Als_Default确认700/1000cm/s落地阈值、自动Roll/Ragdoll开启、空中中断Roll关闭；目标默认设置相符。UE TimerManager以world delta累积，在物理后严格InternalTime>ExpireTime才执行，创建帧pending保留完整0.5秒，当前消费者与4组倍率控制一致。修正后的remaining公式使用gait映射、源curve线性段、pullback、CMC及braking两层子步；主合同夹具保留4m平地6组通过，不把100m箱体的查询数值差异消掉或改容差，另列U01待source查询实证收口。
  - 22:39 本项归纳：已核对源movement→Character→mesh及timer阶段，落地普通/Roll/Ragdoll直接仲裁、残余Walking预算、输入与速度快照、旋转阻塞和pending timer；角色蓝图未增加OnLanded/OnLocomotionModeChanged覆盖。最终3项focused为2PASS/1预期语义FAIL，14组12PASS/2FAIL仅归并D23；U01的另一组12项几何诊断独立保留，不计为合同通过。8个fresh native属性及原始/Build身份、116份原source、6份额外UE闭包与160份生产Rust/库身份PASS；证据`target/bf144/07/final/`、`07/summary.json`。未改生产设计，无公共runtime Gate，截图0、性能暂停；下一项只展开蹲起物理与生命周期，Jump/apex和基座继续分批。

- [x] BF144.08 蹲起胶囊、净空和生命周期：沿fresh Character ApplyDesiredStance/OnStartCrouch/OnEndCrouch与UE CMC Crouch/UnCrouch/UpdateCharacterStateBeforeMovement/AfterMovement，核对standing/crouched尺寸、源缩放语义、保持脚底、净空拒绝/重试、desired/actual时点和地空/动作直接条件。Jump/apex、移动基座与Roll/GetUp内部实现另列后续批次。Gate：native尺寸/配置与source分支合同、实际角色几何和生命周期反例/对照、遗漏及排除记录。开始：2026-10-13 22:42 +08:00；预估结束：2026-10-13 23:32 +08:00；实际结束：2026-10-13 23:01:24 +08:00；偏差：−30分36秒；结果：审计记录Gate PASS，新增4项P2，生产实现未改。估时依据：fresh UE crouch闭包约300行、ALS stance与通知约110行、target reshape/transaction约220行；BF141.02尺寸修复实耗70分45秒含构建/公共Gate，本次只读复审，结合BF144.04几何审计47分40秒与.07回调审计25分41秒独立估时。
  - 22:42 活动摘要：BF144.07已提交，累计21项P2、1项P3，另有U01待源查询实证。本批先核对默认CDO尺寸和源Crouch/UnCrouch实际单位/缩放，再建立头顶阻挡、脚底保持与地空条件对照；截图关闭、性能暂停、无生产修改。
  - 22:52 source/focused 闭环：fresh Crouch以实际世界quat扫掠下移缩短后的胶囊，UnCrouch则以gravity orientation做站起净空，target分别直接传送下移和误用actor quat，确认D24/D25。12组完整角色夹具8PASS/4FAIL，两个失败边界均可通过去掉倾斜或障碍消除。native站立半高90cm/半径30cm/蹲姿半高56cm与target一致；CDO的maintains-base为False，但ALS每次movement-mode change明确设True，不能把未初始化CDO当运行时合同。继续核对常规净空拒绝/重试、倍率和父空间对照，不修改生产实现。
  - 23:00 source/focused 闭环：普通headroom拒绝/恢复、0/.5/1/2 owner倍率、4种尺度×直接/父级center及mesh偏移共16组PASS，低顶板站起后swept floor adjustment也PASS。fresh源UPROPERTY/Setter允许配置蹲姿半高，Crouch明确clamp到radius且变大时先做净空拒绝；target的形体虽自行夹到radius，中心/mesh/cache仍用未夹值，且增高无拒绝，确认D26/D27。两者只在非默认尺寸触发，默认56cm对照通过；最终7项测试3PASS/4预期语义FAIL，37组31PASS/6FAIL归并本批4项差异。
  - 23:01 本项归纳：已核对Crouch/UnCrouch尺寸、gravity/actor query空间、最小轴center/mesh偏移、低顶板floor回退、desired/actual与Before/AfterMovement阶段和重复拒绝；源CanEverCrouch读取NavAgentProps=True，另一个MovementState=False不是该许可输入。D24最终改用4m平面，反例保持，避免把U01的100m查询数值带入新结论。native资产及116份原source、3份额外UE闭包、160份生产Rust/当前库身份PASS，证据`target/bf144/08/final/`、`08/summary.json`。去掉倾斜/阻挡或恢复默认尺寸均消除相应失败；无生产设计新增可消融，无公共runtime Gate、截图0、性能暂停。Roll/Mantle/GetUp的物理姿态override内部生命周期留各自动作/notify批次；下一批进入Jump/apex。

- [x] BF144.09 Jump接受、保持/松开、顶点与通知：沿fresh ALS Jump/OnJumped、UE CanJump/CheckJumpInput/ClearJumpInput/DoJump和PhysFalling apex分支，核对standing/action/mode许可、初始竖直速度、hold时钟、按键边沿、松开/重按、顶点子步与当前实际consumer。Mantle/Ragdoll跳键优先级只核对直接入口，内部动作与移动基座离地速度留后续批次。Gate：native跳跃参数/源阶段合同、独立公式与完整角色反例/对照、漏检及排除登记。开始：2026-10-13 23:03 +08:00；预估结束：2026-10-13 23:48 +08:00；实际结束：2026-10-13 23:21:26 +08:00；偏差：−26分34秒；结果：审计记录Gate PASS，新增1项P2，生产实现未改。估时依据：fresh Character跳跃约200行、CMC DoJump/apex约150行与target自定义Jump适配/运动约250行；BF144.08实耗19分24秒，BF141跳跃倍率修复及既有完整帧反例可复用，但apex与输入组合需独立推导。
  - 23:03 活动摘要：BF144.08已提交，累计25项P2、1项P3及U01。已定位源Jump的两代DoJump入口与apex substep默认开关，下一步先核对本机实际CVar/CDO，再建立取值和执行时点对照；截图关闭、性能暂停、生产实现未改。
  - 23:13 source/focused 闭环：fresh native确认JumpZ=4.2m/s、hold=0、count=1、gravity-while-jumping和DontFallBelowJumpZ=True、apex最多2次；本地源默认UseLegacyDoJump/ForceJumpPeakSubstep=1，项目/引擎配置无覆盖。已有竖直速度−2/0/2/4.2/8m/s起跳5组均保留源max合同。完整角色跨顶点帧遗漏先上后下的分段sweep，1.5mm头顶间隙在50/100ms帧被漏检，确认D28；无顶板3组及16.7ms直接上升撞顶对照PASS。首版误将只代表向上法线支持的airborne_sweep_contact当通用hit计数，已用source pullback+顶点分段位置公式替换，错误断言不计缺陷；继续hold/松开/通知边界。
  - 23:19 source/focused 闭环：0/.5/1/2 owner倍率×默认0/配置0.25秒hold×首帧松开/持续保持共16组、撞顶后hold继续/停止4组全PASS；已启用实际mesh求值的Standing/Crouching/pending-crouch/Air/Custom/held-only六组接受与动画frame-input对照PASS。fresh IA_Als_Jump reader PASS，资产明确是Pressed+Released、threshold0.5、pause不触发，与目标binding一致，排除持续按住自动连跳猜测。源码OnJumped→AnimationInstance.Jump→game-thread latch链已追到实际`source_frame_input.in_air_state.b_jumped`，本批不展开其后完整Air图求值合同。
  - 23:21 本项归纳：默认单次跳跃许可、蹲姿跳键先站起、初速度max、源hold/松开/owner时钟、ceil接触后继续hold与accepted→animation请求已闭合；最终5项focused为4PASS/1预期语义FAIL，37组35PASS/2FAIL仅归并D28。两份native资产、12个角色跳跃属性及Pressed/Released子对象、116份原source和3份新增闭包、160份生产Rust/库hash检查PASS；证据`target/bf144/09/final/`、`09/summary.json`。去掉顶板或避免跨越顶点时失败消失；无生产设计新增，无公共runtime Gate，截图0、性能暂停。移动基座按“跟随transform/阻挡”和“支持速度/离地继承”拆成独立后续批次；AI path-following与网络仍排除，动作内部及完整Air图按原顺序继续。

- [x] BF144.10 移动基座transform跟随、阻挡与保存阶段：沿fresh UE MaybeUpdateBasedMovement/UpdateBasedMovement/SaveBaseLocation、MovementBaseUtility及ALS RefreshMovementBase，核对移动/旋转基座的脚底锚点、刚体与组件阶段、世界/父空间、阻挡和基座切换/失效。支持速度、跳离/走离继承和JumpOff另列下一批；Mantle基座只保留接口事实，不展开动作内部。Gate：fresh native基座设置/source分支、完整角色几何/阶段反例与对照、漏检/排除登记。开始：2026-10-13 23:24 +08:00；预估结束：2026-10-14 00:09 +08:00；实际结束：2026-10-13 23:53:10 +08:00；偏差：−15分50秒；结果：审计记录Gate PASS，新增2项P2与1项P3，生产实现未改。估时依据：UE基座跟随/保存约280行、ALS刷新约40行、target follow/保存约130行；归档BF124.A01-S2b3源/focused实耗10分32秒，BF144.04几何批次47分40秒，本项把速度继承另拆后独立估时。
  - 23:24 活动摘要：BF144.09已提交，累计26项P2、1项P3与U01。已定位source跟随使用组件世界quat且默认忽略基座对actor旋转的改变；target跟随sweep传identity，仍是待实际触发确认的候选。下一步核对native旗标和平台在physics前后两阶段的位置差；截图/性能继续暂停。
  - 23:34 source/focused 闭环：fresh native确认IgnoreBaseRotation=True、FastAttachedMove=False、StayBasedInAir=False；源UpdateBasedMovement扫掠使用actor世界quat，target固定identity，确认D29。8组完整角色的直接/父级姿态×直立/60°×有墙/无墙中6PASS/2FAIL；斜胶囊安全中心z≤0.38038m，target携带到0.88891m，后置solver仅退到0.84728m。源默认BasedMovementMode=2允许simulated base在普通tick与post-physics两次更新/保存，不能把两阶段本身当成重复执行缺陷；继续核对暂停和缓存生命周期。
  - 23:42 source/focused 闭环：fresh native的角色/CMC TickEvenWhenPaused均False，UE LevelTick→TickTaskManager pause分支只执行显式允许暂停Tick的对象；target follow没有world-pause条件，确认D30。完整角色4组3PASS/1FAIL：world pause期间外部平移基座0.1m仍携带并保存；正常时间、owner倍率0，以及暂停且基座未动的对照通过。只沿基座跟随的执行门槛追加TickTaskManager源事实，不展开通用暂停或UI行为。
  - 23:45 控制组边界：18组pre/post-physics、yaw/小倾斜/平面scale与父空间跟随在1mm位置范围内满足foot-anchor/保存合同，基座切换和删除2组通过；但倾斜控制仍有约0.87mm额外抬升，不能据容差直接宣布完整位置等价。fresh ALS PhysWalking→UE AdjustFloorHeight只在floor距越出1.9–2.4cm带时补高；本轮仅沿这个已连接的后置floor consumer追加“移除平台、保持静止平面”的最小消融，判断残差是否属于基座跟随，未改生产代码或放宽阈值。
  - 23:51 source/消融闭环：移除平台后，3种有效离地距离的静止完整角色仍在30帧抬升0.43–3.46mm，源带内不调整，确认较小差异D31/P3。target默认float height固定在2.4cm、Tnua spring strength=400，对带内偏差仍施加竖直力；仅在夹具把spring strength设0后3组全无漂移，owner0另3组通过。18组基座transform控制在该数据消融下均通过收紧至0.1mm的检查，证明先前残差来自独立悬浮motor，而非基座公式；baseline失败及原1mm控制记录均保留，未修改生产或依赖。
  - 23:53 本项归纳：已闭合默认movable-base资格、忽略基座scale/保脚底、actor世界姿态、普通与post-physics保存、阻挡及切换/删除；CS默认不启用VelocityDirection基座旋转继承，未据未使用分支新增缺陷。最终5项focused为2PASS/3预期语义FAIL，41组35PASS/6FAIL归并D29–D31；本项所有生产源码和库hash未变，116份原source、4份额外UE闭包、native基座设置及Tnua实际依赖快照核对PASS，证据`target/bf144/10/final/`、`10/summary.json`。数据消融仅改变独立夹具的spring strength；每个baseline重建原配置，未触及生产源码/资产。无公共runtime Gate、截图0、性能暂停；基座支持速度与离地继承继续独立下一批。

- [x] BF144.11 基座支持速度与离地继承：沿fresh UE GetMovementBaseVelocity/GetMovementBaseTangentialVelocity/GetImpartedMovementBaseVelocity、OnMovementModeChanged与JumpOff/基座许可，核对component/body/bone速度来源、scaled脚底采样、地面相对速度、跳离/走离的线角速度提交和阶段时钟。继承的动画消费者按后续动画批次，Mantle/Ragdoll内部基座规则仍留动作批次。Gate：source选择/单位/阶段合同、实际角色速度反例及对照、遗漏和排除登记。开始：2026-10-13 23:56 +08:00；预估结束：2026-10-14 00:46 +08:00；实际结束：2026-10-14 00:21:09 +08:00；偏差：−24分51秒；结果：审计记录Gate PASS，新增5项P2，生产实现未改。估时依据：UE速度/模式切换与Pawn基座许可递归闭包约300行、target dynamics/accepted jump与基座快照约220行；归档BF124.A01-S2b3实耗10分32秒、本次BF144.10实耗29分10秒，本项增加scaled angular和跳离/走离组合后独立估时。
  - 23:56 活动摘要：BF144.10已提交，累计28项P2、2项P3及U01。fresh源码取脚底使用GetScaledCapsuleHalfHeight，target support helper减未缩放半高；先建立缩放/角速度与跳离反例，再核对component velocity优先级，当前仍为候选。截图关闭、性能暂停、生产实现未改。
  - 2026-10-14 00:12 source/focused 闭环：fresh线/角速度选择、Falling模式切换与native全轴继承旗标确认D32–D34；隔离角速度竖直分量后，scale×parent×轴12组10PASS/2FAIL，component/physical选择6组4PASS/2FAIL，平台竖直±2/0m/s跳离3组1PASS/2FAIL。另沿SetBase→BaseChange→Pawn根部CanStepUp许可确认D35：明确No的角色胶囊仍被站住，等形普通prop和显式Yes角色对照通过。当前24组17PASS/7FAIL，完整结果`target/bf144/11/jump_and_permissions/`；正在追加走离平台与跳跃检查/基座跟随的先后组合，未展开NPC AI或生产修复。
  - 00:16 source/focused 闭环：100ms帧先用50ms走出平台、再用剩余50ms下落，±2m/s平台竖直速度同样被丢掉，2FAIL/静止1PASS，D34扩展为两条离地入口。fresh ControlledCharacterMove先CheckJumpInput→Falling/clear-base，之后PerformMovement才MaybeUpdateBasedMovement；target先follow后接受Jump，确认D36。平台当帧外部平移0.2m并起跳时target多携带0.2m，owner1/0各1FAIL，不跳和平台不动2PASS。继续水平恒速/改速与自驱合成对照，然后收口本批。
  - 00:21 本项归纳：水平支持恒速、2↔−2m/s改速、与1/3.75m/s自驱合成、下一Air帧不重复继承6组全PASS；骨骼专用velocity/transform路径已定位，实际PA body/bone所有权留Ragdoll批次，不以普通box夹具代替。最终7项focused为1PASS/6预期语义FAIL，37组26PASS/11FAIL归并D32–D36；native全轴继承、JumpOff=0、角色ECB_No，以及116份原source/5份新增UE闭包/160份生产Rust和当前库身份PASS，证据`target/bf144/11/final/`、`11/summary.json`。移除scale、组件/physics分歧、竖直支持量、Pawn-No或同帧base位姿改变均消除各自失败，无生产设计新增，无公共runtime Gate、截图0、性能暂停。下一批仅沿已启用的CMC物理交互核对push/touch/standing-force/repulsion，不扩展物件表现或场景编辑。

- [x] BF144.12 CMC物理交互力与触发阶段：沿fresh CapsuleTouched/HandleImpact/ApplyImpactPhysicsForces/ApplyDownwardForce/ApplyRepulsionForce及native物理交互设置，核对普通角色胶囊对simulated body的touch、push、站立下压力和重叠排斥，区分查询命中、实际接触、质量/单位与owner/physics阶段。只审计运动消费者，不修改平台/物件/场景表现；Ragdoll内部物理和剩余特殊movement mode另按原闭包继续。Gate：native开关/标量与源力学合同、完整角色最小力/速度反例和对照、漏检/排除登记。开始：2026-10-14 00:25 +08:00；预估结束：2026-10-14 01:15 +08:00；实际结束：2026-10-14 00:42:00 +08:00；偏差：−33分；结果：审计记录Gate PASS，新增4项P2，生产实现未改。估时依据：本机UE四个交互入口及底层AddForce/Impulse闭包约350行，target普通胶囊/CMC/Avian消费者；归档BF124.A01-S2b3物理接触批次实耗10分32秒，BF144.10/.11含几何与阶段反例实耗29分10秒/25分09秒，加入当前UE力/冲量CVar分支后独立估时。
  - 00:25 活动摘要：BF144.11已提交，累计33项P2、2项P3与U01。fresh CMC Tick在普通运动后调用downward/repulsion，先核对本机native交互开关、Mass/force参数与当前UE的force-versus-impulse选择，再观察目标实际受力；当前未确认本批新差异。截图关闭、性能暂停，生产实现未改。
  - 00:36 source/static闭环：native PhysicsInteraction=True、Mass100、StandingScale1；本机UE静止body推挤默认为500×30力分支，非旧Impulse，source/project配置无覆盖。完整角色14组5PASS/9FAIL确认D37下压力及D38blocking impact施力遗漏，普通Avian接触未补足这两条路径；Static/Kinematic、零gravity等对照通过。下一步只核对同闭包的overlap touch/repulsion，生产源码/资产未改。
  - 00:42 闭环：完整角色5个focused测试1PASS/4个源合同语义FAIL，共29组13PASS/16FAIL；D39首次Touch以COM半径外排除repulsion，D40静止角色排除Touch，实际ContactGraph证明Overlap存在。3组显式提供同量级force的控制全部按质量/physics delta得到正确速度，排除物理积分通道失效；证据`target/bf144/12/final/`。native记录、116份source与补充Primitive/BodyInstance/Chaos源码身份、160份生产Rust及同一target_game库hash全部通过`check_phase12.py`，`12/record_gate.watchdog.json` PASS；测试非零101被记录为等价失败，未冒充runtime通过。
  - 本批累计37项P2、2项P3及U01。仅新增gitignored外部probe与审计记录，未新增生产抽象/状态/分支，设计消融无生产对象；物理力对照属于隔离归因。未运行真实Playground Gate，截图0、性能暂停；下一批核对源可达的ledge/恢复/模式边界，再转Roll/Mantle，未展开外围或未授权修复。

- [x] BF144.13 CMC剩余可达边缘行走与恢复合同：沿fresh ALS PhysWalking/PhysFalling调用及native CanWalkOffLedges/移动模式/约束设置，核对standing/crouching边缘许可、侧向ledge备选、失去floor和penetration恢复的未覆盖源分支；同时清点ALS实际调用的mode/外力入口，将动作所有权闭包交回Roll/Mantle/Ragdoll，未调用的通用UE接口不冒充ALS必需功能。Gate：本次native与源可达性表、必要完整角色反例/对照、未闭合边界登记。开始：2026-10-14 00:43 +08:00；预估结束：2026-10-14 01:33 +08:00；实际结束：2026-10-14 00:55:16 +08:00；偏差：−37分44秒；结果：审计记录Gate PASS，新增2项P2，生产实现未改。估时依据：剩余ALS walking循环及UE ledge/revert/checkfall/penetration约400行；BF144.04/.05几何批次实耗47分40秒/45分53秒，.12物理分支复用夹具实耗17分；本项只覆盖已选剩余入口，动作内部独立验收。
  - 00:43 活动摘要：BF144.12已提交，累计37项P2、2项P3与U01。先核对native边缘许可及源实际调用，尚未确认本批新差异；保留原始source/target身份，截图关闭、性能暂停，生产实现未改。
  - 00:53 source/static闭环：native CanWalkOffLedges与Crouching许可均True、ImprovedPenetration=True、AlwaysCheckFloor=True、ConstrainToPlane=False，排除把UE基类的蹲伏禁出边缘默认值当作ALS行为。完整角色12组7PASS/5FAIL确认D41无walkable floor时丢弃penetration命中，以及D42静止丢失floor时提前一帧积分重力；动作SetMovementMode/Locked仅位于Mantle和Ragdoll，其内部按后续批次，当前不修改。
  - 00:55 闭环：3个focused测试1PASS/2个源合同语义FAIL，共16组11PASS/5FAIL；standing/crouching×移动/静止的4组边缘许可对照PASS，离地当帧蹲伏保持也符合源仅清WantsCrouch、下一BeforeMovement才真正UnCrouch的阶段。`target/bf144/13/final/`、`check_phase13.py`与`13/record_gate.watchdog.json`确认native、116份源快照及MovementComponent补充、160份生产Rust、同target_game库hash未变，原始Playground/Build包相同。
  - 可达边界：fresh已加载Playground原始导出1013个object无PhysicsVolume/Water对象或bWaterVolume，native虽CanSwim=True但没有当前水体入口；Swimming/Flying、AI NavWalking和未被ALS调用的通用Launch/CMC外力API不计入本批通过覆盖。source ShouldCatchAir恒False，未据猜测增加飞出坡面算法；5处动作mode切换与锁归Mantle/Ragdoll。累计39项P2、2项P3、U01；无生产设计消融对象、无真实Playground验收，截图0、性能暂停，继续Roll闭包。

- [x] BF144.14 Roll触发、选择与动作生命周期：沿fresh StartRollingGrounded/IsRollingAllowedToStart/StartRollingImplementation、实际Input与CS配置、AM_Als_Roll段落/通知和UE montage play/stop，核对接收与拒绝、重入/尾段重启、动作tag/蹲伏/恢复、正常结束与中断、owner时钟。根位移碰撞、旋转和montage root提取另列下一批，Mantle/Ragdoll内部仍留其责任项。Gate：fresh native montage/notify/input事实与源阶段、完整角色状态/时间最小反例及对照，漏检和排除登记。开始：2026-10-14 01:00 +08:00；预估结束：2026-10-14 01:50 +08:00；实际结束：2026-10-14 01:24:54 +08:00；偏差：−25分06秒；结果：审计记录Gate PASS，新增2项P2，生产实现未改。估时依据：ALS Roll入口约115行、NotifyLocomotionAction/stance调用及UE montage lifecycle闭包，target roll state约160行与drive实际生命周期约200行；BF140.03动作时钟同类复审窗口20分钟，BF144.09完整Jump批次实耗18分26秒，另计native montage/notify递归读取；根运动空间/碰撞拆出以独立验收。
  - 01:00 活动摘要：BF144.13已提交，累计39项P2、2项P3及U01。CS原生指向AM_Als_Roll，先重新读取该montage及通知/输入来源，再验证Start/Stop阶段；本批尚无新确认差异。截图关闭、性能暂停、生产实现未改。
  - 01:07 source/static闭环：fresh AM_Als_Roll/A_Als_Roll/IA_Als_Roll三个reader PASS，原始/Build逐字节相同；montage长度1.5s、输入PlayRate1.3、action通知起始offset0.0001s及authored end0.933333s（UE GetEndTriggerTime复算有效end0.933433s）、CS CrouchOnStart=True。完整角色启动6组3PASS/3FAIL确认D43：常规启动胶囊蹲伏晚一帧，owner0时active roll始终没有立即设置Rolling tag。下一步核对接收/重入与正常结束/中断，根位移碰撞和旋转仍留下一批。
  - 01:21 source/static闭环：新增D44输入接受读取运动后mode，6组4PASS/2FAIL，分别为空中输入当帧落地被补接收，以及100ms地面走离帧原本有效Roll被拒绝；50ms走离对照通过，未将症状扩展到所有帧。3组owner自然结束、4组重入和4组Ragdoll直接Montage_Stop(0.2s)对照全部PASS，包含owner0冻结fade；Ragdoll只核对停止当前montage及替换action，未开展内部PA/动作审计。
  - 01:24 闭环：6个focused测试4PASS/2个源合同语义FAIL，共35组30PASS/5FAIL；另12组实际键盘/手柄Tap在world倍率0.25/2、保持40/160/400ms时与fresh源的未缩放0.3s阈值一致，释放触发后下一帧清除。自然结束覆盖owner0.5/1/2共250帧，Ragdoll直接停止覆盖4个owner的51个fade样本；`target/bf144/14/final/`。B角色没有SelectRollMontage override，CS→AM_Als_Roll→A_Als_Roll/PostLocomotion实际选择一致；未扩展CameraShake/Footstep效果。
  - `check_phase14.py`与`14/record_gate.watchdog.json` PASS：4份native记录/原始和Build资产一致，116份源、补充UE Montage/AnimInstance/AnimTypes与EnhancedInput调用、160份生产Rust及同target_game库hash均保持。初次记录脚本漏了Build/Content目录层已修正并保留失败报告，不影响反例。累计41项P2、2项P3与U01；无生产抽象/状态/分支新增，未跑真实Playground Gate，截图0、性能暂停。下一批只接Roll根运动的提取/位移/旋转/碰撞与模式语义。

- [x] BF144.15 Roll根运动、旋转与物理消费：沿fresh A_Als_Roll根轨道/Scale通知、UE montage root extraction/ConvertLocalRootMotionToWorld/CMC ApplyRootMotion及ALS RefreshRollingPhysics，核对完整角色每帧root增量、权重/比例、actor/mesh/parent空间、Grounded/Air/Custom处理、碰撞与own/world时间。生命周期D43/D44只作为已证实边界，不混入本批新差异；Mantle/Ragdoll仍独立。Gate：源root样本/公式与坐标推导、实际消费者几何/阶段反例及对照、记录闭环。开始：2026-10-14 01:28 +08:00；预估结束：2026-10-14 02:23 +08:00；实际结束：2026-10-14 02:09:11 +08:00；偏差：−13分49秒；结果：审计记录Gate PASS，新增6项P2、2项P3，生产实现未改。估时依据：Roll自身旋转约30行、UE根提取/CMC消费闭包约400行及target roll drive/root motion link，另需原始根轨道样本；BF144.14同动作生命周期实耗24分54秒、.05几何批次实耗45分53秒，按运动几何与空间闭包独立估时。
  - 01:28 活动摘要：BF144.14已提交，累计41项P2、2项P3与U01。AM/A/IA Roll的fresh原生记录可用，先确认现有uasset工具的根样本入口与完整消费顺序；本批尚无新确认差异，生产实现未改，截图0、性能暂停。
  - 01:39 source/static摘要：fresh原生A_Als_Roll有46个30Hz根样本，平移仅源Y、旋转恒identity、scale恒1；已沿ExtractRootMotionFromRange→montage累积→TickCharacterPose缩放→ConvertLocalRootMotionToWorld→CMC约束核对，并用实际角色采样验证。完整Air反例12组8PASS/4FAIL确认D45；Grounded样本与位移62帧中出现50ms首帧漏位移，仍在核对物理消费，未计为已确认。原生SequencePoseProbe已成功，继续空间/碰撞和阶段边界；累计42项P2、2项P3，生产未改。
  - 01:42 source/static摘要：原生SequencePoseProbe十个时点确认源根轨道与目标采样一致。对首帧位移按16.667/33.333/34/40/50/51/60/100ms细分，8组5PASS/3FAIL确认D46：33.333–50ms间仍消费Roll开始前的零walking budget，51ms以上因多子步清掉该budget而恢复。累计43项P2、2项P3，下一步核对root空间、动作通知结束后的旋转和碰撞路径，不改生产实现。
  - 01:50 source/static摘要：fresh AB_Als主实例RootMotionFromMontagesOnly、CMC两项默认转向关闭且AllowPhysicsRotationDuringAnimRootMotion开启，排除源有第二次转向。Custom的parent/yaw/tilt/world scale×owner16组通过；实际Grounded转向9组6PASS/3FAIL确认D47，动作结束门槛4组1PASS/3FAIL确认D48。当前6tests为1PASS/5预期语义FAIL，111组97PASS/14FAIL，源根采样全部匹配；累计45项P2、2项P3。继续碰撞/落地和根运动尾段的最小闭包，未运行公开Playground Gate。
  - 01:58 source/static摘要：Grounded/Air/Custom×16.667/50/100ms×Block/Overlap的18组阻挡语义对照通过，只证明模式和阻挡关系，不把数毫米的命中数值等同源查询。空中零初速度的12组位置合同9PASS/3FAIL确认D49/P3，源midpoint与目标短帧积分不同；自然blend tail的6组4PASS/2FAIL确认D50/P2，源root结束后应恢复的正常Aiming rotation仍被active动画尾段阻挡。累计46项P2、3项P3；继续核对owner最小tick与落地消费后收束本批。
  - 02:05 source/static摘要：owner低于源1µs运动门槛的8组7PASS/1FAIL确认D51/P3；Grounded首个微小root仍移动约0.834µm，Custom门槛正确。近地/远地×有无Roll×16.667/100ms的8组落地mode对照通过；其中Roll实际落地后的高度及残余运动尚需独立验证，不能以mode通过代替。累计46项P2、4项P3，当前生产文件和原始资产均未改。
  - 02:07 source/static摘要：fresh ProcessLanded→SetPostLandedPhysics→OnMovementModeChanged先调整floor、再StartNewPhysics消耗剩余时间；target完整帧Root路径却给falling sweep传landing_cmc=None，仅末尾改mode。6组2PASS/4FAIL确认D52：50/100ms落地后离地仅约0.02–0.14mm，普通落地为21.499mm，夹具关闭solver仍未补上高度。横向偏移只保留作定位数据，尚未用native查询确认其具体数值来源，不另记第二缺陷。当前12tests为3PASS/9预期语义FAIL，169组145PASS/24FAIL，累计47项P2、4项P3；本批进入身份/记录核对。
  - 02:09 记录Gate：`target/bf144/check_phase15.py`与`15/record_gate.watchdog.json` PASS；160份target源码、116份基线source和实际launcher库身份保持，新增UE root/mesh/montage/math及Tnua/Avian积分源码快照核对一致。native主AB与Roll根样本均已读取且原始/Build一致；`15/summary.json`登记12tests、169组与D45–D52，语义失败未伪称通过。夹具消融仅作用于fresh App状态/solver，未修改生产源码；早期旋转oracle常数与编译夹具错误已按当前源修正，最终证据不引用错误结果。未运行公开Playground Gate，截图0、性能暂停；下一批只接Mantle探测/许可/选择和设置，root source推进另列依赖项。

- [x] BF144.16 Mantle探测、许可与设置选择：沿fresh StartMantling/AutoStartMantling、CS Mantling trace配置与角色蓝图SelectMantlingSettings，核对方向/输入角限制、胶囊及sphere尺寸/单位、forward/downward/两次overlap、目标速度/step权限、地空与高低判定、Overlay分支及实际选择资产。只读相关动作入口；Mantle RMS推进、平台跟随与结束交接另列依赖批次。Gate：源trace数据与公式、native选择图/资产、实际完整角色和精确query夹具的正反例、漏检与排除记录。开始：2026-10-14 02:11 +08:00；预估结束：2026-10-14 03:06 +08:00；实际结束：2026-10-14 02:48:37 +08:00；偏差：−17分23秒；结果：审计记录Gate PASS，新增2项P2，生产实现未改。估时依据：本地StartMantling trace约250行、target detect_mantle约200行及设置选择图；BF144.05几何批次实耗45分53秒、.15动作根运动41分11秒，考虑递归设置/蒙太奇读取后保持独立验收窗口。
  - 02:11 活动摘要：BF144.15已提交，累计47项P2、4项P3及U01。fresh CPP确认Mantle按速度/输入双重限制方向，再依次forward capsule、downward sphere、target capsule overlap、start capsule overlap；原生CS及角色选择图已有当次reader记录，先提取所需字段和递归设置资产。本批尚无新确认差异，生产未改，截图0、性能暂停。
  - 02:42 source/focused闭环：fresh角色选择图与7份Mantle设置reader PASS；完整角色及同一query确认D53子碰撞体速度身份遗漏、D54箱体边角法线导致目标/朝向偏移。3个focused为1PASS/2语义FAIL，共310组306PASS/4FAIL；方向限制300组含半转边界全部PASS。证据`target/bf144/16/initial/`。首个成功native读取因临时外层脚本误判空LASTEXITCODE而中止，已改读机器报告后续读其余6份，成功记录未重复执行；不计为源或产品失败。继续核对6份Montage、设置标量与探测几何控制，RMS内部仍留下一批。
  - 02:48 本项归纳：6个focused为4PASS/2语义FAIL，共420组416PASS/4FAIL，失败只归并D53/D54。fresh 7设置→6Montage引用、13条角色选择路由及起播/warp标量均一致；ground/air×standing/crouching×uniform scale0.5/1/2的12组query形状与目标高度、手动/自动×allow/step的8组完整入口控制通过。证据`target/bf144/16/final/`、`16/inputs.json`、`16/summary.json`；`check_phase16.py`及record_gate watchdog PASS，15份native/原始Build一致，116份source、7份补充UE源码、160份target与实际库hash未变。
  - 本批累计49项P2、4项P3及U01。移除父子速度分歧或边角接触分别消除失败；未新增生产设计，探测数值的完全等价仍未闭合，不能用以上有限控制宣称100%。本次没有公共Playground Gate，截图0、性能暂停。下一批沿已选Mantle启动/停止入口核对RMS安装、碰撞与movement mode的生命周期；RMS根轨道/平台空间数学单独验收。

- [x] BF144.17 Mantle启动、结束与运动模式交接：沿fresh StartMantlingImplementation/RefreshMantling/StopMantling和角色OnMantlingStarted/Ended，核对RMS安装/移除、action与montage生命周期、Custom锁/基座/碰撞与输入许可、自然结束/外部中断/目标销毁的Walking或Ragdoll直接交接。只审计动作边界；RMS根轨道、warp数学与平台空间推进单独列后续依赖批次，Ragdoll内部不提前展开。Gate：native通知/角色回调与CPP阶段合同、完整角色时间线及必要反例/对照、漏检/排除记录。开始：2026-10-14 02:51 +08:00；预估结束：2026-10-14 03:41 +08:00；实际结束：2026-10-14 03:37:28 +08:00；偏差：−3分32秒；结果：审计记录Gate PASS，新增2项P2、1项P3，生产实现未改。估时依据：已定位ALS启动/结束约210行、RMS同步入口及target prepare/pose/release约360行；BF144.14动作生命周期实耗31分左右、.15根消费41分11秒、.16探测37分37秒，分离完整root/warp数值后保持独立验收窗口。
  - 02:51 活动摘要：BF144.16记录Gate已提交，累计49项P2、4项P3及U01。fresh CPP在启动时先Custom+mode lock+base再装RMS并播放Montage；Refresh按action、mode和目标寿命决定Stop，Stop解锁后切Walking。下一步提取角色回调与6份已读取Montage通知，只沿实际动作消费者核对；本批尚无新差异，生产未改、截图0、性能暂停。
  - 03:20 source/focused闭环：fresh native bTickBeforeOwner=True与UE tick依赖证明CMC→Actor→Mesh；3个实际Mantle Sequence均关闭动画root motion，6个Montage的通知及角色回调已提取，OnMantlingStarted/Ended只处理Overlay道具，不展开其实现。确认D55物理阶段胶囊不可查询、D56自动Mantle提前推进首帧RMS；2个focused共6组3PASS/3语义FAIL，证据`target/bf144/17/initial/`。先前“结束前多走一帧”的候选因源Tick顺序尚不能成立，不登记；继续核对自然通知退出、锁定请求和目标销毁。临时夹具曾对immutable CollisionLayers请求可变Query导致编译失败，已改为Commands替换组件，生产代码未改。
  - 03:35 source/focused闭环：自然low/high×owner0.5/1/2的通知退出6组、mode lock/两种Ragdoll交接5组、目标销毁×owner0/1×world pause及恢复6组PASS。主动Ragdoll打断的源mode仍为Walking：旧锁先拦None，再由StopMantling解锁切Walking；初版夹具误期望None，已按fresh源码纠正，不计缺陷。另确认D57新Aiming的EarlyBlendOut晚一帧；12组条件控制仅两种高度的新Aiming失败，预先提供Aiming状态、input/crouch/air/idle全部通过。通知end按fresh AnimTypes GetEndTriggerTime补入trigger offset，manifest数据一致；未修改转换器或资产。
  - 03:37 本项归纳：最终6个focused为3PASS/3语义FAIL，共35组30PASS/5FAIL，归并D55–D57；`17/final/`保留完整结果，`capture_mantle_lifecycle_inputs.py`及`check_phase17.py`/record_gate watchdog PASS。18份native资产、原始/Build一致，116份基线source、6份补充UE源码、160份target及同一launcher库hash未变；`17/inputs.json`记录6Montage/3Sequence与准确通知end，`17/summary.json`区分记录通过与等价失败。
  - 本批累计51项P2、5项P3及U01。普通角色/仅恢复query layers、owner0、预先提供Aiming状态分别消除确认失败；无生产设计新增，不用源自身的Ragdoll锁行为制造新缺陷。尚未验收完整RMS root/warp/平台空间数值；无公共Playground Gate，截图0、性能暂停，继续下一独立闭包。

- [x] BF144.18 Mantle根轨道与warp变换数学：沿fresh AlsMontageUtility、StartMantlingImplementation和RMS PrepareRootMotion提取3个实际Sequence的root事实，核对slot/segment时间、mesh→actor坐标与厘米单位、起止root消除、blend/warp插值、rotation twist及静态目标/缩放/父空间下的绝对角色目标。动态基座的逐阶段跟随、owner/world时钟与速度/物理消费另列依赖批次。Gate：native root采样及源变换推导、生产函数与完整角色的最小数值反例/控制、剩余边界登记。开始：2026-10-14 03:40 +08:00；预估结束：2026-10-14 04:35 +08:00；实际结束：2026-10-14 04:04:06 +08:00；偏差：−30分54秒；结果：审计记录Gate PASS，新增1项P2，未修复。估时依据：本地root utility约65行、RMS核心约100行及起止设置约65行，target state/drive约250行、3个Sequence；BF144.15根轨道/时钟消费实耗41分11秒、.17生命周期46分28秒，本项将动态基座和物理时钟移入下一独立验收闭包。
  - 03:40 活动摘要：BF144.17已提交，累计51项P2、5项P3及U01。3个原生Sequence与6Montage引用已按当次读取确认；下一步只补root轨道/采样及坐标变换oracle，先核对source实际提取API，不按现有target算法自证。当前本批无新增确认差异，生产未改、截图0、性能暂停。
  - 03:49 source/static闭环：6个Montage均为PostLocomotion单slot、单segment，源构造默认segment起点0、play rate1、loop1，3条原生root均为identity旋转/单位scale；blend与warp设置已重新核对。fresh `ExtractRootTrackTransform_Lockless`证明RMS实际按压缩数据有效性选择采样，现有reader强制raw不能单独充当最终数值oracle；仅给既有只读SequencePoseProbe增加`root_track_only`选项，直接调用同一源API，正在重编取证。生产Rust/资产不变；候选仅为初始actor scale/完整rotation信息丢失，尚未确认，动态基座不在本项展开。
  - 04:02 source/focused闭环：原生root API共352采样与对应轨道插值匹配，High montage rate1.2、Low各variant rate1.0；初版High oracle误用1.0已纠正，不计缺陷。独立源VQS/AlphaBlend/FastLerp/twist对照与7种实际动作轨迹PASS；确认D58初始actor Transform信息丢失。12组完整角色仅非单位scale/非零倾斜的5组失败，夹具补回丢失信息后恢复；目标/actor父空间和静态基座4组通过。
  - 04:04 本项归纳：最终5个focused为3PASS/2语义FAIL，317组312PASS/5FAIL，归并D58；其中294组验证blend/相对基坐标数学，7种动作168帧与4种父空间72帧比较实际world位置/rotation。`18/final/`、`18/summary.json`及record_gate watchdog PASS；18份native资产、116份基线source、10份补充UE文件、160份生产Rust和同一launcher库hash核验通过。
  - reader改动限于既有probe的一个只读选项，同目录重编PASS；消融移除请求选项会回到旧raw API/79骨架输出，被root-track取证类型检查拒绝，恢复选项后3采样PASS。旧raw路径兼容检查PASS且本次两种路径root值相同，不伪称已有数值差；保留选项只为提供源实际API证据，无新转换规则、生产状态或运行时分支。本批累计52项P2、5项P3及U01；无公共Playground Gate，截图0、性能暂停。

- [x] BF144.19 Mantle动态基座、RMS时钟与物理消费：沿fresh UE MaybeUpdateBasedMovement/UpdateBasedMovement、RootMotionSourceGroup准备/结束与ALS PhysCustom，核对移动/旋转/缩放基座的采样阶段、owner/world/最小delta、RMS速度和位置提交、暂停/恢复及退出后的残留速度。源基座运动仅作为角色输入，不扩展平台或通用物理设计；Ragdoll内部仍待其独立批次。Gate：逐阶段source合同、独立root oracle与完整角色数值反例/控制、记录闭环。开始：2026-10-14 04:06 +08:00；预估结束：2026-10-14 04:59 +08:00；实际结束：2026-10-14 04:37:53 +08:00；偏差：−21分07秒；结果：审计记录Gate PASS，新增2项P2、3项P3，D61误判已撤销，生产未修复。估时依据：UE基座和RMS有效调用闭包约350行、target mantle drive/基座回写及physics timing约250行；BF144.11基座速度实耗25分09秒、.15根运动消费41分11秒、.18变换24分06秒，本项增加物理基座与owner零时钟的跨阶段组合，根轨道复用刚完成的源API证据。
  - 04:06 活动摘要：BF144.18已提交，累计52项P2、5项P3及U01；source CMC在RMS准备前调用MaybeUpdateBasedMovement，target在物理前计算RMS、物理后另做基座绝对回写，需比较两阶段实际actor和velocity。当前只有候选，没有本批新确认差异；生产实现未改，截图0、性能暂停。
  - 04:13 source/focused闭环：fresh CMC基座携带在RMS之前、物理基座再于PostPhysics携带，native bIgnoreBaseRotation=True且SaveBaseLocation写absolute rotation；物理后携带用脚底、忽略scale。确认D59基座位移混入RMS/实际动画速度，D60物理后重做完整warp导致朝向及位置变化；两个focused共15组6PASS/9语义FAIL。无移动基座、静止/纯平移的post阶段对照通过；只在夹具提前完成source携带即恢复D59。继续当前RMS时钟/暂停/退出边界，未展开其他动画consumer。
  - 04:30 source/focused闭环：普通owner/world组合10组、世界暂停/恢复2组、跨通知/整个RMS duration的6组PASS。D60新增静态基座下绕过1µs位置门槛的反例；无支撑中断提前InAir确认D62，实际graph tag已取样。D61原“Clear置零速度”oracle经fresh底层结构复核撤销，修正后3组保留速度PASS；沿同一旧transform getter继续核对旋转，源在owner0/极小delta仍再应用旧rotation delta，目标保持绝对角，新增D63。当前9个focused为4PASS/5语义FAIL；没有因源行为反直觉而优化或修复它。
  - 04:32 剩余边界：source RMS每次Prepare都经`UseRelativeLocation→IsPhysicsOwnerMovable`读当前Mobility，target刷新函数保留开始时relative_location；只补这个已有source分支的目标存活期间模式变化对照，不展开平台实现。其余时钟、根速度和退出分支已形成记录，尚未做公共Playground验收。
  - 04:38 本项归纳：9项core focused为4PASS/5语义FAIL，新增mobility focused为0PASS/1语义FAIL，合计55组39PASS/16FAIL，归并D59/D60/D62/D63/D64；D61排除后的速度对照3组PASS。证据`19/core_final/`、`19/mobility/`、`19/final/`与`19/summary.json`；`check_phase19.py`/record_gate watchdog PASS。116份基线source、11份补充UE source、18份native资产及160份生产Rust/同一launcher库hash不变。
  - 消融仅在fresh App中提前携带actor、或保持基座变换/初始朝向/正常delta，分别区分各项失败；未添加生产设计或修改阈值。基座字段、实际graph速度及模式tag已沿直接consumer核对，本批累计54项P2、8项P3，D08/D61撤销、U01仍未闭合。RMS duration的source epsilon只作到期标志推过边界，当前原生root尾段已恒定，6组末帧/退出控制未见新增位置或模式差异；未扩展到网络/相机/平台实现。截图0、性能暂停，继续Ragdoll/GetUp。

- [x] BF144.20 Ragdoll启动、退出与GetUp交接生命周期：沿fresh Start/StopRagdollingImplementation、许可及本地输入/蓝图回调，核对所需physics body、action/movement lock、capsule/mesh物理与查询启停、montage中断、Grounded/InAir退出和GetUp输入阻塞的实际提交顺序。只提取本项需要的SKM/PA身份与body存在性；刚体配置/驱动数值、pelvis跟随/出口变换与snapshot以及GetUp完整动画分别留后续独立闭包。Gate：fresh配置及源阶段合同、完整角色最小反例/对照、实际消费者与记录闭环。开始：2026-10-14 04:42 +08:00；预估结束：2026-10-14 05:32 +08:00；实际结束：2026-10-14 05:34:26 +08:00；偏差：+2分26秒；结果：审计记录Gate PASS，新增4项P2、1项P3，未修复。估时依据：ALS启动/退出/许可约280行、target state transition/ragdoll link/GetUp入口约400行，需SKM/PA与2份GetUp Montage原生依赖；BF144.17动作生命周期实耗46分28秒、.19阶段消费31分53秒，本项按独立生命周期边界估时，物理和姿态数值不在此项合并。
  - 04:42 活动摘要：BF144.19已提交，累计54项P2、8项P3，D08/D61撤销、U01未闭合。fresh源码要求pelvis/spine_03 body存在才启动；先停Montage、detach mesh并启用物理，再锁movement与发布Ragdolling。Stop先snapshot/恢复查询和attach，再按ground trace路由Walking/Falling，成功播放GetUp后才封输入并发布GettingUp。下一步取所需原生资产及实际target入口，不凭历史PA或Ragdoll Gate宣称等价。
  - 05:01 source/focused闭环：fresh SKM_Als实际引用PA_Als，21个body中root为Kinematic且禁碰撞，其余为Default；两份GetUp Montage与CS引用已取出。22份native原始/Build身份及reader报告通过。确认D65必需刚体缺失仍启动、D66动画实例缺失仍退出；6组3PASS/3语义FAIL，保留`20/initial/`。当次Jump/Ragdoll/direct出口3组同帧GettingUp且输入阻塞；后续须核对调度约束，单次通过不证明顺序合同。继续正常启动/锁定/重复请求和None→GettingUp交接时ApplyDesiredStance这一直接依赖，不复开普通Crouch条目。
  - 05:21 source/focused闭环：未约束顺序的后续完整App复现Jump晚帧与恢复站姿失败；用公开schedule API仅固定当前允许的两种顺序，Jump 6组5PASS/1FAIL确认D67。姿态交接16组11PASS/5FAIL，分离D68瞬时None未锁存desired stance与D69自身rig阻挡站立；实际同一净空查询仅命中本角色pelvis/spine/thigh，先清rig layers后为空。正常/roll/GetUp/Mantle进入、3类mode锁和重复启动共8组PASS，物理body identity保持、20个Dynamic/1个Kinematic及capsule停用符合native。剩余只补ground/air出口和GetUp播放失败控制；PA参数与姿态数值仍留后续项。
  - 05:34 本项归纳：6个core focused为2PASS/4语义FAIL，追加GetUp interruption focused PASS，合计42组33PASS/9FAIL，归并D65–D69；normal/air×GetUp clip可用性4组的模式、输入锁、capsule/rig停启和父子关系恢复通过。GetUp再入Ragdoll的owner0/1两组按源0.2s Cubic停止，0.05s权重1/0.84375、恢复后实例按期结束，Ragdoll期间保留原输入锁；源Stop不会立即终止branching notify，已读底层确认。
  - 证据`20/core_final/`、`20/stop_final/`、`20/final/`及`20/summary.json`；`capture_ragdoll_lifecycle_inputs.py`与`check_phase20.py`/record_gate watchdog PASS，116份基线source、5份补充UE source、22份native与160份生产Rust及同一launcher库hash核验。调度消融仅固定原本允许的顺序，反向顺序消除对应失败；缺非必需body和完整动画实例是许可对照，无生产设计/转换器修改。本批累计58项P2、9项P3，D08/D61撤销、U01未闭合；截图0、性能暂停，PA数值及snapshot/GetUp完整动画继续后续闭包。

- [x] BF144.21 PA刚体配置、形状/质量与有效碰撞：沿fresh PA_Als/SKM/B_Als_Character及UE InitArticulated/BodyInstance的实际消费，核对21个body、22个shape、质量/惯量与材质数据、component/body/shape碰撞合成及49个排除对；以真实加载角色验证target应用。关节约束/驱动、阻尼/睡眠/CCD的数值积分和Ragdoll启动速度/跟随各留下一独立批次，本项不扩展通用物理或场景内容。Gate：native原始身份、source单位/覆盖合同、完整角色参数/碰撞最小反例与对照、漏检及未闭合边界。开始：2026-10-14 05:43 +08:00；预估结束：2026-10-14 06:33 +08:00；实际结束：2026-10-14 06:18:34 +08:00；偏差：−14分26秒；结果：审计记录Gate PASS，新增1项P2、4项P3，移植实现未修复。估时依据：已提取PA的21 body/22 shape/20 joint/49 collision pair，当前只覆盖body与shape；source BodyInstance/InitArticulated及target profile应用约450行，BF144.18变换闭包实耗24分06秒、.20完整角色生命周期52分26秒，按独立数据/碰撞边界估时；登记时点按实际工具时钟校正，估时窗口不变。
  - 05:43 活动摘要：BF144.20已提交，累计58项P2、9项P3，D08/D61撤销、U01未闭合。fresh PA各body默认QueryAndPhysics/PhysicsBody，但B_Als mesh组件保留CharacterMesh response override；必须先推导UE实际合成再评价target层位。质量/形状先核对原生数据到当前profile和实际body，不能把转换器自检当作独立源证明。截图0、性能暂停，未修复生产实现。
  - 05:49 source/focused闭环：fresh BodyInstance确认PA Block与mesh response取交集，Ragdoll只改变ObjectType/CollisionEnabled而未清Pawn Ignore；完整双角色focused 2组1PASS/1语义FAIL确认D70。有效physical material沿GetMaterial(0)递归到MI_Als_Character→M_Als_Main，两者PhysMaterial=None，最终为DefaultPhysicalMaterial；仅读取物理材质引用，不展开材质表现。继续形状、质量/惯量和49个排除对，关节/积分仍留后续。
  - 05:59 source边界补证：22个source形状/21个body字段及230个跨body shape pair通过，105组质量矩阵初检发现小尺寸差异与默认head的f32分解偏差；尚未按缺陷计数。UE初始GetFinalScaled与运行时UpdateBodyScale的capsule下限不同，新增reader可选PhysicsBodyProbe直接调用原生InitBody/UpdateBodyScale/UpdateMassProperties及质量getters，限定PA数据取证、不改移植实现/转换规则。首轮无scene初始化保留了默认1kg，已据InitDynamicProperties路径排除该无效结果，补显式原生质量刷新后重新取样；不拿公式或utility CalculateMass替代真实body值。
  - 06:14 source/focused闭环：原生126组initial/84组live质量取样完成，常规尺寸105组独立公式与native一致，小尺寸4个body的源分解数值偏差按native原值保留；普通asset导出与此前PA逐字段相同。完整target矩阵126组102PASS/24FAIL，定位D71惯量分解、D72初始小尺寸质量重算；native frame控制10组5PASS/5FAIL，5个控制全部恢复。live pelvis几何4组3PASS/1FAIL定位D73，零mesh scale 2组1PASS/1FAIL定位D74；source OnUpdateTransform调用/默认CVar及无项目覆盖已核对。继续收尾证据与reader消融，不进入关节或驱动积分。

  - 06:18 本项归纳：7个focused为2PASS/5语义FAIL，419组387PASS/32FAIL归并D70–D74；126组native initial与84组native live值已独立保存。初始小pelvis从实际截断shape重算恢复native惯量，native principal/frame控制恢复5个body样本，未放宽容差。两份physical material依赖只用于质量/摩擦解析；未扩展材质图。
  - 证据21/final/、21/native_mass_summary.json及21/summary.json；capture/native_mass/record_gate watchdog PASS，116份基线source、13份补充UE source、24份native、160份生产Rust及同一launcher库身份核验。reader增量在相同package target重建且源码字节一致；缺质量刷新版本的1kg结果被拒绝，当前原生质量恢复正确，普通PA导出逐字段不变。移除未使用的公式Rust输出后7项focused结论不变；未修改转换规则/target资产或移植算法。累计59项P2、13项P3，D08/D61撤销、U01仍未闭合；截图0、性能暂停。下一批先核对PA关节与ALS驱动直接闭包，body积分/零scale恢复、pelvis跟随与GetUp继续独立分批。

- [x] BF144.22 PA关节配置、坐标框架与ALS角驱动输入：沿fresh PA_Als、UE InitConstraint/UpdateRBJointMotors/SetAllMotorsAngularDriveParams及ALS RefreshRagdolling，核对20个关节的骨骼绑定、局部frame、线性/角限位、drive启用/目标/单位与速度缩放，并检查完整角色的实际joint组件及每帧输入。约束数值求解、body阻尼/睡眠/CCD积分、启动速度与pelvis跟随/出口pose继续按独立依赖批次验收。Gate：native源身份与配置合同、实际组件/消费者最小反例及对照、漏检和未闭合边界。开始：2026-10-14 06:23 +08:00；预估结束：2026-10-14 07:08 +08:00；实际结束：2026-10-14 07:08:58 +08:00；偏差：+58秒；结果：审计记录Gate PASS，新增1项P2，未修复。估时依据：PA已提取20个constraint，fresh ALS驱动段约15行，UE约束初始化和动画motor闭包约400行，target profile与角驱动约350行；BF144.18变换闭包实耗24分06秒、.21含原生reader扩展35分34秒，本项将数值求解拆出后独立估时。
  - 06:23 活动摘要：BF144.21记录Gate已提交；累计59项P2、13项P3，D08/D61撤销、U01未闭合。fresh ALS以pelvis速度除10m/s再clamp缩放25000源角刚度，阻尼与force limit均设0；仍需核对UE设置覆盖、动画目标与实际solver输入。截图0、性能暂停，未修复移植实现。
  - 07:08 source/focused闭环：fresh PA reader与原始/Build身份PASS；20 joints在unit/0.75/正nonuniform下60组骨骼绑定、双frame、限位与有效开关全部PASS。19活动joint按当前pre-physics Pose局部旋转和native ref frame独立复算，两种尺度×4帧共152组PASS；这只验证motor目标计算，不提前宣称源flail动画输入等价。首次四元数比较把q与−q误判为2π，已统一最短旋转并保留原记录，未登记缺陷。8组速度输入中6PASS/2FAIL确认D75：目标先限速再计算刚度，源顺序相反；下一步收口live joint scale与记录身份。
  - 本项归纳：活动Ragdoll的mesh在world pause下改变relative scale，0.75/1.5/0.01的双anchor按源更新，正nonuniform保持原frame，80组全部PASS。最终4项focused为3PASS/1预期语义FAIL，300组298PASS/2FAIL；移除超限速度或移出启动限速期，D75反例消失。116份基线source、7份补充UE source、fresh PA原始/Build及160份生产Rust和同一launcher库身份核验PASS，证据`22/final/`、`22/summary.json`；没有生产设计消融对象，未改转换规则或target资产。累计60项P2、13项P3，D08/D61撤销、U01未闭合；截图0、性能暂停。关节求解/积分、启动速度和pelvis/GetUp继续独立闭包。

- [x] BF144.23 PA角约束与角drive数值求解：沿fresh PA当前启用的cached/linear Chaos joint solver及target SourcePhysicsJoint实际调用，核对twist/swing/locked组合、soft limit、角drive误差轴与单位、dt/tolerance和多次位置迭代中的角增量/累计量。locked point、质量/惯量conditioning、projection/velocity阶段及body积分继续拆出；本项只追角求解所需直接输入，不能以静态配置PASS代替行为等价。Gate：source算法与默认分支证据、实际生产solver的独立最小数值反例/对照、真实角色输入链和未闭合阶段登记。开始：2026-10-14 07:12 +08:00；预估结束：2026-10-14 08:07 +08:00；实际结束：2026-10-14 07:33:06 +08:00；偏差：−33分54秒；结果：审计记录Gate PASS，新增2项P2、2项P3，未修复。估时依据：fresh source选中FPBDJointCachedSolver，Init/角限位/角drive及utilities闭包约1200行，target角求解约300行；BF144.18变换24分06秒、.21含native探针35分34秒、.22跨阶段45分58秒，point/conditioning/projection单独后置以保持本批独立。
  - 07:12 活动摘要：BF144.22记录Gate已提交，累计60项P2、13项P3。native PA19个活动joint采用linear/cached solver；fresh Init把角容差乘min(1,3600×dt²)，target当前直接使用配置容差，尚需按实际soft/locked/drive分支验证。截图0、性能暂停，无移植修复。
  - 07:31 source/focused闭环：完整角色的实际physics schedule在受控双body、保留native角限位/6次迭代且隔离point/contact后，复现硬twist锁、dt容差、多轴soft SIMD顺序和近180°混合限位guard四类差异，登记D76–D79；9组3PASS/6FAIL，源参考按fresh C++独立复算，未宣称执行了native Chaos仿真。直接生产solver另有54组36PASS/18FAIL；去掉多轴、去掉惯量耦合、恢复60Hz或跨回退化阈值内，对应失败消失。先前关闭mass conditioning的夹具已删去该覆盖，当前保持native启用且输入惯量比低于其生效阈值，信号不变。下一步补齐19个native drive分支的限位内对照并核对记录；soft阻尼的非零初始角速度/积分衔接留下一物理阶段批次。
  - 本项归纳：19个活动joint的零目标与三轴小目标×1/6次迭代共76组全部PASS；总计7项focused为1PASS/6预期语义FAIL，139组115PASS/24FAIL。fresh源确认SIMD CVar实际传入linear container，源Rotation积分和quat Jacobian按本机实现复算；116份基线source、11份补充UE source、PA原始/Build和160份生产Rust及launcher库身份核验PASS。证据`23/final/`、`23/summary.json`；仅新增审计夹具，未改生产算法/转换器/资产。累计62项P2、15项P3，D08/D61撤销、U01未闭合；截图0、性能暂停。非零角速度soft阻尼衔接、完整point/conditioning/projection/velocity与island迭代预算继续后续批次，没有将受控角分支验证推广为全rig等价。

- [x] BF144.24 PA锁定锚点、conditioning、projection与速度约束：沿fresh Chaos cached point/SIMD及实际PA设置，核对target的线性位置求解、质量/惯量修正、投影与末端速度回写；只读这些分支的直接输入与真实角色调用。非零角速度的soft阻尼/积分衔接及body动力学留下一独立批次。Gate：源公式与实际默认分支、生产solver反例和控制、完整角色消费及未覆盖边界。开始：2026-10-14 07:38 +08:00；预估结束：2026-10-14 08:33 +08:00；实际结束：2026-10-14 08:05:27 +08:00；偏差：−27分33秒；结果：审计记录Gate PASS，新增4项P2，未修复。估时依据：fresh source已定位point/velocity/projection约1000行、conditioning约100行，target相关约300行；BF144.23角分支实耗21分06秒，.22跨阶段实耗45分58秒；将body积分另拆后按本批矩阵/阶段组合估时。
  - 07:38 活动摘要：BF144.23已提交，累计62项P2、15项P3，U01未闭合。当前PA19个活动joint三轴linear Locked，source启用cached SIMD；先核对锚点校正是否使用相同的轴、质量矩阵和增量，再检查后续projection/velocity。截图0、性能暂停，生产实现未改。
  - 07:46 source/focused闭环：源point按三轴cached Jacobian同时计算，目标位置与projection共用沿总误差方向的Avian球形约束，完整角色受控spine_01的8组4PASS/4FAIL，确认D80；直接solver12组10PASS/2FAIL。质量/惯量conditioning的16组全部PASS，覆盖static parent、0.01/0.2/1质量、惯量比100与旋转坐标基。新增读取源实际迭代CVar，仅扩展现有reader枚举前缀；PA的6/1字段由源码标注为RBAN设置，是否错误用到角色world ragdoll仍待实际CVar和消费者核对。
  - 07:57 source/focused闭环：fresh PhysicsSettings与solver CVar确认world默认8/2/1、无迭代覆盖；Mesh/PA body的bOverrideIterationCounts=False，不能直接把显示的body计数当有效覆盖。19个实际活动joint全部错用RBAN的6/1/1，D81由完整角色速度阶段的1次失败/2次通过另行证明。把两边都控制为2次后，同样的预测connector旋转0.5/1rad仍有1.845/3.768rad/s角速度差，确认D82的旧力臂/轴；取消帧内旋转即PASS。projection单轴的角速度回写控制通过，没有把“quat表示不同”单独登记为速度缺陷。
  - 本项归纳：unit/0.75/0.5角色、锁骨/手腕、0/0.0011/0.01rad的硬角速度激活18组15PASS/3FAIL，确认D83，已用源quat Jacobian和惯量单位复算lambda。最终6项focused为1PASS/5预期语义FAIL，79组47PASS/32FAIL；零力臂、零预测旋转、匹配预算、恢复unit或较大角误差分别去掉对应失败。116份基线source、19份补充UE源码、7份实际Avian依赖、2份fresh native、原始/Build及160份生产Rust和同一launcher库身份核验PASS。reader只在既有枚举增加2个前缀，新旧原生payload除了4个迭代CVar完全相同；未改转换规则或生成资产，没有新增运行时抽象。证据`24/final/`、`24/summary.json`。累计66项P2、15项P3，D08/D61撤销、U01未闭合；截图0、性能暂停，未执行公共Playground Gate。非零角速度soft阻尼/积分、完整mixed-axis hard及body动力学继续独立批次。

- [x] BF144.25 Ragdoll角速度积分与角约束衔接：沿fresh UE动态body的W更新、最大角速度与quat积分、cached soft阻尼的初始/预测旋转差，并补选定mixed-axis hard角分支；核对真实角色的预测旋转、soft输入和角修正。body线性动力学、sleep/CCD、动作初速及pelvis/出口pose另立依赖批次。Gate：实际源路径/单位/时钟、独立数值参考与完整角色消费者、反例/控制及剩余边界。开始：2026-10-14 08:10 +08:00；预估结束：2026-10-14 09:00 +08:00；实际结束：2026-10-14 08:43:04 +08:00；偏差：−16分56秒；结果：审计记录Gate PASS，新增1项P2、1项P3，未修复。估时依据：fresh UE Integrate主体约110行、Rotation约60行、cached soft/locked直接闭包约350行，target积分/角求解约200行；BF144.23角分支实耗21分06秒、.24跨阶段含reader实耗27分27秒；本批仅展开角运动，避免合并全部body动力学。
  - 08:10 活动摘要：BF144.24已提交，累计66项P2、15项P3。fresh UE动态body实际调用normalized一阶quat积分；Avian当前使用from_scaled_axis。soft阻尼源又从初始/预测quat差计算ConnectorWDts，target使用raw W×dt；先分别隔离预测积分与阻尼输入，避免把同一结果重复归因。截图0、性能暂停，无移植修复。
  - 08:19 source/focused闭环：fresh PA 21个body均AngularDamping=0、gyro=False；角阻尼全局覆盖−1、HackMaxAngular=1000rad/s，实际body/项目上限62.83185rad/s。保留目标默认6 substeps的完整角色自由角积分15组6PASS/9FAIL，确认D84；30Hz与100ms帧均正确采用约33.33ms物理上限，但旋转积分公式不同。全部15个native Locked轴的60组40PASS/20FAIL、混合lowerarm锁6组2PASS/4FAIL补充已有D76，不新增根因。下一步在双方预测姿态已经相同的条件下检查soft阻尼。
  - 08:41 source/focused闭环：统一预测quat后，spine_01 SIMD与neck_01 scalar soft两路在60/30Hz、正负W和零W共24组8PASS/16FAIL，确认D85；移除阻尼的4个控制全部PASS。预测姿态、有效惯量和native K/D的消费已经核验，未把自由积分D84混入阻尼根因。本批只剩记录身份/最终Gate；目标默认6个物理子步与source未启用substepping的整体调度，以及线性body动力学/sleep/CCD留下一独立批次。
  - 本项归纳：最终4项focused均为预期语义FAIL，105组56PASS/49FAIL，归并新增D84/D85及已有D76；所有执行均正常完成，非watchdog失败。116份基线source、10份补充UE源码、3份实际Avian依赖、fresh PA与当前PhysicsSettings、原始/Build及160份生产Rust和launcher库身份核验PASS。reader在既有枚举增加两个精确前缀，新旧native payload仅多两个全局角运动CVar；无转换器/资产/运动实现修改。夹具中的阻尼消融去掉对应差异，所有生产数据/二进制保持原样。证据`25/final/`、`25/summary.json`。累计67项P2、16项P3，D08/D61撤销、U01未闭合；截图0、性能暂停，未执行公共Playground Gate。下一闭包为整体物理调度和线性body动力学，随后继续sleep/CCD、启动速度、pelvis/snapshot/GetUp。

- [x] BF144.26 Ragdoll物理子步与线性body积分：沿fresh Chaos scene/solver的同步步长、动态body线性速度/阻尼/位置更新，与实际角色的Avian调度逐项对照；重力统一采用Bevy值，只验证算法与执行阶段。sleep/CCD、动作初速、pelvis/snapshot/GetUp继续后续批次。Gate：实际源设置与调用路径、独立数值参考、完整角色反例/消融控制及剩余边界。开始：2026-10-14 08:58 +08:00；预估结束：2026-10-14 09:48 +08:00；实际结束：2026-10-14 09:06:00 +08:00；偏差：−42分00秒；结果：审计记录Gate PASS，新增1项P2、1项P3，未修复。估时依据：fresh Integrate约110行、scene/solver步长分支与Avian预处理/子步直接闭包约400行；BF144.24实耗27分27秒、.25实耗33分04秒，本批另含原生CVar补读和多步长控制，按独立线性闭包估时。
  - 08:58 活动摘要：BF144.25已提交，累计67项P2、16项P3。源Integrate先加加速度再乘线性阻尼，target先乘隐式阻尼再加速度增量；target默认六子步尚需与源实际同步调度和位移消费者闭合。未确认本批新差异，截图0、性能暂停，生产实现未改。
  - 09:04 source/focused闭环：fresh PhysLevel→ChaosScene→PhysicsSolverBase→PBDRigidsSolver证明当前同步、无substepping时每帧只执行一次受MaxPhysicsDeltaTime约束的积分；目标实际六子步。完整角色spine_01保留PA mass，隔离joint/contact并移除阻尼，12组9PASS/3FAIL确认D86：30Hz首帧自由落体少下落4.541mm，恢复单步全部通过。双方统一单步后，native阻尼24组17PASS/7FAIL确认D87；移除阻尼12组全部通过，源/target均使用Bevy重力。原生默认阻尼+六子步的6组3PASS/3FAIL同时复现；没有执行native Chaos仿真或公共Playground Gate。
  - 本项归纳：3项focused均为预期语义FAIL，42组29PASS/13FAIL；扩充阻尼矩阵后只重跑该项，其他两个测试及共享夹具字节保持一致。116份基线source、11份补充UE源码、4份实际Avian依赖、两份fresh native、原始/Build及160份生产Rust和launcher库身份核验PASS。reader只增加两个CVar枚举，新旧PA payload除此完全一致；第一次读取在成功构建后因输出目录尚未存在而结束，补齐目录后读取成功，未计作语义失败。证据`26/initial/`、`26/damping/`、`26/summary.json`。未改转换器、资产或运动实现，无新增运行时抽象；单步与移除阻尼控制完成对应消融。累计68项P2、17项P3，D08/D61撤销、U01未闭合。估时回顾：复用已就绪的完整角色/solver夹具，使本次线性闭包实耗8分钟，后续类似单公式审计应减少预留；sleep/contact仍需独立估算。

- [x] BF144.27 Ragdoll睡眠判定、岛计数与唤醒：沿fresh Chaos IslandManager的实际非partial分支、native材质阈值和平滑速度，核对target SourceBodySleep的字段、采样、计数和真实body休眠/恢复；只展开其直接依赖。碰撞solver/CCD、动作初速及pelvis/snapshot/GetUp另立后续批次。Gate：原生开关/阈值、source阶段公式、完整角色控制与反例或无差异证据。开始：2026-10-14 09:09 +08:00；预估结束：2026-10-14 09:44 +08:00；实际结束：2026-10-14 09:38:40 +08:00；偏差：−5分20秒；结果：审计记录Gate PASS，新增3项P3，未修复。估时依据：fresh索引定位阈值/孤立/连接岛与wake约500行，target约110行及既有focused夹具；BF144.26复用完整角色线性夹具实耗8分钟，sleep增加跨帧及拓扑控制，单独给出本批估时。
  - 09:09 活动摘要：BF144.26已提交，累计68项P2、17项P3。目标显式实现source sleep计数和平滑采样；源partial sleep默认关闭，但仍需本次原生CVar核实。下一步先读取实际阈值与UpdateSleep顺序，随后使用源位移/角度作为共同输入隔离已知积分差异。截图0、性能暂停，未展开碰撞求解。
  - 09:20 source/focused闭环：fresh native证明sleep启用、partial关闭、平滑0.3、孤立倍率均1，21个PA body实际材质阈值为0.01m/s、0.05rad/s、counter=4。完整角色hand_l的孤立/连接岛×60/30Hz，共24组16PASS/8FAIL确认D88；第4帧显式设置0.005m/s后，目标第5帧休眠，源孤立/连接分支应第8/9帧。去掉sleep采样覆盖后四组自然积分反例仍失败；无速度写入、持续超阈值的控制通过。下一步核对同一外部输入闭包中的force对平滑状态预置及连接变化，未扩展接触solver。
  - 09:36 source/focused闭环：实际单帧1m/s²推力经native质量和Avian force阶段进入两body，4组3PASS/1FAIL确认D89，60Hz连接岛因遗漏源force平滑预置而第5帧休眠；30Hz和孤立控制通过。fresh CreateIsland/ProcessWakes另证明新岛计数从0开始，target按每body旧计数最小值继续；静止3帧后第4帧连接的两组均提前第5帧睡眠，source应第8帧，确认D90；反向断开两组正确保留计数。共享夹具已完成无采样覆盖的自然积分版本，接下来只收口本批focused回归和记录身份。
  - 本项归纳：最终3项focused均为预期语义FAIL，32组21PASS/11FAIL，26.25秒正常完成；force参考连续推进满12帧。116份基线source、7份补充UE源码、6份实际Avian依赖、fresh PA及21份native/target sleep字段、原始/Build和160份生产Rust/launcher库身份核验PASS。reader在既有枚举增加sleep与平滑前缀，新旧payload仅新增24个CVar；未改转换器、资产或运动实现。去掉metric覆盖、去掉速度/force更新，以及反转连接变更方向的控制均已记录，没有新增运行时抽象。证据`27/final/`、`27/summary.json`。累计68项P2、20项P3，D08/D61撤销、U01未闭合，截图0、性能暂停；下一闭包为native CCD开关与实际碰撞接触求解，再继续动作初速、pelvis、snapshot/GetUp与动画。

- [x] BF144.28 Ragdoll原生CCD开关与法向接触/回弹：沿fresh PA body开关、实际Chaos碰撞solver选择和restitution阈值/冲量，核对完整角色的接触消费者；先隔离已知子步/积分差异。摩擦及复杂接触形体另立后续批次，动作初速、pelvis/snapshot/GetUp仍保持后续范围。Gate：原生设置、实际source路径、独立公式与完整角色反例/控制、身份与未闭合边界。开始：2026-10-14 09:44 +08:00；预估结束：2026-10-14 10:34 +08:00；实际结束：2026-10-14 10:13:06 +08:00；偏差：−20分54秒；结果：审计记录Gate PASS，新增2项P2、1项P3，未修复。估时依据：fresh索引定位Chaos collision容器/solver直接闭包与Avian contact solver，法向和restitution分支预计约600行；BF144.27跨阶段审计实耗29分40秒，本批另含原生读取与接触夹具，限制在法向响应。
  - 09:44 活动摘要：BF144.27已提交，累计68项P2、20项P3。当前PA消费者通过native CCD标志决定安装独立CCD系统，普通接触仍交给Avian；尚未确认本批差异。先确认实际原生开关及全局回弹配置，再沿真实solver调用闭合。截图0、性能暂停，生产实现未改。
  - 10:04 source/focused闭环：fresh PA全部21个body的CCD/MACD关闭、材质restitution=0.3、soft=None；源普通Gauss–Seidel、全局回弹覆盖关闭。完整角色的中心单接触24组17PASS/7FAIL，确认D91材质未传到子shape、D92回弹阈值未按物理delta换算；各自只替换输入的控制恢复，单步与原生6子步材质结果相同。早期零SpeculativeMargin夹具会清零接触测速，已排除且保留诊断。下一步核对法向位置恢复及原生shape材质消费者，尚不声称完整接触等价。
  - 10:10 source/focused闭环：原生21个body/shape未改的自然落地4组2PASS/2FAIL进一步确认D91，复制材质到子shape恢复；全体body实际均未安装CCD组件。硬接触法向恢复24组16PASS/8FAIL确认D93，去掉初始重叠或改用完整法向修正的控制通过；native材质soft=None、stiffness=1、初始穿透恢复无限制，测试深度均低于10m/s pushout上限。本批保持法向/回弹范围，摩擦、预测接触和复杂形体接触另立后续闭包。
  - 本项归纳：最终4项focused均为预期语义FAIL，52组35PASS/17FAIL，41.14秒正常完成。116份基线source、14份补充UE源码、6份实际Avian依赖、fresh PA/PhysicsSettings、21份target CCD/材质字段、原始/Build和160份生产Rust/launcher库身份核验PASS；证据`28/final/`、`28/summary.json`。reader只在既有枚举增添5个前缀，新旧PA payload仅增加37个CVar，无转换规则/资产/运动实现改动；输入控制完成各差异消融。累计70项P2、21项P3，D08/D61撤销、U01未闭合；截图0、性能暂停，未运行公共Playground Gate。实耗29分06秒与上一批29分40秒接近，后续单个接触子闭包据此估时。

- [x] BF144.29 Ragdoll切向摩擦求解：沿fresh native材质与Chaos位置/速度摩擦的实际阶段、Coulomb限制和角惯量，核对Avian切向消费及首接触/保留接触锚点。仅沿Ragdoll实际普通接触闭包，预测接触/cull与复杂多接触保留后续批次。Gate：源单位与迭代阶段、独立参考、完整角色消费者及控制/反例、身份和剩余边界。开始：2026-10-14 10:20 +08:00；预估结束：2026-10-14 11:10 +08:00；实际结束：2026-10-14 10:46:48 +08:00；偏差：−23分12秒；结果：本批审计记录Gate PASS，新增2项P3，未修复；非零历史锚点位移/滑移与预测接触继续下一批。估时依据：fresh索引定位位置friction cone/velocity摩擦与manifold锚点约500行、Avian切向约250行；BF144.28包含两次native读取与4项focused实耗29分06秒，本批需要隔离已知法向差异及切向/角向耦合，独立估时。
  - 10:20 活动摘要：BF144.28已提交，累计70项P2、21项P3。源当前材质dynamic=0.7、static=max(0,0.7)=0.7；源同时有位置与速度摩擦阶段，不能根据旧注释假定quadratic只有位置摩擦。先核对实际迭代设置和源码公式，以共同法向输入隔离D93；尚未确认本批差异。
  - 10:37 source/focused闭环：源one-shot sphere/box manifold开启且非incremental，位置摩擦4轮×0.5、速度摩擦末1轮已闭合。公共角色common-contact夹具中，法向去柔性并固定其线性间距后24组18PASS/6FAIL确认D94切向位置差异；source零力臂闭式参考8组全PASS。单独恢复目标法向旋转采样后18组10PASS/8FAIL确认D95；换用源quat积分仍失败，仅线性法向控制恢复，避免归入D84。下一步补保留接触分支；初版把Avian内部anchor-offset误读为直接separation的oracle已修正并排除，调度前置失败不计语义缺陷。
  - 10:43 source/focused闭环：保留同一个实际ContactId，先以静止接触清零重叠、次帧仅更新V/W；60/30Hz×摩擦0/0.7×切速0/0.1/1的12组8PASS/4FAIL，继续归入D94。源quadratic本地点未改变，符合保存锚点的精确匹配条件；该分支WorldFrictionDelta与本帧V×dt相同。非零历史滑移锚点、锚点滑移/丢弃和预测/复杂manifold仍为下一独立闭包，未将本批稳定锚点控制扩充为这些边界的通过证据。
  - 本项归纳：最终4项focused为1PASS/3预期语义FAIL，62组44PASS/18FAIL，41.62秒正常完成。116份基线source、14份补充UE源码、9份实际Avian依赖、fresh PA/PhysicsSettings、全部21份target摩擦字段、原始/Build与160份生产Rust/launcher库身份核验PASS。reader在既有枚举增加6个前缀，新旧PA payload仅新增7个CVar；未改转换器、资产或运动实现。对照独立去掉摩擦、切速、法向非线性及替换quat积分，消除了对应失败；证据`29/final/`、`29/summary.json`。累计70项P2、23项P3，D08/D61撤销、U01未闭合，截图0、性能暂停，无公共Playground Gate。实耗26分48秒；本批补齐稳定保留接触，历史锚点跨帧状态需独立建模，按上述未闭合边界自动继续。

- [x] BF144.30 Ragdoll历史摩擦锚点恢复与丢弃：沿fresh保存锚点、static-friction ratio、quadratic匹配距离及下一帧消费者，核对目标接触缓存是否保留同一输入信息；区分D94单帧位置阶段与跨帧状态缺失。预测接触/cull及复杂多点仍随后独立推进。Gate：有效历史状态与单位、保留/滑移/无摩擦/超距控制、完整角色实际接触缓存消费和剩余边界。开始：2026-10-14 10:54 +08:00；预估结束：2026-10-14 11:34 +08:00；实际结束：2026-10-14 11:01:38 +08:00；偏差：−32分22秒；结果：单点历史缓存审计记录Gate PASS，新增1项P3，未修复；多候选匹配/多点裁剪保留后续复杂接触批次。估时依据：fresh缓存保存/匹配直接闭包约220行、Avian匹配约50行及BF144.29已验证的独立位置摩擦参考；上一批实耗26分48秒，本批只增加跨帧缓存输入与四类控制。
  - 10:54 活动摘要：BF144.29已提交，累计70项P2、23项P3。源保留旧ShapeAnchorPoints，超出静摩擦锥时按ratio插值、近零ratio改用当前点；target match_contacts只复制warm-start冲量。仅有字段差别不计缺陷，下一步用源匹配容差内的历史偏移追到实际角色接触求解，保留零偏移与超距对照。
  - 10:59 source/focused闭环：在完整角色head的同一实际ContactId上注入受控历史锚点，当前切向V/W为零，目标下一帧重建几何点时丢弃该偏移。60/30Hz共16组8PASS/8FAIL确认D96；±1mm与5mm历史偏移分别缺少约0.494mm和2.470mm的源位置恢复，ratio=0.5时应恢复一半。零偏移、ratio=0改用当前点、零摩擦及20mm超距不匹配全部PASS。当前输入为受控缓存反例，不宣称复现完整自然Ragdoll轨迹；稳定接触和共同法向参数已核验。
  - 本项归纳：1项focused为预期语义FAIL，16组8PASS/8FAIL，13.17秒正常完成。116份基线source、4份补充UE源码、4份实际Avian依赖、fresh PA与上一批全部原生payload一致、原始/Build及160份生产Rust/launcher库身份核验PASS；复用的BF144.29参考源码与已通过版本逐字节相同。证据`30/final/`、`30/summary.json`。无reader/转换器/资产/运行时变更；零历史偏移、清除旧偏移、移除摩擦和不匹配控制完成输入消融。累计70项P2、24项P3，D08/D61撤销、U01未闭合，截图0、性能暂停。复用现有数值参考使实耗降到7分38秒，此类已有夹具的缓存输入审计后续应缩短预估；下一批转入接触预测与采样时点。

- [x] BF144.31 Ragdoll接触采样时点与预测接触：沿fresh Chaos积分→碰撞检测→回弹目标、有限cull及Avian检测→积分→求解链，核对重力后的接触速度和无实际碰撞轨迹的预测响应。复杂多点接触仍独立推进。Gate：源实际调度/原生开关、完整角色实际消费者、去重力/替换采样/去预测控制与身份记录。开始：2026-10-14 11:12 +08:00；预估结束：2026-10-14 11:52 +08:00；实际结束：2026-10-14 11:38:03 +08:00；偏差：−13分57秒；结果：审计记录Gate PASS，新增1项P2、1项P3，未修复。估时依据：fresh索引定位两侧积分和narrow-phase直接闭包约450行；复用BF144.28单接触夹具，其实耗29分06秒；BF144.30缓存输入复用实耗7分38秒，本批增加原生shape与几何不相交证明。
  - 11:12 活动摘要：BF144.30已提交，累计70项P2、24项P3。源在积分后生成接触，target先缓存normal_speed再进入速度积分；默认speculative margin为MAX，已排除默认有限margin夹断速度的猜测。先核对实际原生shape是否覆写margin，再建立共同重力/材质的回弹时点反例；尚未确认本批新增差异。
  - 11:17 source/focused闭环：完整角色中心接触16组12PASS/4FAIL确认D97，30Hz回弹目标少0.0981m/s；零重力、仅替换积分后normal_speed均恢复，原生六子步与单步一致。fresh PA reader PASS；下一步仅核对原生head形状在全程保持间距时的预测响应，尚未确认预测缺陷。
  - 11:35 source/focused闭环：沿actual child collider→frame-end AABB→下一帧broad-phase闭合D98。子shape同步排在预测包围盒之后，包围盒只覆盖刚结束的位移；当前帧末50mm重叠仍无接触，原速度保留。16组12PASS/4FAIL，单步和原生六子步相同；把同一shape pose同步提前到AABB前，四处失败均恢复。临时保留旧包围盒未改善，改用当前pose后恢复，已删除多余缓存注入；初版首帧搬动及错误“Last时child仍滞后”断言保留为诊断、不计语义证据。原生head的近/远间距和完全穿越非CCD控制继续核验。
  - 本项归纳：最终5项focused为3PASS/2预期语义FAIL，68组60PASS/8FAIL，51.22秒正常完成；原生head的150mm与5mm净距、有限source bounds之外的非CCD穿越控制36组全PASS，未另记预测误撞。116份基线source、10份补充UE源码、10份实际Avian依赖、fresh PA/PhysicsSettings与上一批payload一致、原始/Build及160份生产Rust/launcher库身份核验PASS。证据`31/final/`、`31/summary.json`。无reader/转换器/资产/运行时变更；消融将手工bounds缓存和平移收敛为fixture里的同一pose同步时点。累计71项P2、25项P3，D08/D61撤销、U01未闭合；截图0、性能暂停。实耗26分03秒；下一批按source多点接触的实际normal/restitution消费继续，动作初速与GetUp边界保持待审。

- [x] BF144.32 Ragdoll多点接触速度响应：沿fresh native shape、Capsule/Box manifold与普通collision solver的多点循环，核对回弹迭代、normal冲量边界及实际body角速度；几何生成和求解输入分开取证，避免把D93/D97/D98重复编号。Gate：原生实际开关、同一组点/法向/惯量的独立参考与完整角色消费、单点/零回弹/匹配迭代控制、剩余几何边界。开始：2026-10-14 11:43 +08:00；预估结束：2026-10-14 12:23 +08:00；实际结束：2026-10-14 12:03:19 +08:00；偏差：−19分41秒；结果：审计记录Gate PASS，新增1项P3，未修复。估时依据：fresh索引定位CapsuleConvex one-shot与collision velocity/restitution约450行；复用BF144.31角色接触夹具（实耗26分03秒），新增实际两点输入与独立速度参考。
  - 11:43 活动摘要：BF144.31已提交，累计71项P2、25项P3。fresh源码及原生CVar均显示AveragePointEnabled=False，已排除“源必先按平均点回弹”的候选；当前target restitution默认1次，source世界velocity为2次。先确认实际native多点几何及迭代消费者，再判断是否与既有D81同根，不因配置字段不同直接登记。
  - 11:56 source/focused闭环：实际native head接触的两个端点受力，中点total_impulse=0、回弹会跳过；未把无效点数差记缺陷。相同点/法向/有效惯量和零阶段输入V/W下，16组isotropic控制14PASS/2FAIL、4组native惯量2PASS/2FAIL确认D99；仅匹配collision回弹迭代为2即可恢复。该资源独立于D81的joint RBAN字段，修正joint预算不会改变它。前置点数/单位法向断言已依据实际消费修正，共用实际normal后对照通过，未放宽比较误差或声称执行native Chaos仿真。
  - 本项归纳：最终4项focused为2PASS/2预期语义FAIL，36组32PASS/4FAIL，23.62秒正常完成。Source两点两轮递推的独立闭式结果4组PASS；单点正/负normal冲量、下限不足/足够的12组PASS，给定P/dt时目标下限与源一致。116份基线source、6份补充UE源码、4份Avian与1份Parry源码、fresh PA（与上一批全payload一致）、原始/Build及160份生产Rust/launcher库身份核验PASS。证据`32/final/`、`32/summary.json`。无reader/转换器/资产/生产变更；去掉第二点、去掉回弹、只匹配迭代与恢复native惯量分别完成控制。累计71项P2、26项P3，D08/D61撤销、U01未闭合，截图0、性能暂停。实耗20分19秒；其他姿态的点生成/裁剪/排序和contact/joint耦合仍未闭合，继续独立取证。

- [x] BF144.33 原生Ragdoll接触点生成与裁剪：复用native reader的PhysicsBodyProbe入口和本机公开Chaos collision API，提取PA实际primitive在不同Capsule/Box姿态下的manifold，与目标实际形状的点/法向/深度及consumer对比。工具只读取原始资产，临时请求决定姿态；不复制GJK算法或修改源资产。Gate：原生请求/输出与watchdog、原始/Build身份、坐标/单位复算、正负间距与姿态控制、实际消费反例或明确通过/剩余边界。开始：2026-10-14 12:08 +08:00；预估结束：2026-10-14 12:58 +08:00；实际结束：2026-10-14 13:27:48 +08:00；首轮偏差：+29分48秒；续轮开始：2026-10-14 13:24 +08:00；续轮预估结束：2026-10-14 13:54 +08:00；续轮偏差：−26分12秒；结果：本项记录Gate PASS，新增2项P3，剩余支持点消费者已拆分为BF144.34。估时依据：fresh公开Make/UpdateConstraint接口及CapsuleConvex one-shot约250行、已有PhysicsBodyProbe约65行；上一批多点consumer实耗20分19秒，本批增加reader原生能力和构建/请求自测，限定primitive对box的接触生成。
  - 12:08 活动摘要：BF144.32已提交，累计71项P2、26项P3。已定位本机Chaos导出的constraint factory/update及manifold读取接口，原生point生成尚未执行；先验证可复用的构造/形状提取路径，避免把目标接触数据当作源数值。其他运动/动画边界仍继续在本轮总体范围内。
  - 12:25 工具摘要：已增加独立PhysicsContactProbe请求路径；复用InitBody形状提取、native constraint allocator与UpdateConstraint，原始资产未改。构建已过语法阶段，但直接实例化粒子存储拉入未导出的旧TAABB<double>模板，尚未得到有效原生结果；已保留失败报告并切换到引擎导出的solver构造入口，不复制或修补引擎模板。
  - 12:31 source/native闭环：改用FChaosSolversModule工厂后reader构建及head smoke均PASS；原生输出2个端点，1mm重叠的phi分别为−0.100000054/−0.099999942cm，点差投影与phi一致。除新增probe字段外，PA完整payload与上一批逐值一致。下一步获取全部实际primitive并建立姿态/间距矩阵；尚未按raw点数或排序差异登记新缺陷。
  - 12:56 source/consumer闭环：22个实际primitive远距控制全部无接触；54组head/calf/hand/foot/root的native姿态矩阵已取得，7组版本化自测PASS，零quat请求按预期以7退出。普通head端点顺序经source gather和有限两轮速度求解闭合D100，8组4PASS/4FAIL；近水平0.5度的native法向/接触支持闭合D101，4组2PASS/2FAIL，源几何输入控制恢复。54组raw比较另含较大倾角的支持点及微小法向数值差，未全部计作已确认运动缺陷。
  - 13:23 超时复盘：第一轮开始12:08、预估结束12:58、实际结束13:23:38，偏差+25分38秒，记录Gate尚未收口。已回读本项、native factory/shape→UpdateConstraint→source gather→实际回弹消费者闭包及最终watchdog；当前3项focused均正常完成，66组38PASS/28差异，其中54组raw数据比较不能等同54组运动验收。漂移来自公开头文件可用但旧模板未导出、solver工厂所有权与新夹具消费输入的核验，未出现外部权限阻塞。纠正：停止扩展当前样本，保留已完成的D100/D101与工具验证；当前项只收口身份/记录，其他原生支持点影响拆到BF144.34。按上一轮估时窗口1.5倍建立分段：本项续轮开始2026-10-14 13:24 +08:00、预估结束2026-10-14 13:54 +08:00；下一段开始2026-10-14 13:54 +08:00、预估结束2026-10-14 14:39 +08:00，由下一项按实际开工更新。
  - 本项归纳：116份基线source、13份补充UE源码、4份Avian与1份Parry、22个native primitive/54组接触/7组版本化自测及无效quat拒绝、原始/Build、160份生产Rust与launcher库身份核验PASS。reader新增generic PhysicsContactProbe，除probe字段外各完整PA payload与上一批一致；构建包源码与工作区逐字节一致。最终3项focused为预期语义FAIL，66组38PASS/28FAIL，9.81秒完成；其中实际consumer共12组6PASS/6FAIL对应D100/D101，其余为raw对比。移除继承的单点/stage分支后反例和对照不变；直接粒子存储及旧形状模板路径因不可链接被淘汰，保留native工厂。证据`33/final/`、`33/summary.json`；无转换器/目标资产/生产Rust改动，未触发AnimBP重生成。累计71项P2、28项P3；D08/D61撤销、U01及BF144.34边界未闭合，截图0、性能暂停。总实耗79分48秒表明此类首次native接口工作应先独立验证模块可链接性，再估计矩阵与consumer。

- [x] BF144.34 原生接触支持点与后续消费剩余边界：依赖BF144.33记录Gate；只沿已取得的capsule倾角支持点、box面选择及微小normal数值差核对实际normal/friction/跨帧消费，已确认D100/D101不重复编号。Gate：复用native数据身份、区分零冲量点与活跃点、最小实际消费者与受控替换/排除、未覆盖边界明确保留。开始：2026-10-14 13:35 +08:00；预估结束：2026-10-14 14:20 +08:00；实际结束：2026-10-14 13:48:02 +08:00；偏差：−31分58秒；结果：审计记录Gate PASS，新增1项P3，未修复。估时依据：BF144.33剩余54组原生矩阵中的支持点/数值边界，源gather和现有完整角色夹具已经可用；来自本次超时复盘后拆分，依赖已通过，按实际开工提前，不重新构建native工具或重复已验证反例。
  - 13:35 活动摘要：BF144.33已提交，累计99项确认差异。剩余raw矩阵只表示点集不同；本项先核对source position求解后的净pushout及velocity启用条件，再判断支持点能否改变实际角色响应。截图0、性能暂停。
  - 13:42 source/focused闭环：54组native姿态的source位置求解中，移除78个零净pushout点后DP/DQ全部保持；较大倾角capsule侧点和box远侧面不能仅因raw点集不同登记。另沿gather的质量加权接触位置确认D102：完整角色实际contact constraint的独立速度阶段，24组20PASS/4FAIL，仅替换接触位置恢复；初版0.1m球的零重叠受f32正间距影响，改为精确可表示的0.125m后零重叠控制通过，未放宽容差。下一步将剩余native活跃点送入实际回弹consumer，区分点生成与启用条件。
  - 13:46 source/consumer闭环：head倾角1/10/30度与foot 45度×60/30Hz的当前native形体响应8组全PASS（先匹配既知点序/迭代）。注入完整源点集的8组有2处head 10度失败，原因是替换后目标仍给源已归零的侧点启用回弹；只保留源活跃点的8组全PASS。该失败只发生在受控替换输入，未另记生产缺陷，也未反推当前几何错误。native BaseFrictionImpulse全部为0、全局override=−1，零pushout点没有额外摩擦下限；历史锚点信息缺失沿已确认D96保留，不重复编号。
  - 本项归纳：116份基线source、5份补充UE、4份Avian、54组native数据/21份材质、原始/Build、reader包源码及160份生产Rust/launcher库身份核验PASS。最终分组证据共3项focused、102组96PASS/6FAIL：4处实际consumer差异归D102，2处源全点集替换失败单独保留；8组原几何响应与78个零pushout点消融均通过。证据`34/weighted/`、`34/support_final/`、`34/summary.json`。删除继承的sphere/native-inertia分支后信号保持；无新增生产层、转换器或资产改动。累计71项P2、29项P3；contact/joint耦合与剩余缓存分支仍待审，截图0、性能暂停。实耗13分02秒；复用已验证native几何明显缩短了后续消费审计。

- [x] BF144.35 Ragdoll碰撞与关节耦合：依赖BF144.34记录Gate；沿fresh世界constraint container优先级、位置/速度阶段及当前SourcePhysicsJoint接线，核对碰撞回弹后关节约束与迭代交换，保持原生body/joint入口。Gate：实际世界排序与时钟、独立最小两body接触参考、完整角色消费者及去掉contact/joint的控制；已有D81/D93/D99不重复计数。开始：2026-10-14 13:50 +08:00；预估结束：2026-10-14 14:35 +08:00；实际结束：2026-10-14 14:01:44 +08:00；偏差：−33分16秒；结果：审计记录Gate PASS，新增1项P2，未修复。估时依据：fresh索引已定位constraint group循环、joint container与target调度，约500行调用闭包；BF144.24已完成点约束速度参考、BF144.34消费者实耗13分02秒，本项复用后只增加contact/joint联合路径。manifold多候选/保存分支另项，避免一次扩展超过一小时。
  - 13:50 活动摘要：源collision/joint优先级默认均0，实际顺序仍需由container注册和稳定排序证明；尚未按默认字段或调度名称登记差异。先核对fresh调用链，再构造两body中心接触，排除几何点序和角运动影响。
  - 13:58 source/focused闭环：fresh container注册、稳定排序及串行/并行prerequisite共同证明每轮collision→joint；目标joint velocity在Substep内，而Restitution在其后。32组速度消费者28PASS/4FAIL、6组位置消费者4PASS/2FAIL，确认D103；仅增加两侧各自迭代预算仍不能替代交替。移除contact/joint、零回弹控制通过，显式交替同一collision消费者和中心点约束后速度恢复。下一步核对native开关/身份并收口本项，不扩展其他物理类型。
  - 本项归纳：116份基线source、9份补充UE、5份Avian、fresh PA完整payload与BF144.32一致，原始/Build及160份生产Rust/launcher库身份核验PASS。native collision/joint priority均0、joint min/max stiffness均1且shock关闭；最终2项focused为预期语义FAIL，38组32PASS/6FAIL，30.90秒正常完成。证据`35/final/`、`35/summary.json`。用中心frame/单点消去角运动，只保留一处交替控制；移除contact或joint均失去失败信号，未新增生产抽象或修改调度。累计72项P2、29项P3；剩余缓存分支和动作/动画链路继续，截图0、性能暂停。实耗11分44秒；联合消费者可复用既有独立参考，后续估时按实际闭包缩小。

- [x] BF144.36 Ragdoll原生接触缓存剩余分支：依赖BF144.35记录Gate；只沿PA实际primitive的TryRestoreManifold、saved点多候选与容量分支，区分几何复用、静摩擦历史和初始穿透状态；D96已确认的输入丢失不重复编号。Gate：fresh原生开启条件和实际caller、必要的两帧native查询、target实际接触consumer或可证明排除、重复点/失配控制和剩余边界。开始：2026-10-14 14:06 +08:00；预估结束：2026-10-14 14:46 +08:00；实际结束：2026-10-14 14:37:11 +08:00；偏差：−8分49秒；结果：审计记录Gate PASS，新增1项P3，未修复。估时依据：fresh索引约500行恢复/匹配闭包，native公开SetLastShapeWorldTransforms/TryRestoreManifold已定位；复用BF144.33原生工具与BF144.34消费者（13分02秒），预留一次现有模块重编和两帧数据自测。首次native接口BF144.33超时的ABI教训要求先验证链接，再扩样本。
  - 14:06 活动摘要：source有独立geometry恢复与saved friction匹配，target当前contact query每次生成新manifold，再复制旧impulse。仅凭缓存实现不同尚不能确认新的运动差异；先核对本批实际开启条件及两帧消费。
  - 14:29 source/native/consumer闭环：28组native两帧结果确认箱体小幅移动可复用、超过容差重建，球/胶囊禁止geometry复用；fresh PA新增SimpleAssignContact=True外与上一批全payload一致。保持实际ContactId、关闭摩擦并统一点序/回弹预算的32组两帧消费者28PASS/4FAIL，确认D104；仅换入native复用点后恢复。首版反复写collision组件导致ID重建，修正初始化时点后通过；平放四点的活跃点前置不成立，改用已有源证据的45度两点支撑，未改语义容差。
  - 14:34 source/static闭环：native SimpleAssignContact=True走首个exact/最近near匹配，允许复用同一saved点；这一路输出的ShapeAnchorPoints仍归D96缺失，不因多候选另计。primitive/box one-shot将点数压到最多4，8点以上且friction ratio=0的保存省略分支在本批不触发；复杂mesh聚合不据此宣称已审计。原生depenetration=-1经`SetDepenetrationVelocity`变为1e10cm/s并写入粒子，当前正常接触的InitialPhi归零，没有新增有限恢复历史差异。
  - 本项归纳：116份基线source、9份补充UE、4份Avian、28组两帧native、原始/Build、reader包源码与160份生产Rust/launcher库身份核验PASS。reader增加可选next transform和三个阶段的点输出；旧7组自测输出逐值保持，新8组自测PASS，非法next quat按7退出。受控地把next世界坐标替换成旧坐标后新增自测按预期FAIL，原数据PASS，未改源文件或恢复mtime。最终1项focused为预期语义FAIL，32组28PASS/4FAIL，27.03秒完成。证据`36/final/`、`36/summary.json`。无转换器/目标资产/生产Rust变更；累计72项P2、30项P3，截图0、性能暂停。实耗31分11秒；下一批回到动作生命周期，已有Playground convex/trimesh的接触生成边界保留到物理剩余项，不扩展场景表现。

- [x] BF144.37 Ragdoll启动运动交接与初期速度限制：依赖BF144.36记录Gate；沿fresh SetSimulatePhysics/ResetAllBodiesSimulatePhysics与pelvis速度读取，核对动画驱动刚体转动态时的线/角速度、初始speed limit来源及8帧预算，再核对同闭包的驱动强度测速时点。BF144.20已完成的许可、movement lock和输入交接不复开；pelvis地面跟随/出口snapshot及GetUp动画另项。Gate：实际UE和target阶段/时钟、完整角色最小反例与静止/匀速/预算边界控制、source字段与生产身份。开始：2026-10-14 14:42 +08:00；预估结束：2026-10-14 15:32 +08:00；实际结束：2026-10-14 15:15:11 +08:00；偏差：−16分49秒；结果：审计记录Gate PASS，新增2项P2、2项P3，未修复。估时依据：fresh ALS入口/Refresh/限速约210行，UE mesh/body simulate接口与target模式/animated-body同步约450行；BF144.20真实交接审计实耗52分26秒，本项复用加载夹具与PA身份，只扩运动交接和限速输入，必要的更深kinematic求值闭包单独拆分。
  - 14:42 活动摘要：source从pelvis physics actor读取初始速度，并非直接把CMC速度赋给全部body；target也保留进入动态前的body速度。下一步追清两侧kinematic速度产生与simulate切换时点，不能仅凭缺少显式复制认定初速丢失。远端pull force属于本轮排除的网络路径，本地pelvis跟随保持后续边界。
  - 15:00 source/focused阶段：21个body的初始限速、entry speed下限及角速度保留控制通过；刷新前后驱动力160组144PASS/16FAIL全部归并已登记D75，不新增编号或复开实现。fresh UE的kinematic更新使用clamped physics delta与quat差分，target使用全帧delta与轴角；两帧动画→动态夹具尚有handoff前置失败，当前不将它登记为新差异。另已定位Controller→movement→actor的输入帧时序，将核对8帧预算是否包含启动当帧。
  - 15:10 source/focused闭环：前置失败已追到启动帧仍消费旧Kinematic图输出，8组6PASS/2FAIL及提前Dynamic控制确认D105；在夹具中先排除此项后，两帧速度生成24组15PASS/9FAIL分别确认D106的quat差分和D107的physics delta，单独替换两项可区分根因。Controller输入启动44个逐帧样本42PASS/2FAIL，确认D108仅多限速第9帧。source的立即限速与启动当帧Refresh必须分开计数；没有因为总调用数同为9而判等。下一步仅核对身份/记录Gate，不修改生产实现。
  - 本项归纳：116份基线source、17份补充UE、2份实际animation graph依赖、fresh PA/PhysicsSettings、原始/Build及160份生产Rust/launcher库身份核验PASS。kinematic transform回读开关不屏蔽V/W回读（SingleParticlePhysicsProxy.cpp:502–525），上述源速度确实到达game-thread与动态切换。最终4项focused为预期语义FAIL，236组207PASS/29FAIL，其中16组属于既有D75；38.62秒完成，证据`37/final/`、`37/summary.json`。模式提前、零旋转、物理分母及高入口限速的控制分别去掉对应信号，无新生产抽象/转换规则/资产修改。累计74项P2、32项P3；截图0、性能暂停。实耗33分11秒，初版handoff前置失败促成完整producer链路检查；下一项沿pelvis跟随与pose交接继续。

- [x] BF144.38 Ragdoll pelvis跟随、地面探测与恢复朝向：依赖BF144.37记录Gate；沿fresh RefreshRagdolling/RagdollTraceGround/StopRagdolling核对pelvis采样阶段、actor定位、sphere sweep尺寸/响应/零点回退，以及pelvis Rotator对朝上/朝下与恢复yaw的选择。入口/出口pose snapshot、mesh恢复、零scale恢复及完整GetUp动画另项；不复开已登记的许可/模式锁/形状响应差异。Gate：实际native tick/shape设置与坐标换算、完整角色的地面/空中/斜面/缩放/角度边界消费者、最小对照和source身份。开始：2026-10-14 15:18 +08:00；预估结束：2026-10-14 16:03 +08:00；实际结束：2026-10-14 16:10:03 +08:00；偏差：+7分03秒；结果：审计记录Gate PASS，新增2项P2、2项P3，未修复。估时依据：fresh ALS跟随/探测/退出朝向约150行，target follow/trace/rotation约230行；BF144.20场景/路线实耗52分26秒、BF144.37跨阶段实耗33分11秒，本项复用完整角色和碰撞过滤证据，snapshot另拆以控制闭包。
  - 15:18 活动摘要：sphere起终点与MIN_FLOOR_DIST初步逐项一致，尚未将数值或阶段猜测计为缺陷；先读取角色native TickGroup，再验证源actor tick与目标physics后pelvis采样。保留源码坐标基，避免把UE Roll判定直接替成任意Euler分解。
  - 15:48 source/focused阶段：fresh角色CDO确认PrePhysics及暂停不Tick；8组匀速采样6PASS/2FAIL、4组暂停3PASS/1FAIL确认跟随时钟两处遗漏。ground trace的42组中6个30度斜面较解析值高约0.16mm；实际退出99组中3组在roll零点/近pole选择相反起身方向，均先保留候选。读取器新增公开FQuat::Rotator与Sphere SweepQuery原生取样，首轮模块构建及flat=70cm/identity smoke通过，正在用同输入裁定。几何fixture的原点样本曾被physics后的亚纳米漂移改成非零，已将query前输入固定，隔离已证实的采样阶段差异；不放宽容差。
  - 16:04 native/focused闭环：99个原生Rotator逐项确认3个相反facing，36个原生sphere sweep确认6个30度斜面的提前TOI，登记D109–D112。原生Single Sweep的MTD旗标、初始重叠返回start以及半高/半径缩放已由实际caller核对；没有用解析平面代替源数值。新增10组工具自测及旧8组接触恢复自测PASS，旧contact payload逐值保持；零半径按7退出，去掉flat sweep测距的数据消融按预期FAIL。下一步只收口记录与提交，Capsule U01仍须自身源实证。
  - 本项归纳：116份基线source、12份补充UE、3份查询依赖、fresh角色/135组native几何、原始/Build及160份生产Rust/launcher库身份核验PASS。角色与基线仅7个生成GraphGuid及pin identity变化，按一一映射核对完整拓扑与其他payload相同；CDO Tick值直接一致。最终同一probe二进制的4项focused为预期语义FAIL，153组141PASS/12FAIL，证据`38/{clock_final,geometry_final,recovery_final}/`、`38/summary.json`。读取器仅加GeometryQueryProbe公开API取样，旧接触恢复8组与原数据逐值相同；数据消融未改原始导出或源码mtime。无生产实现/转换规则/目标资产修改，累计76项P2、34项P3，截图0、性能暂停。实耗52分03秒；原生数值取样及生成ID身份核对使本轮比估时多7分03秒，下一批复用接口，继续pose snapshot/mesh恢复与GetUp。

- [x] BF144.39 Ragdoll姿态快照与mesh挂接恢复：依赖BF144.38记录Gate；沿fresh Start/Stop、SnapshotFinalRagdollPose及UE SnapshotPose，核对进入时KeepWorld、退出snapshot的骨骼覆盖/局部变换、mesh默认offset/rotation/world scale与pelvis世界姿态保持。恢复朝向数值D111与跟随D109不重复计数；零scale重建及GetUp montage生命周期另项。Gate：fresh原始骨架与offset事实、完整角色实际snapshot/恢复transform消费者、unit/缩放/父空间对照和source身份。开始：2026-10-14 16:18 +08:00；预估结束：2026-10-14 17:03 +08:00；实际结束：2026-10-14 16:49:39 +08:00；偏差：−13分21秒；结果：审计记录Gate PASS，新增3项P2，未修复。估时依据：source SnapshotPose约50行、ALS Stop约100行与Character offset缓存，target capture/restore约180行；BF144.37跨阶段实耗33分11秒、BF144.38含native接口实耗52分03秒，本项复用原生骨架/transform读取与既有角色，不增加未证实的动画算法闭包。
  - 16:18 活动摘要：source snapshot先于关闭physics及移动actor，根骨始终写入，未评估骨骼回退reference pose；退出mesh位置显式使用TransformPositionNoScale，并保持此前world scale再KeepWorld挂回。target恢复保存的local transform，需先核对实际层级和骨架root，不能因函数名或省略root循环就直接判差异。
  - 16:37 source/focused闭环：fresh SKM_Als为68根原始骨骼+11根虚骨，79根均有目标实体；被省略的是GltfNode0和空路径占位，真实root保留，排除漏根骨候选。完整角色24组entry KeepWorld及78根非pelvis源骨骼与mesh容器局部值检查通过；退出offset与世界scale分别确认D113/D114，非均匀缩放的pelvis姿态仍在核对父空间，暂不计入。最初采样hook早于state transition导致前置失败，已调整到实际capture前，未修改生产调度或容差。累计78项P2、34项P3，继续完成本项局部姿态与身份Gate。
  - 16:39 父空间归因：snapshot实际包含real root，按mesh×root×pelvis重建后24组世界姿态全部通过，排除先前漏乘root的夹具候选。该检查直接暴露非均匀角色的root local scale被回读成actor scale的倒数；fresh PhysAnim.cpp:200–206,339–345始终保留动画骨骼scale，而target normalize_scaled_readback_pose:142–145跳过非均匀scale。本项仅沿这个已到达snapshot的producer追加Flail根轨道与实际readback核对，不据此展开其他图或通用物理链路。
  - 16:42 producer闭环：fresh Flail根轨道三个原生采样均identity；实际Ragdoll读回前root scale=1，非均匀(1.4,0.8,0.6)读回后变为(0.7142857,1.25,1.6666666)，登记D115。8组7PASS/1FAIL，unit/0.75/1.5 uniform及关闭readback的控制全部通过；source保持输入骨骼scale的分支不受kinematic/dynamic权重影响。累计79项P2、34项P3，下一步只完成骨骼名称合同、源资产/库身份及最终记录Gate。
  - 本项归纳：116份基线source、6份补充UE、3份animation graph依赖、fresh mesh/Flail/Ragdoll图及原始/Build、160份生产Rust/launcher库身份核验PASS；图原生引用确认Flail与FinalRagdollPose。最终3项focused为1PASS/2预期语义FAIL，33组12PASS/21FAIL；其中mesh位置16处、world scale12处失败，pelvis世界保持24组均通过。证据`39/final/`、`39/summary.json`。最终readback矩阵固定native根scale=1，另保留未固定输入的8组原始反例；中间一次输入前置失败未纳入等价结论。移除多余Arc/Mutex临时状态后信号保持，未新增生产层/转换规则/读取器接口。累计79项P2、34项P3，截图0、性能暂停；零scale恢复及GetUp继续。实耗31分39秒，复用已有读取器与完整角色缩短了骨架和挂接审计。

- [x] BF144.40 Ragdoll零缩放后的物理重建与恢复：依赖BF144.39记录Gate；沿fresh UpdateRigidBodyScaling的coming-from-zero→RecreatePhysicsState→body/constraint初始化，核对恢复时pose、速度、模拟默认值、质量/关节及一次性重建生命周期。进入零scale不停模拟已登记D74，不重复计数；完整GetUp动画另项。Gate：source重建/阈值/默认值合同、完整角色零→非零与非零→非零控制、去掉已知D74影响后的独立反例与source身份。开始：2026-10-14 16:52 +08:00；预估结束：2026-10-14 17:22 +08:00；实际结束：2026-10-14 17:08:35 +08:00；偏差：−13分25秒；结果：审计记录Gate PASS，新增1项P2、1项P3，未修复。估时依据：fresh scaling分支约90行，UE create/destroy/InitBody直接闭包及target rig同步预计约350行；BF144.21初次形状与质量审计包含native工具建设，本项复用其PA记录及BF144.39完整角色，后者实耗31分39秒，缩小到零点恢复生命周期。
  - 16:52 活动摘要：source仅uniform relative scale进入此分支；从近零恢复显式重建整个physics state，再记录新scale，非零→非零只更新已有body/关节。下一步核对ALS所用SetSimulatePhysics与SetAllBodiesSimulatePhysics的默认值差别，再决定实际消费者判据，不能照搬源注释推断所有恢复body都回到Kinematic。累计113项确认差异，截图0、性能暂停。
  - 16:59 source/focused闭环：SetSimulatePhysics保留component级bSimulatePhysics=True，SetAllBodiesSimulatePhysics(False)不改它；fresh BodyInstance初始化因此恢复20个Dynamic+1个Kinematic，目标相符。Source重建销毁旧body/constraint并从actor速度和零角速度初始化；24组恢复矩阵18PASS/6FAIL，含先强制Kinematic并清零以隔离D74的三组独立反例：零期间写入的V/W被目标保留。该写入经source公开BodyInstance setter对Kinematic合法；不把D74产生的旧动量重复计数。继续阈值及native质量/关节控制后登记收口。
  - 17:06 坐标/阈值闭环：near-zero合同在0/5e−5/1e−4三组失败、2e−4/1对照通过；登记D116，独立于D74。fresh 63组body mass及20个constraint记录验证恢复质量/锚点一致；pose检查按source GetBoneTransform重建，18组10PASS/8FAIL，平移mesh到(3,0.0015,−2)后缩到0.75产生约0.901m额外位移，mesh origin归零即消失，登记D117。该source nonzero→nonzero分支也重设body到当前bone transform，故本项仅沿实际缩放调用扩到这个直接控制，不复审已完成求解器。累计80项P2、35项P3，移除暂停下不必要的12帧预热后做最终记录核验。
  - 本项归纳：116份基线source、7份补充UE、fresh 63组body初始化、原始/Build及160份生产Rust/launcher库身份核验PASS；PA完整payload保持，20个constraint与BF144.22逐值一致。最终3项focused为预期语义FAIL，47组30PASS/17FAIL；378组mass及360组关节锚点控制全部通过，body rotation与有限pose检查通过。证据`40/final/`、`40/summary.json`。删除12帧预热后失败边界不变；零写入、非零中间scale和世界原点的控制分别去掉对应信号。无新生产层/转换规则/工具接口，累计80项P2、35项P3，截图0、性能暂停。实耗16分35秒；复用原生body接口及已有坐标合同，使零点恢复闭包早于估时完成，下一批进入完整GetUp montage。

- [x] BF144.41 GetUp montage时钟、通知与结束清理：依赖BF144.40记录Gate；沿fresh Front/Back montage及sequence、Montage_Play/Advance/Stop、ALS locomotion-action与输入解锁notify，核对配置选择、首帧/持续播放时钟、blend-out权重、通知边界、正常结束与中断清理。D66–D69的恢复许可/请求顺序/stance交接不重复计数；最终pose组合、snapshot交叉混合与root motion另项。Gate：fresh原生四个动画资产和当前设置、source时钟/事件合同、完整角色两朝向与倍率/帧步长/中断对照、source身份。开始：2026-10-14 17:12 +08:00；预估结束：2026-10-14 17:57 +08:00；实际结束：2026-10-14 17:50:28 +08:00；偏差：−6分32秒；结果：审计记录Gate PASS，新增2项P2、2项P3。估时依据：target get_up的约350行及事件消费者、UE Montage直接时钟/停止闭包约600行；BF144.14 Roll生命周期实耗约27分钟、BF144.37跨阶段实耗33分11秒，本项复用既有clock/notify取证入口并加入两套montage原生读取，pose另拆以控制本项范围。
  - 17:12 活动摘要：fresh ALS Stop先成功Montage_Play再阻塞输入并设置GettingUp，Front/Back由当前facing选择；target有独立elapsed/clip_time/weight和stop状态，需要逐项对照真实montage通知时点。接下来读取实际montage/sequence，而不从生成JSON反推源配置。累计115项确认差异，截图0、性能暂停。
  - 17:24 source/focused闭环：四个动画和正确CS路径已原生读取，确认两侧montage为1.5秒/1.2倍速、零blend-in、0.3秒Cubic自动混出，动作notify终点1.166567秒。24组时钟矩阵的解锁时点一致；50ms步长出现混出提前一帧候选，另有近零权重的结束边界需核对原生FAlphaBlend。源FMontageSubStepper逐步累加clip position，target用elapsed×rate重算；只读公式不足以确认浮点临界点，下一步给既有读取器增加通用montage时钟探针，直接执行本地引擎UpdateWeight/Advance后再判定。未登记新差异，pose/root-motion仍留下一项。
  - 17:46 source/focused闭环：原生时钟确认D118的50ms混出提前及半速近零权重结束尾差；49.9/50.1ms和动态倍率对照通过。62.5ms独立复现D119：Roll已启动但GetUp读旧状态/旧槽权重；夹具补顺序仅修正停止时点，不能把Graph后才写入的Roll pose提前，保留这个阶段边界。晚期Ragdoll中断还暴露source section末端偏移与target完整长度夹取的D120。通知原生精确终点1.1665666103与清单1.166567之间的夹具失败，仅在内存把manifest恢复原生精度即通过，登记D121；常规40组解锁均通过。动作替换保护和播放器/assembly失效清理通过。接下来合并本批最终证据、复核工具消融和源码身份，无生产修改。
  - 本项归纳：116份基线source、7份UE直接闭包、6个原生资产与raw/Build身份、160份生产Rust及launcher库身份核验PASS。新增通用MontageClockProbe直接调用本机UpdateWeight/Advance；两montage各32组及4组补充轨迹、停止消融/恢复、watchdog与专用self-test通过，未执行原生owner通知回调，通知结论由fresh源码条件和实际target消费者证明。最终6项focused为1PASS/5预期语义FAIL，72组54PASS/18FAIL归并D118–D121；证据`41/final/`、`41/summary.json`。移除公式模拟器后改用原生结果，补顺序与精确manifest只存在于对照夹具；停止请求移除使self-test失败，恢复同一reader二进制后重新读取通过。无生产/转换规则改动，累计82项P2、37项P3；截图0、性能暂停，无公共runtime Gate。实耗38分28秒，原生微边界取证占用额外时间但在估时内；姿态与根运动按原拆分继续。

- [x] BF144.42 GetUp姿态采样、快照交叉混合与根运动边界：依赖BF144.41记录Gate；沿fresh AB_Als的PostLocomotion→Ragdolling直接路径、AB_Als_Ragdolling的Snapshot/GameplayTag Blend、两套GetUp sequence与UE SlotEvaluate/PoseSnapshot，核对采样骨骼/曲线、最终快照进入/退出混合、非加性槽权重与root-motion许可/实际输出。D113–D115快照生产、D118–D121时钟/通知/接替顺序只作已知边界，不重复登记；Head/Overlay/Control Rig的内部算法仍留动画图责任批次。Gate：fresh原生根图/子图及动画姿态、源坐标和混合公式、实际角色槽/图消费者与必要对照、身份核验。开始：2026-10-14 17:54 +08:00；预估结束：2026-10-14 18:39 +08:00；实际结束：2026-10-14 18:36:39 +08:00；偏差：−2分21秒；结果：记录Gate PASS，确认D122–D124，语义仍不等价。估时依据：两个图的直接Slot/LinkedInput/Snapshot/GameplayTag节点、target采样及slot/context消费者约700行；两套fresh sequence均非加性且bEnableRootMotion=False，先以实际标志限制root闭包；BF144.39快照生产实耗31分39秒、.41原生时钟闭包38分28秒，本项复用原生骨骼采样与完整角色入口。
  - 17:54 活动摘要：BF144.41已提交，累计119项确认差异。目标Ragdolling图以0.4秒回到输入姿态，另一个零时长分支在快照与Flail之间切换；下一步重新读取原生根图/子图，验证默认值与连接后才进入求值对照。两套GetUp的root-motion关闭是fresh原生事实，不展开未授权的通用root-motion优化。截图0、性能暂停。
  - 18:13 source/focused闭环：fresh根图/子图确认Snapshot→0.4秒退出混合及PostLocomotion非加性槽；两sequence的RootMotion/ForceRootLock均False。原生16帧×68原始骨骼与实际GetUp槽姿态通过既有3e−5导入容差，root_motion为空；11条montage曲线遗漏已在两朝向实际槽上下文复现，登记D122。快照测试初次未开启骨骼刷新，定位为无渲染夹具的visibility条件；只在夹具启用AlwaysTickPoseAndRefreshBones后六组混合通过，绕开快照的对照明显失败，不登记时序缺陷。下一步只补共槽多姿态旋转混合与曲线消费对照，生产和转换规则未改。
  - 18:33补充闭环：同输入权重下，实际GetUp→SourceSlot的中间归一化导致三方旋转差异，四组失败/十组双姿态控制通过；去掉该归一化后误差恢复约1.2e−7，登记D123。补全通用SequencePoseProbe的GetAnimationPose模式后，当前Sequencer模型的帧间虚拟骨骼差异由实际target及重算对照确认，登记D124；这补上了raw track不含的消费者语义。读取器编译、完整模式self-test、raw请求消融与恢复均已通过预期结果；接下来只做本批完整证据冻结、身份/Gate核验与提交。
  - 本项归纳：116份基线source、10份UE直接闭包、7个源资产raw/Build身份及160份生产Rust/launcher库核验PASS。最终四项focused为1PASS/3预期语义FAIL，56组46PASS/10FAIL；含68骨骼raw与79骨骼完整姿态、16组原生sequence曲线及实际RootMotionOutput、Snapshot六组、真实SourceSlot和producer数据对照。新增通用完整姿态读取分支，编译、front/back/restored self-test及冲突模式拒绝通过；关闭完整采样后self-test按缺曲线失败，恢复同一二进制请求后原生帧数据一致。资产载入临时生成的四类MovieScene Signature经fresh NewGuid源码确认后剔除，其余完整payload/原始文件身份核对通过。夹具仅用内存曲线补全、去掉中间归一化、虚拟骨骼重算和Snapshot旁路作消融；未改生产/转换规则。累计84项P2、38项P3；截图0、性能暂停，无公共runtime Gate。证据`42/final/`与`42/summary.json`。实耗42分39秒；原始track遗漏当前Sequencer虚拟骨骼处理，补原生完整消费者后闭合，仍在估时内。



- [x] BF144.43 U01胶囊sweep数值边界的原生核验：依赖BF144.42记录Gate；回到BF144.07的100m/4m平地箱体、H=0.9/R=0.3m胶囊和落地剩余积分反例，仅补同输入本机Chaos查询及target实际hit/consumer对照。源闭包限于PrimitiveComponent/WorldCollision→GeometryQueries/GJKRaycast与capsule shape adapter；根据结果把U01转为确认差异或排除，不能继续用解析平面当作UE结果。实际convex/trimesh PA接触仍留下一独立项，不借本项复开其他碰撞条目。Gate：fresh源shape/查询旗标、原生同输入输出、4m控制与100m反例、实际target消费、工具self-test/消融及身份核验。开始：2026-10-14 18:38 +08:00；预估结束：2026-10-14 19:13 +08:00；实际结束：2026-10-14 18:55:30 +08:00；偏差：−17分30秒；结果：记录Gate PASS，U01归并为D125，未修复。估时依据：BF144.38已有约80行GeometryQueryProbe和sphere/box原生数据流程；U01已保留12组完整target反例与source调用链，补capsule输入映射、读取器构建和单一消费者约35分钟，参照.38球查询闭包及.42工具扩充的实际耗时。
  - 18:38 活动摘要：BF144.42已提交，累计122项确认差异；U01仍为唯一未解释数值边界。下一步先核对既有原生GeometryQueryProbe是否覆盖capsule及当前CMC的inflation/MTD输入，再生成同输入查询，生产代码不改，截图0、性能暂停。
  - 18:51 source/focused闭环：从真实target Collider和当前重力积分导出四组原生输入；GeometryQueryProbe沿原调用增加capsule形状数据，构建及七样本self-test通过。4m/100m原生结果相同，target三个raw样本不符；将原生距离直接替换BF144.07解析oracle后，12组落地消费六组100m失败、六组4m通过，登记D125。U01将在本项最终记录Gate后关闭；下一步只核对既有sphere工具回归、胶囊高度消融/恢复、完整源码与证据身份，不改生产查询核。
  - 本项归纳：116份基线source、9份UE直接闭包、3份Avian/Parry源码、原始SKM/Build及160份生产Rust/launcher库身份PASS。七组原生胶囊输入、135组既有查询完整输出回归、十组工具self-test与高度退化/恢复通过预期结果；扩展只增加形状数据选择，调用和MTD/厚度保持同一源路径。最终两项focused均按确认语义差异FAIL，16组7PASS/9FAIL，具体为raw四组1PASS/3FAIL、落地消费十二组6PASS/6FAIL。证据`43/final/`、`43/summary.json`。U01完成原生取证后关闭并保留D125；累计85项P2、38项P3，未解释数值项清零不代表剩余模块已审完。生产、转换规则和原始资产均未改；截图0、性能暂停，无公共runtime Gate。实耗17分30秒，复用既有查询入口与消费者夹具使本项早于估时；初次把135组完整回归当作十组self-test输入造成夹具计数失败，改用既有专用请求后通过，不修改测试合同。



- [x] BF144.44 非box接触几何的可达入口与source合同：依赖BF144.43记录Gate；先重新读取PA_Als及实际Playground五类convex、七类trimesh的碰撞源，核对动态body/静态环境几何、simple/complex选择、资产到实际Collider的坐标/scale/flags和运动过滤入口。这里只取运动系统实际接触的数据与可达链，不审场景表现、不凭名称扩展通用物理；接触manifold/三角特征算法在本项确认可达几何后另项执行。Gate：fresh原生碰撞源与target生成数据、实际入口/实例合同、focused最小对照和身份核验，明确后续算法闭包。开始：2026-10-14 18:59 +08:00；预估结束：2026-10-14 19:24 +08:00；实际结束：2026-10-14 19:16:27 +08:00；偏差：−7分33秒；结果：输入合同审计Gate PASS，未新增差异；接触算法未在本项验收。估时依据：既有入口按8类box/5类convex/7类trimesh分类，`playground_collision.rs:195–258`已明确convex从离线hull、trimesh从导入mesh构造；原生contact probe目前仅PA primitive对box。本项先独立完成约13份资产/配置和构造选择闭包，参照.43取证实耗17分30秒估时，避免把未盘清规模的mesh接触求解塞进同一批。
  - 18:59 活动摘要：BF144.43已提交，累计123项确认差异，U01已归并D125。下一步读取源PA和非box资产清单，确认实际几何/过滤与源碰撞复杂度后再决定接触算法矩阵；保持原始资产、生产代码和转换规则不改，截图0、性能暂停。
  - 19:14 source/focused闭环：fresh PA、12类非box资产、原始Playground与PhysicsSettings共15份原生记录PASS；PA是17个capsule/4个box/1个sphere，root的sphere禁止响应，未出现动态凸包。环境5类共15凸包为UseDefault→SimpleAndComplex，另7类为UseComplexAsSimple；10133个源三角形与glTF索引逐项相同，顶点单位/坐标换算最大差1.22µm，凸包数据逐项相同。原生关卡13个实例均Static、无parent、BlockAll/QueryAndPhysics，含Barrel均匀1.5与Rock2非均匀(1.5,1.5,1.75)源scale；实际生产bridge的13组Collider构造/缩放全部PASS，最大局部顶点差3.82µm。两楼梯位置有既有场景导出保留0.01cm造成的0.03333mm舍入，已回读`export_playground_actors.py:136`解释并记录，未当成精确坐标一致或运动缺陷。source convex wrapper的margin、复杂面的背面raycast标志以及mesh/convex接触算法仍留后续；本项不将输入一致冒称为接触等价。证据`44/input_summary.json`、`44/initial/`，下一步完成bridge消融恢复与身份Gate。
  - 19:16 本项归纳：fresh `BodySetupCore::GetCollisionTraceFlag`、`ChaosInterfaceUtils::CreateGeometry`与`CollisionFilter.h:DoCollide`确认simple/complex选择及模拟过滤；`ChaosCooking.cpp:41–43`确认源三角形绕序转换，后续须按实际cooked几何取原生接触结果。移除PlaygroundCollisionPlugin后首个Collider缺失而失败，按原字节恢复并在同一target重编后13组再次PASS。116份基线source、7份额外UE源码、2份Avian依赖、14份原始/Build资产及160份生产Rust/实际启动器库身份全通过；15份native reader报告与最终focused报告均PASS。证据`44/summary.json`、`44/final/`；未改转换器、reader或生产实现，未启动公共Playground Gate。后续拆为实际cooked非box原生接触读取/最小合同，再按差异证据追踪manifold及消费者；背面查询属于单独实际调用边界，不在此推定缺陷。

- [x] BF144.45 实际非box原生接触入口与逐shape合同：依赖BF144.44记录Gate；沿已证实的PA primitive→15个convex hull/7类trimesh链，以源BodySetup/InitBody构造实际cooked几何与scale，扩展现有原生contact probe的静态shape输入，核对margin、有效shape选择和单次manifold的source/target输出。每种有效几何族保留接触/分离对照；跨shape聚合、历史缓存与实际布娃娃动力消费者依据结果另列下一闭包，不预先复开已完成primitive项。Gate：源实际构造/过滤/接触分派证据、原生reader focused self-test及消融、旧primitive报告回归、新矩阵及身份核验。开始：2026-10-14 19:18 +08:00；预估结束：2026-10-14 20:03 +08:00；实际结束：2026-10-14 19:47:43 +08:00；偏差：−15分17秒；结果：审计记录Gate PASS，保留U02待分类几何边界，未新增确认缺陷。估时依据：.44实耗17分27秒已盘清12类资产/13实例；现有contact probe约190行已支持primitive、首帧和二帧缓存，新增范围为第二shape的BodySetup构造及原生几何记录，预计约70组pair而不含全动力求解；参照.35/.36原生接触工具闭环与.43读取扩展实耗17分30秒，独立预留编译、自测和逐shape对照。
  - 19:18 活动摘要：.44已单独提交，累计123项确认差异不变。fresh `BodySetup.cpp:719–746`证明InitBody链先CreatePhysicsMeshes再用Scale/CollisionTraceType/双面标志创建几何；本轮读取实际cooked shape，不用原始顶点手造Chaos凸包或三角网格。下一步补上旧工具无法读取非box的最小请求，再实现通用静态mesh读取和原生控制。
  - 19:33 source/static闭环：旧probe拒绝非box请求（exit 7）已留证；扩展初版虽能构造凸包，但直接把三角网格instance wrapper交给UpdateConstraint触发源ensure（exit 1），该输出不作oracle。fresh `ShapeInstance.cpp:120–164,440–455`及`ParticlePairMidPhase.cpp:207–216`证明实际shape-pair入口用GetLeafGeometry/GetLeafRelativeTransform，三角网格解除instance、凸包保留margin；已直接调用该原生接口，重编后12组接触/分离self-test PASS。98组原生记录含96组主矩阵和2组整个mesh上方的分离补充；原矩阵两组楼梯上移30cm仍接触更高台阶，属于位置前提，保留原始结果而未误报。source单个mesh约束最多14点，source默认simple/complex选择、native过滤和cooked/scale路径有效；下一步用实际PA Collider及生产Playground bridge输出比较逐shape的manifold，未修改生产实现。
  - 19:47 本项归纳：98组actual Collider逐shape合同54PASS/44预期FAIL，接触/分离状态98组全部一致；convex为37PASS/27FAIL，trimesh为17PASS/17FAIL，15组仅点数不同，均保留到U02而未直接计作运动缺陷。原有54组单帧、28组历史接触payload保持相同，7组旧primitive self-test PASS；旧33整包仅差此前已加入的SimpleAssignContact记录，当前PA metadata与.44逐项一致。GetLeaf消融使三角网格原生ensure并以1退出，自测拒绝该输出；按源码原字节恢复、同一package重编后12组self-test PASS且整包与消融前一致。恢复构建尚未完成时一次提前读取触发AutomationTool互斥，已保留失败日志，等待原构建正常成功后读取通过，没有重启生产入口。116份source、10份额外UE/4份依赖、14份原始/Build资产、160份Rust及启动器库身份核验PASS。证据`45/summary.json`、`45/final/`；只改原生读取工具及其自测，未改转换器/生产Rust/资产，未启动公共runtime Gate。后续依次核对convex支持面减少与trimesh特征/聚合的实际消费者，再闭合历史mesh接触。

- [x] BF144.46 非box凸包支持面与实际动力消费：依赖BF144.45记录Gate，只追U02中5类convex/15个hull的支持点减少和重复点；先核对源完整polygon裁剪与target首4点/GJK补点，再区分被零冲量淘汰的几何差异和影响实际normal/restitution/rotation的边界。复用已取证且身份不变的native输入，保持PA形状与生产Playground凸包构造，受控替换仅在focused fixture内；trimesh与历史mesh缓存留下一项。Gate：源支持特征→manifold→solver闭包、原生输入/重复点对照、实际消费者最小反例与消融、确认/归并/排除和身份核验。开始：2026-10-14 19:50 +08:00；预估结束：2026-10-14 20:30 +08:00；实际结束：2026-10-14 20:50:01 +08:00；偏差：+20分01秒；结果：审计记录Gate PASS，新增D126/P2，U03单独拆分。估时依据：.45已有64组convex（32接触/32分离）中27组raw差异及完整source；.34同类支持点消费实耗13分02秒，当前增加n边形面截断与多hull逐项分类，按现有真实PA/solver夹具独立估时，不重建已验证的reader。
  - 19:50 活动摘要：.45已单独提交，U02保留，123项确认差异不变。Barrel最小平面接触已经证明源head两点/foot四点而target仅一点；fresh Parry `ConvexPolyhedron::local_support_feature`取支持面前4顶点，当前只是候选根因。下一步用实际支持面几何定位截断，并进入真实角色创建的contact constraint，排除已有点序/质量加权/柔性求解差异的干扰。
  - 19:59 source/consumer闭环：Barrel实际顶部是8边形，Parry支持特征取前4顶点后，源head两处及foot四处接触位置全部落在被丢掉的区域；同位置四边形面控制全部保留。fresh UE完整面/线段裁剪与32点中间buffer→最后减到4点的顺序已核对，Native AveragePointEnabled=false与既有两轮速度公式一致。实际角色的头部/足部、scale=1/1.5、帧长1/60与1/30共16组中，原target八组失败，仅在夹具换成同输入native支持点的八组通过；源式预算、硬接触、零摩擦、禁用关节及源点序排序共同排除已知差异干扰。确认D126/P2，累计124项有效差异；下一步完成其他convex样本的净pushout/重复点分类与本项Gate，trimesh仍保留U02。
  - 20:50 完成与超时复盘：原计划结束20:30，最终runtime于20:48:44完成，身份记录Gate于20:50:01完成，总实耗60分01秒，较容差上沿约晚1秒。漂移来自将raw集合接入实际窄相位后补齐过滤/裁点，以及默认足部惯量对照暴露了矩阵阶段混用；后者另立U03，不沿用本项的常值矩阵oracle作结论。本项最终6项focused为3PASS/3预期FAIL，61组48PASS/13FAIL；27份原生样本移除36个零pushout支持点后位置响应均不变，拱门额外GJK点经实际窄相位裁掉后的四组回弹控制通过，拱门/长凳四组实际穿透支持控制也通过，排除未先走真实过滤/裁点的简化夹具假象。保留实际head惯量的源几何对照另有1PASS/1FAIL，确认D126并不依赖人为统一惯量。五边形拱面与破桶的raw影响保留与D126的归并边界，trimesh仍留U02。删除无信号GJK移除分支及不适用于部分接触的通用回弹oracle扩展；最终同target重编复验，所有原始失败证据保留。116份source、5份UE/6份依赖、14份原始/Build资产、160份Rust、reader与实际启动器库身份PASS；证据`46/summary.json`、`46/final/`。仅登记差异，未改生产/转换器/读取工具；累计86项P2、38项P3。后续先独立完成惯量准备/消费阶段，再继续U02；不再启动本项runtime。

- [x] BF144.47 接触惯量准备、积分与速度消费的阶段合同：依赖BF144.46记录Gate，沿U03实际foot对照的cached effective_mass与当前world inverse inertia差异，核对UE gather/position/velocity以及Avian prepare/substep/restitution各阶段的矩阵、力臂和刷新触发。先捕获准备阶段张量，区分oracle时间点错误与目标新旧数据混用；只在受控夹具恢复已取证阶段值，不改生产求解器，不扩展相邻已完成关节/质量规则。Gate：fresh source阶段链、实际角色前后输入快照、最小反例/控制与消融、确认/排除和全身份核验。开始：2026-10-14 20:53 +08:00；预估结束：2026-10-14 21:28 +08:00；实际结束：2026-10-14 21:07:28 +08:00；偏差：−20分32秒；结果：审计记录Gate PASS，新增D127/P3并关闭U03。估时依据：.46已给出零力臂误差但约0.26%的有效质量阶段差，原生几何与真实constraint夹具均已可用；本项读取约5个source/target阶段入口并增加prepare快照，参照.34消费审计13分02秒和.46惯量诊断，独立估时35分钟，避免与trimesh特征审计混合。
  - 20:53 活动摘要：.46已提交，D126累计124项有效差异；U03尚未计为缺陷。已观察实际foot的cached质量约0.669501与消费时重算0.667765/0.671513不一致，源ContactNormalAngular为保存字段；下一步核对实际启用的scalar/SIMD生命周期并捕获初始tensor，再做同输入恢复对照。
  - 21:05 source/consumer闭环：原生SolverType=−1与fresh构造函数共同确认实际为Gauss–Seidel；源FinalizeManifold同时缓存有效质量和InvI×RxN，位置修正只累计DQ，速度阶段沿用同一角响应，shock传播缩放也同时刷新二者。Avian接触准备缓存质量，积分后刷新SolverBodyInertia，回弹按新矩阵施加旧质量冲量。真实foot的prepare矩阵与四处cached质量逐项一致，post矩阵变化；16组头/足、60/30Hz、原生/各向同性、恢复/保留中14PASS/2FAIL，两个默认足部失败仅恢复prepare矩阵即通过，实际输出也逐项符合新旧混用公式。确认D127/P3，U03已定位；补核当前六子步消费并删除夹具无用的多hull/几何选择分支后完成记录Gate。
  - 21:07 完成摘要：最终32组28PASS/4预期FAIL，四个失败全部为实际foot惯量且包含默认六子步；所有prepare矩阵重算质量均与缓存相同，原始输出符合混用公式，恢复对照均通过。移除无用几何分支后原16组结果逐字相同；同target重编及116份source、8份额外UE/5份依赖、14份资产、160份Rust、reader与启动器库身份Gate PASS。记录工具首次经第二层PowerShell传数组丢失脚本参数，watchdog在60秒自动结束；改为当前shell直接传数组，并修正首行测试名前缀的日志计数后PASS，失败日志保留，未重跑生产或扩大测试。证据`47/summary.json`、`47/final/`、`47/record_gate_final.watchdog.json`。总实耗14分28秒，未超时；累计125项有效差异（86项P2、39项P3）。下一步回到U02的剩余凸面与trimesh动力消费者，生产/转换器/读取工具未改，公共runtime未启动。

- [x] BF144.48 剩余凸包裁剪与逐shape位置消费：依赖BF144.47记录Gate，继续U02中破桶和五边形拱面等已取证convex差异，核对实际窄相位过滤、部分支持点的源位置/速度激活及多hull生产Collider输入；实际聚合生命周期据本项新证据拆到下一项。只沿已有27组raw差异及其直接消费者归并D126、排除无效点或确认独立原因；trimesh和历史mesh缓存留独立下一闭包。Gate：fresh完整凸面/接触激活/聚合source链、真实PA和生产凸包的最小反例与控制、必要夹具消融及身份核验。开始：2026-10-14 21:09 +08:00；预估结束：2026-10-14 21:54 +08:00；实际结束：2026-10-14 21:39:58 +08:00；偏差：−14分02秒；结果：审计记录Gate PASS，新增D128/D129，聚合生命周期拆到下一项。估时依据：.46已核对27组源零pushout、四组真实裁点并确认八边形D126；.47用14分28秒闭合惯量前提。剩余主要为5边形/破桶的部分接触和多hull聚合，已有原生几何无需扩展reader；参照.46实耗60分01秒扣除已闭合的特征截断、阶段混用与过滤入口，按45分钟独立验收。
  - 21:09 活动摘要：.47已提交，累计125项有效差异，U03归并D127。此前将部分接触送入全点回弹公式的假设不成立，本项先核对源位置净pushout与速度激活/下限，再使用实际过滤后的constraint；不沿用raw点数直接判断动力差异。
  - 21:36 source/consumer闭环与拆分：原生读取补充15个cooked convex的未缩放顶点、面平面/索引和包装scale；96组旧payload除新增字段外逐项相同，截断面或丢scale的派生数据均被self-test拒绝。源拱门本身也含非共面四边形；fresh UE优先选择Convex2参考面并统一投影，target把面内投影与原始边插值混用，后者逐点预测实际深度至约2.2e−8m，源参考面公式匹配native至1e−10m。两处约0.535mm深度差进入实际首次位置求解，确认D128/P3；平面控制通过。另确认head三点中额外GJK点使修正量变化，仅删该点的60/30Hz控制恢复，登记D129/P3；破桶两组归D126。46组剩余逐shape位置消费为41PASS/5FAIL，4组GJK控制为2PASS/2FAIL；所有23组native几何替换通过，其他raw差异未在此首轮位置消费者形成超容差偏差。114组新增同姿态跨hull原生输入覆盖26姿态，其中24姿态同时有多个hull；逐pair读取还不能证明源聚合/裁点结果，将该独立生命周期拆为下一项，当前完成范围收敛为逐shape几何与首次位置消费，未缩减整个审计目标。下一步完成本项身份Gate及reader固定fixture，然后继续聚合。
  - 21:39 完成摘要：3项focused为1PASS/2预期FAIL，50组位置消费及GJK控制43PASS/7FAIL；去掉夹具无用惯量开关和回弹设置后，原46组结果逐字相同。96组完整native回归、114组聚合输入、16组固定reader fixture通过；截面/丢scale消融失败、原始数据复验通过。首次读取未拆缩放包装的断言、夹具Rotation类型及记录脚本误指已移除HostProject目录均留存失败证据，按实际API/类型/打包路径修正后通过。116份source、7份额外UE/5份依赖、14份资产、160份Rust、reader源码/package/DLL及实际启动器库身份核验PASS。证据`48/summary.json`、`48/final/`、`48/record_gate_final.watchdog.json`；新增reader通用cooked凸包拓扑读取及self-test/fixture，没有转换器或生产修改，不触发AnimBP重转换。总实耗30分58秒，未超时；累计127项有效差异（86项P2、41项P3）。U02的多hull聚合、trimesh与历史mesh继续保留；公共runtime未启动，截图0、性能暂停。
  - 有限复核完成：开始2026-10-14 22:31 +08:00；预估结束22:47；实际记录Gate结束22:45:52；偏差−1分08秒，PASS。BF144.50证明源静态接触应使用动态shape表面点作为force point；旧中点参考仅在其差向量平行normal时成立。改为原生world_point_0并在接触切平面匹配源点序后，46组仍为41PASS/5FAIL、23组原生控制全PASS，D126/D128/D129分类不变；D128数值已更新。只复核消费参考，拓扑、原生点、读取工具和其余已闭合证据不变。证据`50/reopen_final/`、`50/summary.json`；旧中点消费数值由本次结果替代。

- [x] BF144.49 多hull接触集合与源聚合生命周期：依赖BF144.48记录Gate，沿已记录114组同姿态pair输入与24个多hull姿态，核对源midphase→manifold聚合/内部边裁剪→有效constraint以及目标compound→ContactGraph的实际集合和消费。复用原生cooked几何及PA输入，必要时只扩展读取工具的统一聚合入口；不把逐shape输出相加冒充最终集合。Gate：fresh源聚合/过滤链、单hull与多hull原生对照、实际目标集合/最小消费控制、reader与夹具自测/消融、归并或确认及身份核验。开始：2026-10-14 21:42 +08:00；预估结束：2026-10-14 22:27 +08:00；实际结束：2026-10-14 22:10:14 +08:00；偏差：−16分46秒；结果：审计记录Gate PASS，聚合集合闭合，新增侧面几何样本拆分到下一项，确认差异数不变。估时依据：.48实耗30分58秒完成cooked拓扑读取及50组消费；当前15个源凸包与114组逐pair输入已具备，剩余为一个聚合生命周期入口及26姿态的集合合同，按45分钟独立验收。trimesh特征/聚合和历史mesh缓存继续留后续独立闭包。
  - 21:42 活动摘要：.48已提交，累计127项有效差异；原生拓扑与source/target逐shape几何已可复用。26姿态中24个同时触及多个源hull，尚未确认源是否跨这些pair裁掉内部边/重复支持。本项先定位实际聚合阶段与读取入口，再构造完整compound对照；生产实现不改，截图0、性能暂停。
  - 21:59 source/native闭环：42处PA平滑边属性均False；fresh MidPhase按有效shape pair建立约束，EndDetect后跨mesh/edge/subsurface裁剪仅对启用该标志的dynamic执行。读取工具沿资产完整root geometry及原shape碰撞过滤进入BasicBroadPhase→原生MidPhase→EndDetect，32组通过；26姿态的最终集合与.48全部对应pair逐点相同，24组含多个hull，4个单hull与2个分离控制通过。未用pair求和代替本次原生证据。下一步核对生产compound实际集合和多约束首次位置消费；新增读取入口的回归、自测及消融尚待完成，未登记新差异。
  - 22:08 target/consumer闭环：Parry完整compound与单叶compound控制32组全部通过，参与接触的hull集合与native一致；直接拆成裸凸包曾有10组胶囊点数不等，fresh dispatcher显示compound额外往返相对变换，保留同一包装后全部恢复，未据此登记聚合缺陷。真实角色多约束首次位置消费60组42PASS/18FAIL，30组native几何控制全PASS；其中桶类4组和Arch01顶部head归既有D126/D129，其余13组涉及新增的相邻hull侧面/内部接触，不能直接套用此前只对齐顶部的几何结论，保留U02分类。7组读取fixture含平移/旋转、scale、单/多hull及分离；原生96组旧payload逐字相同。下一步完成本项自测/消融和身份Gate，剩余侧面几何独立拆分，trimesh及历史mesh仍待后续。
  - 22:10 完成摘要：最终2项focused为1PASS/1预期FAIL，32组集合PASS、60组消费42PASS/18FAIL；移除无用回弹设置后60行结果逐字相同。读取self-test PASS，仅保留选中hull、重复约束、移除world变换的三种数据消融均被拒绝，原始结果复验通过；旋转控制暴露的约2微厘米浮点差已与直接/native世界点分别核对，不作为几何缺陷。首版原生指针类型和Rust lifetime编译错误保留后修正，未改变生产。116份source、15份额外UE/7份依赖、14份资产、160份Rust、reader源码/package/DLL与实际启动器库身份Gate PASS；证据`49/summary.json`、`49/final/`、`49/record_gate.watchdog.json`。实耗28分14秒，未超时；累计127项确认差异不变。只新增通用原生聚合读取与固定自测，没有转换器修改；公共runtime未启动，截图0、性能暂停。
  - 有限复核完成：开始2026-10-14 22:31 +08:00；预估结束22:47；实际记录Gate结束22:45:52；偏差−1分08秒，PASS。聚合集合及原生pair证据不变；按BF144.50的native动态表面force point和切平面点序匹配重做60组，仍为42PASS/18FAIL、30组原生控制全PASS。旧中点消费数值由`50/aggregate_final/`替代，六个胶囊侧面原因由D130闭合，剩余箱体按U02继续分类；生产和读取工具未改。

- [x] BF144.50 胶囊侧面支持与源受力点复核：依赖BF144.49记录Gate，沿U02的13个混合失败姿态及26个有效shape pair，保留compound包装后逐叶隔离，完成胶囊端帽/圆柱支持和正确force point消费；box参考平面/完整特征依据本项新证据拆至下一项。复用已取证native完整集合、cooked拓扑与实际PA消费者，未复开聚合算法。Gate：fresh对应叶子算法/坐标闭包、逐叶消费者与原生几何控制、最小根因消融和身份核验。开始：2026-10-14 22:12 +08:00；预估结束：2026-10-14 22:47 +08:00；实际结束：2026-10-14 22:45:52 +08:00；偏差：−1分08秒；结果：审计记录Gate PASS，确认D130/P2，完成BF144.48/49有限复核，六个箱体样本拆分。估时依据：.48实耗30分58秒、.49实耗28分14秒，当前26个pair已具备原生点和拓扑，不需扩展reader；按35分钟独立验收。trimesh和历史mesh仍为后续闭包。
  - 22:12 活动摘要：.49已单独提交，127项确认差异不变。原生聚合与逐pair完全相同，完整compound与同包装单叶集合也相同；13处未分类失败来自相邻侧面/内部几何，当前先隔离主/邻hull，避免把已知顶部差异与新接触混作一个原因。截图0、性能暂停，生产不改。
  - 22:23 source/胶囊闭环：逐叶52组38PASS/14FAIL，26组原生几何控制全部通过；其中6个head斜面case的native支持面均为三角形，排除D126截面。fresh UE端帽分支从圆柱表面裁剪辅助点，Parry先裁胶囊轴线再沿GJK法向统一加radius，导致辅助点距真实胶囊表面内陷0.56–19.09mm，满足独立半径公式。保留实际GJK点与法向、只替换圆柱辅助点的60/30Hz共24组12PASS/12预期FAIL，所有替换控制通过，确认D130/P2；累计128项（87项P2、41项P3）。足部剩余侧面样本继续核对raw裁剪与实际裁点，不把几何数量差直接计作新缺陷。
  - 22:31 oracle复盘与有限复开：侧面quad/triangle中出现支持点几何仅有微差、角响应却改变的症状；fresh `PBDCollisionContainerSolver.cpp:95–103`再次确认静态环境的源force point是动态shape表面点。源凸面正交投影方向R可能不同于GJK法向N，因此原先“中点沿N平移不影响法向力矩”的前提不能泛化。BF144.48/49仅复开消费oracle及D128数值范围，原生拓扑、聚合集合、D126/D129独立根因保留；先把本项参考点改为native world_point_0，再用相同实际消费者复核。D130六组三角形斜面支持点确在胶囊内部，几何根因不受此修正影响。未开始新的生产runtime。
  - 22:45 完成摘要：正确force point下，旧凸包46组41PASS/5FAIL、逐叶52组38PASS/14FAIL、完整compound60组42PASS/18FAIL、胶囊根因控制24组12PASS/12FAIL；91组原生/根因替换控制全部通过，四批共用同一最终二进制。删除单叶夹具无效的跨manifold排序和graph扩缩，单约束断言覆盖26组；D130保留实际GJK点与法向的最小替换仍通过。棱边候选发现GJK位置还存在独立差别，初版“只删辅助点”的前提断言失败，保留到下一项而不预报缺陷。116份source、5份额外UE/10份依赖、14份资产、160份Rust、reader源码/package/DLL及启动器库身份Gate PASS；证据`50/summary.json`、`50/{reopen,leaf,aggregate,capsule}_final/`。实耗33分52秒，未超时；累计128项（87项P2、41项P3）。六个未分类box侧面样本及原生参考面/裁点/棱边投影链按新证据拆分，trimesh与历史mesh继续保留；无生产、转换器或reader修改，截图0、性能暂停。

- [x] BF144.51 剩余box侧面参考面与棱边接触：依赖BF144.50记录Gate，只追`50/summary.json`的六个foot_l侧面pair。沿已取证raw→过滤/裁点→正确force point链，核对源参考面裁剪/正交投影、额外GJK点及棱边接触选择/重定位，分别归并已有差异或确认独立原因；必要时只补原生约束类型/margin读取，不改转换器或生产实现。Gate：六组对应source算法与原生分支证据、实际消费者的最小分离控制、读取或夹具自测/消融及身份核验。开始：2026-10-14 22:53 +08:00；预估结束：2026-10-14 23:23 +08:00；实际结束：2026-10-14 23:19:34 +08:00；偏差：−3分26秒；结果：审计记录Gate PASS，确认D131/P2、D132/P3，六box分类闭合。估时依据：.50实耗33分52秒已完成26个pair、182组消费及受力点纠正，剩余六组的raw点/完整面/原生点均已具备；当前只含box参考面/裁点和一个棱边分支，按30分钟独立验收。trimesh与历史mesh继续留后续闭包。
  - 22:53 活动摘要：D130及BF144.48/49有限复核已分别提交，累计128项确认差异。当前六组失败均使用正确源动态表面force point，原生几何控制通过；Arch02侧面候选的“只删辅助点”控制因GJK位置不一致而未成立，不能据此直接登记新缺陷。先读取实际constraint分支/margin，再分离几何与裁点原因；不复开已闭合的胶囊或聚合集合。
  - 23:01 source/native闭环：统一读取入口新增实际contact type及constraint两侧margin；旧报告因字段缺失被self-test拒绝，原生重编后8组固定fixture通过。六个box的实际margin均为(0.6cm,0)，五个VertexPlane=1，一个EdgeEdge=2；胶囊水平/斜面控制分别为PlaneVertex=3/VertexPlane=1，margin=(9cm,0)。fresh `InitMarginsAndTolerances`证明静态凸包自身margin不能替代有效constraint margin；`CollisionOneShotManifolds.cpp:914–969`的棱边分支在正margin时将点投影到最近棱边并只保留一处，解释此前GJK位置不相同。下一步在实际消费中分离辅助点删除与源棱边force point，再核对其余五个box面接触；未新增确认缺陷。
  - 23:12 source/focused闭环：96组逐叶和32组完整聚合重新导出，递归去掉新增的三种metadata字段后与旧native结果完全相同；删除type或margin的消融均被self-test拒绝，恢复通过。棱边2帧长×删除辅助点/恢复源force point的四组合为2PASS/6FAIL，仅同时恢复两项通过，确认D131；既有normal/phi保持实际值。其余五个面接触的原生参考平面已逐点验证投影深度，继续在相同collider顺序及compound包装下分离裁点与投影。
  - 23:18 面接触闭环：相同collider顺序/compound/COM的raw重建与实际ContactGraph全部相符；五姿态×删除GJK/恢复源参考深度的20组仍FAIL，排除仅靠裁点或深度的解释。保留目标实际支持多边形、GJK法向和统一源深度，直接使用同一Parry公开裁剪函数，只将裁剪轴N改为原生参考面R，5组全部恢复；保持N的5组仍FAIL，确认D132/P3。Arch01 hull 4对3的第五个GJK触发深度加权裁点，丢失一个角点，导致z角修正从源+0.0617变为−0.4318rad；删除后剩余差异由参考深度/裁剪方向解释，归并D129而不重复编号。下一步精简夹具并同target复编，核验最终矩阵及文件身份；六box已具备根因分类。
  - 23:19 完成摘要：删除夹具未使用的整组source geometry替换分支，同target重编后此前38组输出逐字相同；裁剪矩阵增加30Hz后，最终48组12PASS/36预期FAIL，12组最小根因恢复控制全部通过。reader仅新增5行通用metadata读取，8组固定fixture和128组原生回归通过，丢字段消融失败且恢复通过；116份source、5份额外UE/5份依赖、14份资产、160份Rust、reader源码/package/DLL及启动器库身份PASS。证据`51/summary.json`、`51/final/`、`51/record_gate.watchdog.json`。实耗26分34秒，未超时；D129因已证实box丢角点导致约0.49rad角修正差提升P2，累计130项（89项P2、41项P3）。无生产或转换器修改，不触发AnimBP重转换；U02只余trimesh和历史mesh，公共runtime未启动，截图0、性能暂停。

- [x] BF144.52 三角网格当前帧接触与首次位置分类：依赖BF144.51记录Gate，只沿U02已取证的32组trimesh输入（18组有接触）及实际PA head/foot，核对源capsule/convex-triangle特征、法向、去重/聚合/裁点和实际消费者。保留原生产Playground网格、同一原生force point、质量/惯量及源顺序；将已证实同根原因归并，独立差异另行登记。历史manifold/摩擦保留单独闭包，不混入当前帧输入。Gate：fresh实际源分支、原生完整集合、实际约束及原生几何对照、最小根因消融与身份核验。开始：2026-10-14 23:22 +08:00；预估结束：2026-10-15 00:07 +08:00；实际结束：2026-10-15 00:11:39 +08:00；偏差：+4分39秒；结果：审计记录Gate PASS，确认D133/P2、D134/P3、D135/P2，Cart胶囊支持按fresh分支拆分。估时依据：.49聚合实耗28分14秒、.50混合逐叶实耗33分52秒、.51分离控制实耗26分34秒；当前32组已具备原生point/type与生产几何，但trimesh单约束可有不同法向及最多14点，需补对照的全局点序而非沿用单normal manifold，按45分钟独立验收。
  - 23:22 活动摘要：.51已提交，读取工具及生产树身份稳定。当前32样本含7种trimesh、Rock的1.75倍缩放及14组空接触控制；不将45批次raw数量/法向差直接判成缺陷。先沿本机Capsule/ConvexTriMesh调用及Parry triangle dispatcher重新取证，再使用实际约束逐点消费；截图0、性能暂停。
  - 23:37 source/focused闭环：实际cvar确认capsule2、convex mode=1和optimized mesh loop启用，额外排序关闭；32组直接/完整聚合原生数组相同，点数据与.51回归逐字一致。36组首次位置消费27PASS/9FAIL，18组native geometry控制全PASS；Fountain/楼梯head先分离同根GJK，Cart及楼梯foot继续追法向/共享特征。全正间距控制最初错误向无碰撞island的pair插入约束，触发夹具自身panic；仅跳过无位置激活可能的替换后全矩阵完成，不计产品缺陷。native face index证实cooked编号不等于原始provider编号，旧直接索引诊断不作通过证据；按本机TriangleMesh API补读取完整cooked顶点/拓扑，未改转换或生产几何。
  - 23:53 分类进展：Fountain及三种Stairs的head在60/30Hz只删除GJK后全部通过，归D129；Cart仍未通过。fresh cooked topology将34个已用face与native静态表面逐项对应，最大平面残差约7.1e−8m，旧raw编号诊断已替换；新增读取API的signed-size编译错误按实际uint32/GetX修正，重编及12组self-test通过。三种楼梯共享cooked边75/76；只删除该边上重复立面接触使02/03两帧长全部通过，确认D133/P2，累计131项。01还存在朝背面的下向接触，Cart继续核对SAT三角面偏置/胶囊面法向；不把未闭合部分归为新缺陷。
  - 00:05 source/分离控制闭环：Cart脚部的原生triangle988偏置条件成立：三角面与最大edge轴分离差约68.4µm，小于源0.01cm偏置；目标实际选择该edge轴。保留实际动态表面和深度，只恢复同一目标三角面的法向即通过，确认D134/P3。Stairs01的背面假设被原始triangle归属否定：额外向下法向来自两个+Z共面三角形之间的内部对角线，源邻面点积1、接触点积0，差1超过实际0.7剔除阈值。内部法向剔除/共享边去重四组合在两帧长均仅同时恢复通过，确认D135/P2；两个分别修改ContactGraph的控制曾触发夹具调度歧义，合成一个有序系统后通过。32+12原生新旧输出除新增topology外逐字相同，四种metadata消融拒绝、原始数据恢复通过。
  - 拆分边界：本项完成32组原生集合、18组首次位置消费及九处失败中的八处分类；Cart head在去GJK、恢复三角面法向后仍失败。fresh `CapsuleTriangleContactPoint.cpp:47–464`进入独立端帽/圆柱/逐边生成规则，当前已确认的法向与共享边规则不足以解释其支持点集合，拆为下一独立审计项继续；trimesh有速度/摩擦及历史仍保留U02。本项不把这些未验证输入算作通过。
  - 00:11 完成摘要：精简无效的面法向替换及foot GJK组合，合并有先后依赖的几何控制后同target重编；原position/capsule/shared-edge/internal-edge共76组输出逐字相同，Cart最小偏置控制在两帧长均通过。最终80组43PASS/37预期FAIL，18组原生参考（含全正间距空响应）及16组完整根因恢复通过；九处原始位置失败仅Cart head待下一项。32组完整native及12组固定fixture回归、自测与四种metadata消融通过，116份source、14份额外UE/10份依赖、14份资产、160份Rust、reader源码/package/DLL及启动器库身份PASS。证据`52/summary.json`、`52/final/`、`52/record_gate.watchdog.json`。实耗49分39秒，偏差在允许窗口内；累计133项有效差异（91项P2、42项P3）。新增通用reader的算法开关、face index及cooked mesh拓扑取证，没有生产或转换器修改，不触发AnimBP重转换；截图0、性能暂停。

- [x] BF144.53 Cart胶囊薄条面支持点：依赖BF144.52记录Gate，仅追`SM_Als_Cart_shape_0_head_scale_1_gap_-0.1`剩余首次位置差异，沿fresh CapsuleTriangle2端帽、圆柱、逐边接触和网格特征归并核对原生4个三角面；复用Fountain/Rock已有姿态作控制。已证实的D129、D133–D135不复开，不改生产或转换器。必要时只在统一reader中调用同一原生逐三角面函数，读取聚合前集合，禁止用猜测补写原生点。Gate：同一实际capsule/mesh的逐面原生集合、聚合前后归属、实际消费者最小控制、工具自测/消融和身份核验。开始：2026-10-15 00:15 +08:00；预估结束：2026-10-15 00:45 +08:00；实际结束：2026-10-15 00:36:35 +08:00；偏差：−8分25秒；结果：审计记录Gate PASS，新增D136/P2，未修复生产差异。估时依据：.51六box分离实耗26分34秒，.52含拓扑补读/32输入/80消费实耗49分39秒；本项只剩一个姿态、4个三角面，已有cooked拓扑和失败矩阵，专用CapsuleTriangle2约420行闭包，按30分钟独立验收。速度/摩擦/历史及动画输入另项继续。
  - 00:15 活动摘要：.52已提交，累计133项；Cart head去GJK及恢复三角面法向的控制仍FAIL，不能归为已完成原因。当前原生6点分属988/989/491/498，目标对应149/150/542/911；后两面近水平且点位控制正常，薄条面支持集合仍需区分端帽/边交叉及聚合删除。先用现有原生API补聚合前取证；截图0、性能暂停。
  - 00:26 source/工具闭环：fresh CapsuleTriangle2及MeshContactGenerator调用链确认全部逐面接触先在胶囊空间生成，再归并/变换；统一reader增加可选triangle_indices，直接调用公开原生函数，两侧world anchor均经胶囊变换。缺失字段自测已稳定失败；FTransform与Chaos刚体变换类型差别导致首次编译失败，按源构造接口修正后编译通过。当前一次读取44组旧输入及Cart/Fountain/Rock两种缩放4组新输入，随后核对旧结果和薄条面点集；没有生产或转换器修改。
  - 00:35 source/consumer闭环：44组旧native完整JSON一致；4组新增输入与未开选项时的points/aggregate结果一致，Cart四面生成前后均为2/2/1/1点，排除网格归并删除。fresh端帽/最近边条件逐项复算精确命中原生：988只留端帽，989只留一个穿越边，另两个边候选的有符号DotFace约−7.54cm而被拒绝。32组thin分离矩阵仅支持点选择+法线+表面位置同时恢复通过，深度替换无必要；删除该分支及未使用的全量native注入后同target重编，保留28组结果逐字不变，12PASS/16预期FAIL。确认D136/P2，累计134项（92项P2、42项P3）；Fountain已知D129及Rock两缩放控制保持原信号。正在核验最终身份与记录Gate。
  - 00:36 完成摘要：原生读取/两种工具自测、四种metadata消融、条件复算、最小消费者及身份记录Gate通过。116份source、10份本项额外source/依赖与上一轮24份闭包、14份资产、160份Rust、reader源码/package/DLL和实际启动器库身份一致；证据`53/summary.json`、`53/final/`、`53/record_gate.watchdog.json`。实耗21分35秒，提前8分25秒完成；本轮新增统一reader可选逐面取证与自测，不涉及转换器或155图重转换。剩余U02速度/摩擦及历史接触另项继续；截图0、性能暂停。

- [x] BF144.54 三角网格接触的速度与摩擦消费：依赖BF144.53记录Gate，仅沿U02已取证的PA capsule/box和trimesh集合，比较fresh碰撞gather→法向/切向速度阶段。固定已知点序、迭代预算、共同质量/惯量及接触几何作分离控制，区分既有D91–D102和D129/D133–D136；历史匹配/保留接触另项继续，不扩展通用物理或生产修复。Gate：source阶段/公式合同、实际prepared contact消费、源几何控制与最小反例/消融、身份及剩余边界。开始：2026-10-15 00:41 +08:00；预估结束：2026-10-15 01:21 +08:00；实际结束：2026-10-15 01:06:02 +08:00；偏差：−14分58秒；结果：共同几何的速度摩擦审计记录Gate PASS，新增D137/P2与D138/P3，未修复。估时依据：fresh `PBDCollisionContainerSolver`及`PBDCollisionSolver`直接闭包约350行、Avian normal/tangent约250行，复用.52/.53原生集合和实际消费者；.29摩擦实耗26分48秒、.53逐面分离实耗21分35秒，本项增加多点与不同法向，独立估时40分钟。
  - 00:41 活动摘要：.53已提交，累计134项。当前native Use1DFriction=False、AveragePointEnabled=False；源先取单点法向/切向速度误差再合并施加，目标先完成法向再读取切向速度，尚未将此静态差别计为缺陷。先用共同原生几何和零摩擦对照隔离输入，随后追必要的实际摩擦消费；截图0、性能暂停。
  - 00:46 夹具核验：首轮40组中，10个全负phi姿态的共同原生几何/零摩擦控制全部通过；原始目标点中有分离点，不应向其预置历史法向冲量。共同几何的摩擦控制还漏传了manifold材质，新建集合默认friction=0，故初版9处摩擦失败只保留诊断，不构成缺陷。现保留原manifold材质并断言系数/切向solver，分离点不预置冲量后重跑；正负混合phi及纯分离输入尚未验收。
  - 00:55 source/分离控制闭环：修正材质后40组为12PASS/28FAIL，共同原生几何的10组零摩擦全部通过，9组摩擦仍失败。新增40组两因素控制，恢复源的法向/切向共同采样和分轴有效质量后10组全部通过。两个单点实验进一步分离根因：单独最后速度步时，head/foot均只需恢复采样顺序；保留前一法向步时，head只需恢复分轴质量、foot对照通过。两实验各32组，分别24PASS/8FAIL、28PASS/4FAIL，零摩擦全通过；拟归D137/P2与D138/P3，待精简重复组合并确认最终身份。该闭环使用已明确的共同位置冲量输入；零位置冲量资格、混合phi、原始几何在速度下的余差及历史状态按源分支另项推进，不算完成。
  - 01:06 完成/拆分摘要：精简两个单点矩阵后同target重编，最终112组51PASS/61预期FAIL，所有保留响应与修正材质后的对应旧行逐字一致；10组共同几何零摩擦、10组联合根因恢复及两类单点恢复全通过。闭式复算重现冲量、有效质量和锥截断，源BaseFrictionImpulse及override已核实为0/−1。116份source、11份本项闭包、52/53已用source、14份资产、160份Rust及启动器库身份PASS；证据`54/summary.json`、`54/final/`、`54/record_gate.watchdog.json`。实耗25分02秒，提前14分58秒；累计136项（93项P2、43项P3）。本项固定正法向位置冲量、零回弹目标及源几何，以分离速度公式；source零pushout资格和混合phi是独立分支，原始几何/分组/历史的未闭合边界继续下一项，不将当前通过推广到这些输入。生产、reader、转换器未改，截图0、性能暂停。

- [x] BF144.55 三角网格零位置冲量资格与混合间距：依赖BF144.54记录Gate，沿U02及fresh `PBDCollisionSolver::SolveVelocity` 的normal/tangent资格，只比较共同原生几何、真实源8次无摩擦位置投影所产生的pushout与后续速度输入；覆盖已取证18个非空trimesh姿态，正/负混合及全正间距不能用统一正冲量替代。固定源点序、预算、质量/惯量和线性位置间距，以隔离D93/D95/D99/D100；摩擦分轴/顺序保持已完成D137/D138，不复开。Gate：源位置→速度状态映射、实际prepared consumer、资格/间距分离控制及身份记录；原始几何分组和历史另项继续。开始：2026-10-15 01:09 +08:00；预估结束：2026-10-15 01:44 +08:00；实际结束：2026-10-15 01:27:42 +08:00；偏差：−16分18秒；结果：审计记录Gate PASS，新增D139/P2，D95追加同根已证实边界。估时依据：fresh资格约80行，复用已验证8次位置与2次速度参考、18姿态和实际consumer；.54含两处夹具纠正和112组分离验证实耗25分02秒，本项去掉摩擦，仅增加零pushout和混合phi状态，独立估时35分钟。
  - 01:09 活动摘要：.54已提交，累计136项；当前剩余source资格要求NetPushOutNormal>0或显式禁用position/启用split条件，目标normal solve逐点执行。只凭分支不同不登记缺陷；下一步从实际源点集计算8次位置投影、将DP/DQ映射到共同速度并逐点保留零pushout，再核对实际约束是否产生额外响应。截图0、性能暂停。
  - 01:24 source/consumer闭环：共同原生几何、8次无摩擦位置投影及2次速度消费共72组51PASS/21预期FAIL，资格和间距同时恢复的18个姿态全部通过。Crate/Rock零pushout点继续产生速度冲量，拟记D139；Cart及Stairs的正位置残差仍进入预测分支，按fresh源速度公式归并D95的已证实边界，不新增重复编号。初版把全正phi等同无contact，在下落输入下被实际pair反例纠正；共同点先加世界偏移再相减造成的力臂舍入亦已消除，未放宽阈值。独立Python复算12组，位置误差小于4e−8、速度预测误差小于1.7e−5。下一步去掉速度阶段无用的softness覆盖与重复组合，并核验60/30Hz最小控制及身份Gate。
  - 01:27 完成摘要：最终一般36组27PASS/9预期FAIL、最小24组10PASS/14预期FAIL；18个姿态及60/30Hz的完整恢复全通过，消融前后保留响应逐字一致。全正phi在有实际pair时也使用原生点，当前输入未产生额外冲量，不登记该分离对照为缺陷；共同力臂按同一源相对坐标准备，消除了原有舍入假失败。116份source、16份本项source/依赖、52/53闭包、14份原始/Build资产、160份Rust及启动器库身份PASS，最大pushout上限未触及本批输入。证据`55/summary.json`、`55/final/`、`55/record_gate.watchdog.json`；实耗18分42秒，累计137项（94项P2、43项P3）。公共runtime未启动；生产、reader与转换器未改，截图0、性能暂停。原始几何速度/摩擦、分组及历史继续U02，不以本项共同输入控制代替这些剩余闭包。

- [x] BF144.56 原始三角网格几何的速度与摩擦分类：依赖BF144.55记录Gate，仅追U02的18个非空trimesh姿态和实际PA capsule/box；在真实prepared点上固定已证实的速度资格、预测偏置、采样顺序、分轴质量、预算与点序，分别比较原点、中点改为动态shape表面点和完整原生几何。共同位置输入仍用无摩擦8次投影，以隔离D94；不宣称完整原生轨迹。已有几何原因归并，不因速度放大重复编号；分组顺序及历史接触留下一项。Gate：fresh gather/shape点/速度闭包、实际消费者与共同几何控制、剩余失败分类/最小消融和身份核验。开始：2026-10-15 01:33 +08:00；预估结束：2026-10-15 02:13 +08:00；实际结束：2026-10-15 02:09:26 +08:00；偏差：−3分34秒；结果：审计记录Gate PASS，新增D140/P2，D102/D129/D133补充已证实消费边界，Cart foot新SAT闭包拆分。估时依据：.54速度摩擦实耗25分02秒、.55含两处夹具纠正和60组最终控制实耗18分42秒；本项复用全部原生数据，剩余是18组原始点和两个摩擦输入，增加精确shape表面点匹配，按40分钟独立验收。
  - 01:33 活动摘要：.55已提交，累计137项。fresh gather按inverse mass加权shape表面点，而target仍从第一个shape点沿法向构造中点；当前需要区分D102受力点和已证实特征/裁剪差异在速度阶段的影响。先完成原始/表面点/native三种输入与独立公式对照；分组和历史不纳入本项。
  - 01:48 source/consumer闭环：无摩擦54组36PASS/18FAIL，摩擦54组27PASS/27FAIL，全部36组完整native控制通过。五个摩擦余差只恢复动态表面受力点即通过，归D102；Crate的另一余差来自重合反向三角面。fresh CapsuleTriangle2拒绝轴心位于背面，ConvexTriangle及MeshContactGenerator亦保留单面条件；原生逐面175/176分别生成2/0点，target triangle138/150均参与，法向与面法向点积约±1。32组背面/GJK分离控制22PASS/10FAIL，foot只需剔除背面，head需同时剔除背面和既有D129额外GJK；拟记D140。下一步把既有D129、D133–D136几何控制接入本轮速度消费，确认原九处几何失败是否均已覆盖，再精简无用组合并完成身份Gate。

  - 01:57 分离/拆分摘要：既有控制36组31PASS/5FAIL；其中三个Stairs foot摩擦失败来自同法向共享边重复点，fresh EdgeContactDataMap同样只保留一个面，追加24组18PASS/6FAIL且全部去重控制通过，归并D133。唯一剩余姿态是Cart foot：cooked face989源为PlaneVertex/+Y，target为近+Y的edge轴，已有face988偏置恢复不能解释；fresh ConvexTriangle2另有Minkowski edge资格和分支裁剪，按新source链拆为下一独立闭包，不预先归D134。原生Crate逐面报告与旧aggregate/PA完全相同，轴心至正/背面距离约±8.898cm、源拒绝容差0.00009cm，确认单面差异远离阈值。当前做夹具消融及恢复验收，公共runtime仍未启动。
  - 02:09 完成摘要：恢复源码字节、同target重编后最终五组矩阵共192组131PASS/61预期FAIL；36组全native控制通过，已证实几何控制在17个姿态的两种摩擦下通过，只余Cart foot两组按上述新分支拆分。背面矩阵由32缩至24组后保留输出逐字一致；取消三角面唯一匹配使恢复控制失败，恢复后通过。初版Cart head三个表面点跨相邻面匹配，精确归属后位置移动约0.22µm、两行数值更新且分类不变，已单列`56/precision_refinement.json`，没有放宽runtime门槛。116份source、40份当前/继承闭包、14份原始/Build资产、160份Rust、reader源码/package/DLL及启动器库身份PASS；证据`56/summary.json`、`56/final/`、`56/record_gate_final.watchdog.json`。实耗36分26秒，累计138项（95项P2、43项P3）；生产、读取工具和转换器未改，截图0、性能暂停。Cart剩余SAT/裁剪、分组和历史继续U02。

- [x] BF144.57 Cart足部PlaneVertex/edge选择与裁剪：依赖BF144.56记录Gate，只追本批剩余Cart foot姿态的cooked face989/target triangle150及其与face988的共享边；核对fresh ConvexTriangle2的凸面/棱边资格、SAT比较、参考特征和接触裁剪，并以同一实际prepared速度/摩擦消费者分离原因。已验证的face988偏置D134保留，不凭相邻面推断归因；确有读取缺口时仅扩展统一原生逐面读取。Gate：源有效候选与Native type/normal/support逐项重算、原始字段及最小恢复控制、必要工具自测/消融、身份记录。开始：2026-10-15 02:12 +08:00；预估结束：2026-10-15 02:47 +08:00；实际结束：2026-10-15 02:31:14 +08:00；偏差：−15分46秒；结果：审计记录Gate PASS，新增D141/P3并补充D134裁剪消费，当前18姿态速度/摩擦分类完成，分组与历史继续。估时依据：.53单个Cart胶囊分支含native读取扩展实耗21分35秒、.51六个box姿态26分34秒；当前仅一个姿态两面，已有完整cooked拓扑与消费者，但增加Minkowski edge资格和PlaneVertex裁剪，按35分钟独立验收。分组与历史随后继续。
  - 02:12 活动摘要：.56已提交，累计138项。native face989法向为+Y/type=PlaneVertex且仅两点；target triangle150为近+Y的edge轴且四点，已知控制后normal与friction各留一处失败。下一步先独立复算实际box/triangle候选，区分资格/比较与跨面归属；生产、转换器和截图状态保持当前范围。
  - 02:29 source/focused闭环：fresh Minkowski候选、Box拓扑、SAT偏置与两种参考面裁剪已独立复算；face989目标棱边的CBA×BDC=0.00106483cm²，未过源0.01条件，原生PlaneVertex匹配；face988仍由0.01cm面偏置选择Triangle。八个原始裁点按共享边归属保留六点，与完整native位置误差至多0.019µm。16组法向/裁剪单因素矩阵10PASS/6预期FAIL，联合恢复全通过；新资格原因登记D141，face988约10.8µm支持点偏移归D134的参考面选择后果。恢复后reader重新读取Cart完整网格、Crate胶囊逐面及PA数据均与旧native逐字段相同；逐面box模板链接不可用，已删除试用扩展并同target重编旧源码。点序匹配补入深度以区分重叠面，16条响应逐字不变；共享边删点也不改变本姿态响应，最终移除该多余控制，不据点数另增缺陷。下一步同target最终回归18个姿态及记录Gate。
  - 02:31 完成摘要：精简共享边控制后同target重编，最终Cart16组10PASS/6预期FAIL与精简前输出逐字一致，联合恢复四组全部通过；18姿态×摩擦0/0.7的36组恢复全PASS，其他17姿态响应与.56逐字一致。深度匹配消融把重叠平面错误配到相差12.3mm的源phi，恢复后最大误差0.37µm，V/W不变；它用于固定源点序合同，不登记为生产差异。116份source、46份当前/继承闭包、14份原始/Build资产、160份Rust、reader源码/package与重建DLL、启动器库身份均PASS；证据`57/summary.json`、`57/final/`、`57/record_gate.watchdog.json`。实耗19分14秒，累计139项（95项P2、44项P3）。生产、读取工具和转换器均无持久修改，不触发AnimBP重转换；截图0、性能暂停。U02仅余分组与历史，随后继续动画输入和核心图审计。

- [x] BF144.58 实际三角网格约束的点序与分组消费：依赖BF144.57记录Gate，在已分类的18个非空姿态中比较actual prepared自然点序与源点序；只沿本批实际manifold核对每组法向/切向阶段及跨组执行。固定已证实的几何、力臂、资格、位置输入和预算，将同根结果归入D100/D137，只有独立原因才新增。Gate：fresh标量solver入口及分组循环、实际consumer单因素对照、共同输入恢复/最小消融、身份记录；跨帧历史另项。开始：2026-10-15 02:33 +08:00；预估结束：2026-10-15 03:08 +08:00；实际结束：2026-10-15 02:48:37 +08:00；偏差：−19分23秒；结果：审计记录Gate PASS，D100/D137补充已证实入口，无新增编号。估时依据：.54速度因素分类实耗25分02秒、.57两面source/math/consumer闭环19分14秒；本项复用已分类几何和同一消费者，18姿态点序矩阵加实际多点manifold阶段控制，按35分钟独立验收。
  - 02:36 活动摘要：fresh默认GaussSeidel及原生SolverType=−1确认当前标量逐点路径；源每点合并normal/tangent后才进入下一点，target每manifold先全部normal再全部tangent。D137已有单点前后采样反例，本项仅补实际多点分组边界；接触图自然顺序不预设等于生成顺序。下一步固定实际约束与已分类几何，按原顺序/源顺序分别运行，避免把分组元数据本身记作运动差异。
  - 02:45 source/focused闭环：18姿态分别在摩擦0/0.7下比较源点序与实际constraint graph点序，两矩阵均36组25PASS/11预期FAIL，所有源序恢复通过；位置投影、逐点push、几何和自由V/W完全固定，11个自然序失败归D100。八姿态的活动点均属于一个实际manifold，先整组normal后整组tangent，即使各点保留自身pre-normal速度，仍有五姿态在60/30Hz失败；48组38PASS/10FAIL归D137。Fountain foot另有零push的边约束，初版把“所有点”误当“活动点”而触发前置断言；已按既有D139资格核对，未删除几何或放宽数值门槛。每个normal块与原多点ContactConstraint::solve的V/W逐位一致，分组影响来自切向施加时机。下一步最终同二进制复验、精简与记录Gate；历史消费者未展开。
  - 02:48 完成摘要：最终同一二进制三矩阵共120组88PASS/32预期FAIL，所有源序逐点恢复与零摩擦分组控制通过；36条共同基线与.57逐字一致，normal36条与初版逐字一致。自然序/源序、组内整块/逐点控制分别移除对应差异，保留的visit与pre-normal输入用于隔离这两个因素；前置断言修正前已输出的19条结果也逐字保留。116份source、48份当前/继承闭包、14份原始/Build资产、160份Rust、reader源码/package/DLL及启动器库身份PASS；证据`58/summary.json`、`58/final/`和`58/record_gate.watchdog.json`。实耗15分37秒，累计139项不变。覆盖回查发现.46–.51的convex/compound主要是首次位置和部分回弹，.54–.58的完整速度摩擦矩阵只覆盖trimesh；此前U02“仅余分组与历史”的摘要不能扩展到convex速度摩擦，需补独立批次，原已完成记录不复开。生产/工具/转换器未改，截图0、性能暂停。

- [x] BF144.59 convex/compound的速度摩擦消费补齐：依赖BF144.58记录Gate，沿.49原生聚合32个姿态中的30个非空集合及.48–.51已分类几何，重新核对共同位置投影/速度输入，比较实际原点、动态shape表面点与完整native几何；先固定点序和已证实的速度阶段语义，避免把D100/D137再次混入。只有新失败的实际consumer边界才展开对应已取证叶子，不复开已完成的集合或几何算法；跨帧缓存另项。Gate：fresh source与原生聚合集合、原点/表面/native共同输入矩阵、既有原因归并或最小新反例、消融与身份记录。开始：2026-10-15 02:50 +08:00；预估结束：2026-10-15 03:30 +08:00；实际结束：2026-10-15 03:28:11 +08:00；偏差：−1分49秒；结果：审计记录Gate PASS，新增D142/P2，两个微小输入字段的算法原因单独拆分，未修复。估时依据：.49含原生聚合扩展和60组首次位置实耗28分14秒、.56三角网格速度分类36分26秒；当前读取工具与速度公式已具备，但输入从18增至30个多hull姿态，按40分钟独立验收。
  - 02:51 活动摘要：.58已提交，累计139项。旧.49完整compound与单叶包装的集合合同、.50正确force point复核均保持有效；本项补查此前未覆盖的法向速度和切向摩擦。首先重读现有聚合输入与source位置上限，验证native共同输入正控制，再运行原始几何；尚无本项新增差异。
  - 02:55 原生闭环：当前reader重读同一32组聚合输入，30组非空；全部旧sample字段逐字数值相同，当前PA对象也与.57完全一致。新增的contact_type/face_index与margin字段只补证，不替代旧集合。最深phi为−14.94644cm；下一步结合fresh gather核对60Hz位置上限，再建立逐点native速度正控制。证据`59/aggregate.native.json`、`59/native_check.json`与对应watchdog，读取67秒内正常完成；没有修改reader或原始资产。
  - 02:59 source/static闭环：fresh `PBDCollisionContainerSolver.cpp:230–237,280–301,848–866`证明10m/s上限在gather按dt乘一次、迭代内上限为0；本批native最深点未触及60Hz的16.667cm。32组force-point输入从fresh报告重新生成，与.50修正后的当前数据逐字符相同；48份source/依赖身份已重读。夹具移除全部trimesh专属控制，完整native对照逐点保存N/r/phi；先跑30组共同输入法向正控制，再进入摩擦及原始/表面矩阵，生产未改。
  - 03:04 consumer矩阵：180组105PASS/75预期FAIL，native两种摩擦共60组全部PASS；原始/表面法向各15FAIL，摩擦由29FAIL降为16FAIL，13组仅恢复动态表面力臂即通过，归并既有D102候选消费。最深target phi同样未触及源上限；本轮未放宽容差。下一步只在对应旧证据的hull恢复支持点/参考面数据，保持其目标法向、原GJK与其他hull，并用删除GJK控制核对新增的Bench foot摩擦信号；尚未登记新缺陷。初始二进制、夹具及6矩阵已冻结于`59/initial/`。
  - 03:13 source/控制复盘：首轮统一删GJK误删了源保留的端帽点，修正为fresh `CollisionOneShotManifolds.cpp:1133–1196,1200–1273`的原生Cylinder/EndCap分支，并在原始候选层删除后再裁点，避免保留已丢点的集合。两处剩余叶子已重新读取，与聚合points完全相同；Arch01 hull 2的三角参考面投影逐点误差<0.14nm，表明D128的投影差也可能出现在共面三角面。两个姿态的0/10m平移控制均未恢复，尚不能把小余差判为夹具平移；继续分离R裁剪/深度及head的N/r/phi。工具修正和未通过样本全部保留，未新增缺陷。
  - 03:21 source/最小反例：fresh Parry `polygonal_feature3d.rs:55–61,333–369`把两顶点Segment按两个闭合边遍历，原生同一线段仅维护两个裁剪端点。Arch02 head的同一feature交点被正反各生成一次，独立于额外GJK；只去掉完全相同的feature/点/深度记录，60/30Hz×摩擦0/0.7的4组全部恢复，保留重复的4组全部失败。共同DP/DQ完全相同，第二重复点约8.18e−10的位置冲量却使其按源>0规则进入速度求解；未放宽资格阈值。确认D142/P2，累计140项（96项P2、44项P3）。完整恢复矩阵及剩余微小字段边界继续核对，未修复生产。
  - 03:28 完成与拆分：最终11项focused共257组173PASS/84预期FAIL，60组native共同输入和60组字段恢复对照全PASS；初始180行结果逐字不变，重复点8组和Bench字段4组也与精简前一致。删除无用Cylinder表、平移参数、宽泛GJK矩阵及无效N/phi分支后，同target重编，保留的五个字段消融响应与旧二进制逐行一致。34个fresh native输入、116份source/48份补充源码、14份资产、160份Rust、reader源码/package/DLL与启动器库身份Gate PASS，证据`59/summary.json`、`59/final/`及`59/record_gate.watchdog.json`。实耗38分11秒，未超时；生产、reader及转换器未改，截图0、性能暂停。
  - 未闭合边界：Arch01 hull 4 head仅恢复圆柱支撑点r即可通过，N/phi单改无效；源3D裁线独立复算接近native至13nm，但实际target端点相差约6.7µm，精度与裁剪算法的原因还未分开。Bench hull 2 foot去掉触发裁点的GJK并恢复有效支撑后，仍需把实际N=(-9.8717e−5,1,0)恢复为源+Y；其余字段不改即通过两种摩擦，但GJK法向根因尚未闭合。两处已定位字段并具备真实消费者，单独追source数值/特征路径；完整恢复通过不冒称原因全部解释。Convex自然点序/分组和历史仍待后续，动画输入/核心图/公共Playground Gate尚未完成。

- [x] BF144.60 两处convex微小输入的源数值路径：依赖BF144.59记录Gate，仅追Arch01 hull 4 head的圆柱裁剪端点与Bench hull 2 foot的GJK法向。对齐实际shape局部输入、单位、相对变换及margin，分别核对UE三维裁线/Parry二维线交的中间值、UE与Parry的GJK/EPA终止及支持特征；不将f32/f64或输入量化的猜测直接计作新缺陷。复用.59的N/r/phi单字段消费者，必要时仅扩充通用原生取证；其他已分类几何、convex点序/分组与历史不在本项展开。Gate：两处fresh source算法/输入闭包、独立复算或原生中间量、实际消费者最小消融、归并/确认/排除及身份核验。开始：2026-10-15 03:30 +08:00；预估结束：2026-10-15 04:10 +08:00；实际结束：2026-10-15 03:52:00 +08:00；偏差：−18分钟；结果：审计记录Gate PASS，新增D143/D144两项P3，未修复。估时依据：.59实耗38分11秒已完成全部30姿态矩阵并定位两个单字段；剩余源裁线约55行与目标线交约45行，GJK/EPA需沿实际shape/margin有限展开，本项不重复矩阵构建或全量几何扫描，按40分钟独立验收。
  - 03:30 活动摘要：.59已提交，累计140项。两个字段恢复已通过，但尚无足够证据把它们归入既有算法差异；先比较相同几何/变换下的实际中间量，保留严格容差与原生产数据。当前reader和257组最终证据已冻结；截图0、性能暂停，未授权生产修复。
  - 03:42 source/native闭环：两个选定leaf重读与旧样本逐字段一致；实际局部Collider/relative pose/支持特征已从生产查询捕获。fresh box路径先按有效0.6cm margin缩小各面，再用核心GJK；source实际UseGJK2=false、GJK/EPA epsilon均1e−6cm。读取工具补出原有constraint的GJK simplex，Bench三顶点重心复算得到0.5000003cm分离及+Z源法线，与native完全一致；不是triangle SAT路径。新增字段前检查按缺失失败，当前检查通过，PA原字段与接触点未变。继续用相同target公开GJK仅改变核心margin，以及同输入3D裁线控制；尚未登记新缺陷。
  - 03:50 算法/消费闭环：目标无margin的GJK/EPA返回倾斜法线，保持相同Parry公开API和实际输入，仅给box核心减0.006m即返回源竖直法线；4组速度/摩擦原始FAIL、4组恢复PASS，确认D143/P3。Arch查询经compound相对变换的两次inverse后，用同一公开裁剪API逐位复现；二维Gram公式的s=0.499925822，而同输入稳定交线约0.50000119。源3D裁线使用同样f32输入，端点间距由6.793µm减为0.103µm；只改变实际一个受力点即恢复两帧长摩擦，确认D144/P3。16组10PASS/6预期FAIL，8个算法恢复全PASS；N/r/phi单字段不变性检查通过，未放宽容差或复制native几何作恢复。

  - 03:52 完成摘要：保留的16组响应与精简前逐行一致，单字段不变性通过；删除未使用的native注入、相邻字段模式和重复诊断后，同target重编恢复了8个正控制。新增GJK取证的设置/点/权重/坐标/margin六种消融均被拒绝，恢复通过；旧分类8个native fixture也通过。116份原source、59份补充source/依赖、14份资产、160份生产Rust、reader源码/package/DLL和启动器库身份Gate PASS，证据`60/summary.json`、`60/final/`及`60/record_gate.watchdog.json`。本项实耗22分钟，累计142项（96项P2、46项P3）；生产与转换器未改，截图0、性能暂停。两个字段根因已闭合，convex自然点序/分组及接触历史继续U02。

- [x] BF144.61 convex/compound自然点序与分组：依赖BF144.60记录Gate，限定已有30个非空聚合姿态，固定.59/.60已分类的支持几何、源位置输入和速度阶段，比较actual prepared遍历与源点序，并核对每个实际manifold内部的N/T分段。跨hull分组保持实际身份，不把trimesh单活动组假设套入compound；优先归并D100/D137。Gate：fresh原生集合顺序及源/目标solver循环、全部姿态单因素矩阵、实际多点normal消费者复算、最小消融与身份记录；跨帧缓存下一项。开始：2026-10-15 03:54 +08:00；预估结束：2026-10-15 04:29 +08:00；实际结束：2026-10-15 04:10:38 +08:00；偏差：−18分21秒；结果：审计记录Gate PASS，归并D100/D137，无新增编号。估时依据：.58同类18姿态点序/单组阶段实耗15分37秒，.60数值字段实耗22分钟且无需重做；本项增至30姿态、多hull需按实际manifold分段，复用已验收的恢复输入与公开solver，按35分钟独立验收。
  - 03:54 活动摘要：.60已提交，累计142项。两个数值字段已闭合，U02剩余自然序/分组与历史；先重读原生32组有序集合并与.59逐字段比较，再扩展已有分段消费者至compound。无生产修复、截图或性能测试。
  - 03:58 source/static闭环：当前reader的32组聚合集合（30非空）与.59已有字段完全相同；原生container输出的hull顺序本已递增，与旧生成数据一致，无需复开旧几何结论。fresh源container按constraint/point次序逐点施加N/T，target先overflow后各color/manifold、组内N循环再T循环。夹具按实际manifold身份分段，全部source-order法向30组PASS；继续自然序/组阶段四组合与摩擦对照，不把单活动组假设用于多hull。
  - 04:03 初始矩阵闭环：240组189PASS/51预期FAIL，8个自然序敏感姿态、13个组阶段敏感姿态，组合失败正好为两者14姿态的并集；60个全部恢复控制PASS，且与.59恢复基线逐字一致。24姿态含多个actual manifold，源排序后各组仍连续；每个法向块与同一实际多点ContactConstraint::solve逐位相同。四组合的点、DP/DQ/push完全不变，零摩擦分组也逐位不变；归并D100/D137，不新增原因。继续三个30Hz边界及夹具精简复验。

  - 04:10 完成摘要：最终240组189PASS/51预期FAIL与初始逐行相同，三个30Hz姿态的12组5PASS/7FAIL也保持分类；另一个组边界最小测试PASS，共253组195PASS/58预期FAIL。合并两个manifold的消融被实际多点normal重放合同拒绝，按源码字节恢复并在同target重编后最小测试通过；12个每组至多一个活动点的姿态逐位不受组阶段影响，15个无活动点组正确跳过。32组fresh原生集合、116份source/59份补充源码、14份资产、160份Rust、reader源码/package/DLL及启动器库身份Gate PASS，证据`61/summary.json`、`61/final/`与`61/record_gate.watchdog.json`。实耗16分39秒，累计仍142项；生产、reader和转换器未改，截图0、性能暂停。U02当前只余convex/trimesh跨帧历史。

- [x] BF144.62 convex/trimesh跨帧几何复用：依赖BF144.61记录Gate，限定已有PA head/foot与生产Playground碰撞形体，核对源bounds/CanRestore开关、相对位移/旋转及逐点保留阈值、重建后集合；先补当前实际leaf的两帧原生矩阵，再在保留ContactId的真实角色约束中验证最小运动影响。D104同根优先归并；静摩擦saved点、冲量warm start和初始穿透历史另项，不把几何复用当作全部历史。Gate：fresh eligibility/caller/容差闭包、原生静止/小平移/超限/旋转控制、实际缓存身份与消费者、源因分类及消融/身份记录。开始：2026-10-15 04:11 +08:00；预估结束：2026-10-15 04:51 +08:00；实际结束：2026-10-15 04:46:54 +08:00；偏差：−4分6秒；结果：审计记录Gate PASS，归并D104，无新增编号。估时依据：.36含reader扩展的28组primitive历史及32组消费实耗31分11秒；当前已有next接口、54个convex leaf与18个trimesh姿态、全部首次接触几何分类，本项复用读取器，按几何历史独立验收并排除其他缓存类型。
  - 04:16 source/static摘要：fresh `CollisionUtil.h:46–77`排除capsule/sphere，large-mesh开启时允许box/mesh复用；`SetLastShapeWorldTransforms`把实际bounds资格写入CanRestore，现有native字段可直接验证，不先增加读取接口。`TryRestoreManifold`的形体平移阈值为0.2×CollisionTolerance、旋转quat点积>0.9999，逐点横向阈值0.8×CollisionTolerance，至少保留min(原点数,4)。当前世界UseManifolds/AllowManifoldUpdate均为1；target仍从空manifold/workspace查询。下一步重读两帧几何；现有fresh_points指同一constraint重新运行窄相，保留源GJK缓存语义，不冒称全新constraint。
  - 04:30 原生/消费者正控制：72个已有leaf×四种运动共288组取证PASS，所有初始point字段与旧单帧/聚合记录一致。36个box在静止、2mm平移、1度yaw时保留，在6mm平移时全部重建；36个head均不允许复用。小平移27个leaf、旋转28个leaf与重建几何不同；静止/超限均无显著几何变化。另48组当前姿态完整聚合确认hull集合无增减。24个foot两帧都保留实际ContactId，目标每个prepared点与当帧冷查询匹配；共同阶段注入原生保留几何的24个正控制全PASS。下一步只比较保留/重建几何的实际消费者，静态生成差异已作为共同输入控制，其他缓存类型不在本项宣称等价。
  - 04:43 消费/消融闭环：24姿态小平移对照48组33PASS/15预期FAIL，重建几何的15处偏差只恢复旧点即可通过，归并D104。Bench原始target点不做几何替换，与native重建响应在既有容差内一致；两帧长×四种运动×原始/重建/保留的24组16PASS/8FAIL，静止和超限全PASS，小平移/旋转仅保留控制PASS。Stairs02再以两帧长×摩擦0/0.7确认32组24PASS/8FAIL。每帧初始化配置的消融使ContactId从2变0，被历史身份断言拒绝；按源码字节恢复、不回退mtime，并在同target重编后最小测试PASS。已删去未使用的上一帧几何字段及162条fixture记录，继续最终复验，不新增缺陷编号。

  - 04:46 完成摘要：最终104组运动响应与精简前逐行一致，另1个生命周期最小测试PASS，共105组74PASS/31预期FAIL；所有保留几何控制和静止/超限控制通过。288组两帧native及48组当前完整聚合、116份source/63份补充源码、14份资产、160份生产Rust、reader源码/package/DLL与启动器库身份Gate PASS，证据`62/summary.json`、`62/final/`和`62/record_gate.watchdog.json`。实耗35分54秒，累计仍142项；生产、reader和转换器未改，截图0、性能暂停。几何复用已闭合；saved friction/impulse、重建中的GJK状态与initial penetration继续U02，不用本项代替其他历史验收。

- [x] BF144.63 convex/trimesh静摩擦保存与冲量历史：依赖BF144.62记录Gate，沿实际head/foot接触核对SetSolverResults、8点保存边界、saved点匹配和下一帧摩擦输入，并核对目标warm-start冲量在源求解阶段的对应语义。几何复用沿用.62；GJK重建状态与initial penetration另项。Gate：fresh保存/匹配/solver初始化闭包、必要的native公开接口取证、实际角色缓存和消费者控制、同根归并或新差异登记、最小消融及身份记录。开始：2026-10-15 04:55 +08:00；预估结束：2026-10-15 05:45 +08:00；实际结束：2026-10-15 05:11:50 +08:00；偏差：−33分10秒；结果：审计记录Gate PASS，归并D96并新增D145/P3，未修复。估时依据：.30单点历史实耗约19分钟、.36含native接口的历史批次31分11秒、.62两帧完整mesh消费35分54秒；本项复用两帧夹具，限定静摩擦/冲量历史，预留一次读取器公开API扩展和自测。
  - 04:55 source摘要：fresh源保存条件是点数严格小于8，8点及以上的零ratio点不保存；此前primitive最多4点，不能覆盖该分支。当前实际Stairs为8点、Cart为14点，具备直接核验输入；源Activate从saved点恢复锚点，solver InitManifoldPoint将累计pushout/impulse归零，目标match_contacts复制旧冲量。先把接口和实际消费者闭合，不凭字段差异新增缺陷。
  - 05:02 source/native/输入消费闭环：读取器仅增加可选history输入，通过公开ResetSolverResults/SetSolverResults/Activate取回保存点和下一帧锚点，30组覆盖4/6/8/14点、零/保持/插值/混合比例、静止及失配；147点匹配、69点不匹配，独立保存/匹配合同PASS。24个真实角色两帧输入×有无锚点的48组34PASS/14FAIL，全部恢复控制通过，另4个零摩擦控制PASS；几何与位置算法使用共同已证实输入以隔离D96，未宣称执行native动力仿真。8/14点的零ratio对照通过，4/6点则仍保存当前接触并产生跨帧切向误差。继续实际warm-start冲量及读取器消融/兼容验证。

  - 05:11 完成摘要：最终105组79PASS/26预期FAIL，三组运动矩阵与精简前逐行相同；12个单点/闭式位置公式及锚点有效性最小反例PASS。忽略有效标志的消融FAIL，源码字节恢复并在同target重编后PASS；4种元数据消融均被拒绝。读取器新增夹具从30组精简到8组且共同输出逐值一致，10组旧接口字段保持，非法ratio按7退出。116份source、65份补充源码、14份资产、160份生产Rust及reader/package/DLL/启动器库身份PASS，证据`63/summary.json`与`63/final/`。实耗16分50秒；复用已证实阶段及单个读取接口使本项提前完成，累计143项（96P2/47P3）。生产与转换器未改，截图0、性能暂停；下一批只核对U02剩余重建状态与初始穿透。

- [x] BF144.64 GJK重建缓存与初始穿透历史：依赖BF144.63记录Gate，沿已有PA/Playground接触闭包核对失败几何复用后的GJK simplex、冷重建结果及initial-overlap参数来源。Capsule专用和trimesh路径先由实际caller/空缓存排除；只将box/convex的有效重建路径送入已有消费者，不扩展其他物理功能。Gate：fresh实际开关/算法/参数链、同姿态原生冷/暖重建和完整compound输入、必要的消费者/反例控制、工具消融及身份记录。开始：2026-10-15 05:16 +08:00；预估结束：2026-10-15 05:56 +08:00；实际结束：2026-10-15 05:40:59 +08:00；偏差：−15分01秒；结果：审计记录Gate PASS，无新增差异，关闭U02。估时依据：.62几何两帧闭包35分54秒、.63保存/冲量历史16分50秒，当前已定位GJK Restore/Save及初始穿透递归入口，只有box/convex走跨帧simplex；复用读取器和消费夹具，按剩余两类状态独立收口。
  - 05:16 source摘要：fresh `ConstructConvexConvexOneShotManifold`使用constraint中的GJK cache，ResetActiveManifoldContacts不清缓存；Capsule专用函数和trimesh路径不使用该跨帧字段。几何复用成功时不会调用GJK，因此本批先核对6mm平移导致复用失败的27个convex foot leaf，并读取对应完整当前compound。初始穿透链已追到BodyInstance override→PhysicsProxy→solver默认值→gather；需要实际project SolverOptions字段，不能把PA的RBAN MaxDepenetrationVelocity=0误当世界参数。
  - 05:23 source/native摘要：当前GjkWarmStart=True，project initial-overlap velocity=−1，21个PA body均未override；`ChaosScene.cpp:89–95`应用project SolverOptions，`PBDRigidsSolver.cpp:3668–3678`再把解析后的1e10cm/s写入body。27个box/convex的6mm重建与3个无缓存对照，暖/冷point字段全部逐值相同，未发现新的运动差异。fresh `GJK.h:644–695`还有核心重叠后的EPA分支，浅接触矩阵不能覆盖；增加1cm向下位移及5度yaw/组合控制，保留同批已有形体，并用下一帧simplex证明实际分支，分别读取45个完整当前compound以核对新增/消失的leaf。

  - 05:40 完成摘要：27组浅接触及81组穿透/旋转重建与独立冷约束逐值相同，3组无缓存对照通过；60个完整compound无新增/消失leaf，58组四顶点simplex由源TetrahedronSimplexFindOrigin直接证明进入EPA。旋转先按源SetQ量化为float32，消除直接查询与完整物理路径的世界变换精度差；未放宽比较容差。删除重复冷查询接口后全部共有字段保持，4组新夹具、10组旧接口及4种元数据消融通过。初始穿透的21个body均继承世界默认，7722个本批源公式控制归零，有限速度反控制3152处保留非零值；没有用探针初始化的零值冒充动态仿真结果。116份source、75份补充源码、14份资产、160份生产Rust及reader/package/DLL/启动器库身份PASS，证据`64/summary.json`。实耗24分59秒；无新运动输入差异，未重复启动运动runtime，累计仍143项。生产与转换器未改，截图0、性能暂停；U02关闭，下一批进入运动动画输入与生命周期。

- [x] BF144.65 AnimationInstance可见性更新与时钟：依赖BF144.64记录Gate；核对NativeUpdate/ThreadSafeUpdate/PostUpdate在可见性跳帧中的实际caller、pending与历史时间输入，并沿直接Head消费者验证。主/linked实例的静态角色已核对；原计划的NativeInitialize、首次pose及实例替换独立拆至下一项，骨轴计算归Feet；View/Locomotion、Feet/Turn/Transition、通知及核心155图仍各自后置。Gate：fresh根/Head AnimBP与角色/动画设置、实际UE更新/跳帧caller和target阶段、完整角色的有/无montage恢复与倍率控制、最小反例/消融和身份记录。开始：2026-10-15 05:47 +08:00；预估结束：2026-10-15 06:32 +08:00；实际结束：2026-10-15 06:32:15 +08:00；偏差：+15秒；结果：审计记录Gate PASS，新增D146/D147两项P3，未修复；初始化闭包独立拆分并继续。估时依据：原计划源ALS生命周期约250行、UE更新及target约600行，参照.02/.37完整角色审计；实际外层资格、CallFunction和初始化配置闭包增加，拆分保留未验收部分。
  - 05:47 活动摘要：fresh NativeUpdate使用上一轮global dilation求未缩放dt，PostUpdate在组件注册完成后清pending；target在provider/head writer发布输入，再由post-physics evaluator控制是否求值。源默认OnlyTickMontagesWhenNotRendered，源码存在标记pending及失去可见性后的特殊更新路径；先确认哪些字段仍会被更新和实际head回调时点，不把镜像值不同直接计为缺陷。当前只覆盖根实例生命周期入口与必要Head输入链，源子图总数及fresh静态合同待本项原生读取后登记。
  - 06:12 source/consumer摘要：fresh根/Head/B_Als_Character/AIS四份读取PASS，当前只核对2个authored AnimGraph的生命周期入口与Head的2个CallFunction节点；Head明确使用OnBecomeRelevant/OnUpdate，linked实例共享main montage数据。UE外层ShouldTickPose要求隐藏时存在montage，纯空闲不进入ResetUpdateCounter；首版空闲oracle据此撤销，不计缺陷。加入实际transition montage（隐藏时clip推进到0.2s且pose_ready）后，60/30Hz×两视角的16组12PASS/4FAIL；恢复可见时Head pitch分别停在4.921/30.767度，源初始化应为45度，只补InitializeHead标志即恢复。纯空闲隐藏和持续可见对照全部通过。具体Head平滑算法与其他图未据此宣称完整等价，继续无montage跳帧的时间历史及初始化/更新边界。
  - 06:29 闭包与拆分：逐帧确认无transition/turn/action后，隐藏中1→0.25倍率变更的6组5PASS/1FAIL确认D147；此前16组计数恢复确认D146。删除montage资格条件会使纯空闲控制误报，最小消融FAIL；按源码字节恢复并在同target重编后PASS。首次pose另受`InitAnim:1200–1216`的全局bTickAnimationOnSkeletalMeshInit、bUseRefPoseOnInitAnim及deferred root初始化控制，需要额外原生配置与首次pose/实例替换矩阵；该独立初始化闭包拆到下一项，当前项收口为已验证的可见性更新与时钟，保持原总体审计目标。NativeInitialize中的骨轴计算归后续Feet，完整155图合同和公共Playground仍未宣称通过。

  - 06:32 完成摘要：最终23组18PASS/5预期FAIL（可见性16组、倍率6组、纯空闲caller边界1组）；22组主体的数值/判定与加上逐帧无montage资格核验后的初版一致。丢掉montage资格的消融被拒绝，字节恢复、同target重编后最小边界PASS。源四资产、Root32/Head6节点中的必要生命周期连线、6份ALS/7份UE/14份目标闭包与116份基线source、160份生产Rust、原始/Build资产及启动器库/reader身份PASS，证据`65/summary.json`、`65/final/`。首版空闲oracle保留为已撤销诊断，不计缺陷。未改生产、转换器、reader或目标图；截图0、性能暂停，公共Playground与完整155图Gate仍后置。实耗45分15秒；累计145项（96P2/49P3），下一项继续已拆分的初始化配置、首次pose及实例替换。

- [x] BF144.66 AnimationInstance初始化与替换：依赖BF144.65记录Gate，核对实际AnimationSettings与角色mesh的InitAnim分支、NativeInitialize/主linked实例所有权、首次pose和替换后的状态生命周期；骨轴计算归Feet，其他输入算法与通知后置。Gate：fresh配置/原生角色与初始化caller、target首帧/替换最小反例及对照、必要消融与身份记录。开始：2026-10-15 06:37 +08:00；预估结束：2026-10-15 07:22 +08:00；实际结束：2026-10-15 06:54:30 +08:00；偏差：−27分30秒；结果：审计记录Gate PASS，新增D148/D149两项P3，未修复。估时依据：.65已完成可见性/时钟闭包，剩余初始化约350行UE caller与ALS/target所有权；复用完整角色夹具与通用CDO读取，参照.65实际45分15秒安排本项独立验收。
  - 06:38 活动摘要：.65的2处差异已提交；本项只展开已拆分的初始化链。先经通用原生Class/CDO读取AnimationSettings，再核对注册时的零delta求值及第一次普通tick；尚未认定首帧语义有差异。
  - 06:49 source/反例摘要：fresh AnimationSettings确认bTickAnimationOnSkeletalMeshInit=True，实际body mesh的ref-pose覆盖=False；初始化走零delta求值。原生Uninitialize明确ResetAnimationCurves，目标的曲线投影组件却跨播放器替换保留。实际Rifle瞄准曲线0.994912令新实例首帧Head alpha变成0.005088而非1；覆写/移除重插两种路径共12组10PASS/2FAIL，删除旧投影或仅清PoseAiming均恢复，确认D148/P3。首次生成角色另见pending在零delta后立即清除的候选，正在沿Actor注册→首个普通tick→Jump实际FSM核验，尚未计数。无生产修改。
  - 06:52 首次更新闭环：fresh Actor在全部component注册结束后才设HasRegisteredAllComponents；mesh OnRegister中的零delta动画PostUpdate仍保留pending，之后InitializeComponent的InitAnim(false)不重复初始化已有实例。目标首个零delta求值立即清pending；新角色下一帧实际Jump输入被接受且进入Jump，源应抑制bJumped并进入Fall。只保留pending一帧即恢复Fall，延后一帧再跳的对照仍进入Jump；3组2PASS/1FAIL，确认D149/P3。原生Locomotion条件与实际FSM状态/权重均已读取；完成消融及最终身份核验后提交。
  - 06:54 完成摘要：最终15组12PASS/3预期FAIL；移除夹具未消费的返回值及重复deadline后，同target重编，全部数值、姿态及FSM判定与精简前逐行一致。保留有界循环和现有wall/no-progress watchdog，单字段恢复控制均通过。5份原生读取、5份ALS/7份UE/18份目标闭包、116份基线source/160份生产Rust及全部累积原始/Build资产、reader和启动器库身份PASS，证据`66/summary.json`、`66/final/`。实例主/linked所有权、首帧零delta、独立输入清理已覆盖；完整图算法与Feet等未据此验收。实耗17分30秒，复用CDO通用读取及既有角色夹具避免了工具扩展；累计147项（96P2/51P3）。生产、转换器、reader未改，截图0、性能暂停；继续View/Head/Spine输入算法。

- [x] BF144.67 View、Head与Spine输入和算法：依赖BF144.66记录Gate，核对ALS RefreshViewOnGameThread/RefreshView/RefreshSpine/RefreshHead及它们直接消费的Character view、actor/base旋转、时间和曲线；逐项比较输入来源、坐标基、阈值、阻尼/弹簧与状态切换。初始化、隐藏时钟等已确认根因只作隔离控制，不重复编号；Feet、Locomotion运动参数和完整Rig另行验收。Gate：fresh源配置/算法/直接图绑定、独立数值或原生合同、完整角色消费者与最小消融、身份和归并记录。开始：2026-10-15 06:57 +08:00；预估结束：2026-10-15 07:37 +08:00；实际结束：2026-10-15 07:16:17 +08:00；偏差：−20分43秒；结果：审计记录Gate PASS，无新增差异。估时依据：当前源三组算法约300行，数学helper及输入caller约350行；.65/.66已完成生命周期和真实Head夹具，按40分钟独立闭合输入及计算，不重做已闭合的时钟矩阵。
  - 06:57 活动摘要：.66已提交，累计147项。当前只沿View/Head/Spine的直接输入链展开；wrap边界先比较两侧公式，源同样存在的行为不登记为遗漏。截图0、性能暂停。
  - 07:15 source/consumer摘要：fresh三组ALS算法、SpringMath/InvExpApprox及Head配置、主/linked初值、Head Alpha与GetControlRigInput绑定均已核对；当前bIgnoreBaseRotation=True，源SaveBaseLocation和目标snapshot筛选均关闭相对基座旋转，未把禁用路径猜测列为缺陷。±180°处YawSpeed大值两侧同源公式，排除候选。19个阈值、24个scaled/unscaled/zero时钟、8个实际Spine输入发布、6个完整角色view/倾斜/owner-zero/Roll锁存，共57组PASS；头部角度与独立公式最大差仍在原1e−4度合同内，未放宽阈值。Rig局部基为(x,z,y)，yaw取反已核对到实际发布值；完整Rig运算后置。当前未发现新增差异，继续精简夹具后做身份Gate。
  - 07:16 完成摘要：删除夹具无用Option分支并把五次等待更新精简为一次，同target重编后57组全部PASS，所有记录值与精简前逐行相同。4份原生读取、9份ALS/5份UE/16份目标闭包及116份基线source、160份生产Rust、累积原始/Build资产、reader/启动器库身份PASS，证据`67/summary.json`、`67/final/`。没有新增或复开缺陷，累计仍147项；实耗19分17秒。直接公式与已核验输入链能完成本批，无需扩展原生读取器或生产算法。截图0、性能暂停，完整155图与公共Playground仍后置；下一项核对Layering/Pose与Locomotion通用输入。

- [x] BF144.68 Layering/Pose与Locomotion通用输入：依赖BF144.67记录Gate，核对RefreshLayering/RefreshPose和RefreshLocomotionOnGameThread的曲线默认值、权重、actor/mesh变换、速度/加速度/输入方向及pending语义，并追到当前生成图的直接读者。Grounded/Standing/Crouching/InAir算法、Feet查询和动作/通知分批后置；只因直接源链路扩展必要消费者。Gate：fresh主/linked图、配置和C++合同、关键边界及完整角色消费者、最小消融和身份记录。开始：2026-10-15 07:18 +08:00；预估结束：2026-10-15 07:58 +08:00；实际结束：2026-10-15 08:12:10 +08:00；偏差：+14分10秒；结果：审计记录Gate PASS，新增D150/D151两项P3，未修复。估时依据：本批约230行源算法，target共享provider/curve投影已在.65–.67完成生命周期和部分输入核对；参照.67实耗19分17秒，保留actor动作输入与真实图消费的核验空间。
  - 07:18 活动摘要：.67已提交，57组全部PASS且无新增差异。当前继续曲线状态与通用运动输入，先核对source逐字段用途，再进入最小反例；已有147项未修复问题不重复编号，截图0、性能暂停。
  - 07:41 source/consumer摘要：源GetMaxBrakingDeceleration在Custom/Falling/None选择不同于Walking的值，provider却始终发布GroundedDynamics。fresh Locomotion T19/T20均为Inertialization；实际Jump后Grounded写帧停留、权重0，因此未把空中字段差直接计数。沿直接consumer扩到High Mantle的0.2秒Hermite混入、Grounded Refresh与Standing Lean绑定；Custom第2帧dot(V,A)<0、slot权重0.074074且Grounded仍更新，目标lean=0.053087、源=0.023253，仅恢复braking=0即通过，确认D150/P3。实际Lean输入缓存等于当帧值，BS输出骨骼角差0.013047rad；正点积控制姿态逐值相同。独立运行的最终整姿态存在基础步态相位差，未用它冒充隔离证据。继续Layering/Pose字段及普通运动输入核验。
  - 08:04 闭包摘要：Layering/Pose六组各30字段通过；普通运动11组中9PASS/2FAIL，0.5和1.5m/s严格边界的局部投影误差使moving标志提前成立。沿同一speed输入追到Standing Idle→Move：受控actor俯仰60度、相同世界水平速度1.6，目标speed=0.8并留在Idle，恢复单个speed字段即进入源规定的Move转换；四组3PASS/1FAIL，当前actor quaternion与场景前置均已断言，确认D151/P3。旧夹具只等待Gameplay Grounded，尚未到实际Standing Idle的失败已作为前置错误排除；完整条件绑定已读原生对象。几何位置/胶囊尺寸的具体查询消费者仍按计划归InAir/Feet，完整移动算法另行核对。当前累计149项，继续消融与最终Gate。
  - 08:12 完成摘要：最终27组23PASS/4预期FAIL，另180次曲线字段断言通过；制动、曲线、运动与speed矩阵记录和精简前相同。移除重复的晚期旋转覆盖后，当前actor quaternion前置与恢复控制仍PASS，故删除该覆盖；50帧Mantle探索与重复Air样本精简为必要边界，原始诊断保留。9份原生读取、9份ALS/5份UE/20份目标闭包、116份基线source/160份生产Rust及已追踪原始/Build资产、reader与启动器库身份PASS，证据`68/summary.json`、`68/final/`。实际54分10秒；两条新consumer链和场景前置补证使本项比预计多14分10秒，后续同类输入审计需计入这类成本。累计149项（96P2/53P3）；未改生产、reader或转换器，截图0、性能暂停。继续Grounded/Standing/Crouching运动算法。

- [x] BF144.69 Grounded、Standing与Crouching运动算法：依赖BF144.68记录Gate，核对RefreshVelocityBlend/GetAccelerationAmount/GroundedLean、方向与YawOffset、Sprint、Stride与PlayRate及当前源曲线和直接图消费者。沿用已核验的公共输入，已登记的D150/D151仅作隔离；InAir/Feet、Turn/Transition和通知另行验收。Gate：fresh算法/配置/原生曲线与图绑定、必要边界及实际消费者对照、最小消融和身份记录。开始：2026-10-15 08:19 +08:00；预估结束：2026-10-15 09:14 +08:00；实际结束：2026-10-15 08:32:22 +08:00；偏差：−41分38秒；结果：审计记录Gate PASS，无新增差异。估时依据：源约300行算法及数学helper、当前三组linked图和曲线依赖；.68实耗54分10秒，计入consumer前置与浮点边界核验后独立估时。
  - 08:19 活动摘要：.68已提交，累计149项。本批先读取实际设置和曲线，再判断同名或复用函数是否存在行为差异；不凭字段名推断。截图0、性能暂停，生产实现保持不变。
  - 08:30 source/consumer摘要：9份原生读取完成；5曲线33键、厘米/米与Yaw反向转换、实际半衰期和动画速度均一致，源Crouching确实复用Walk曲线。60组数学边界、7组实际Grounded更新、26组站立/蹲姿速度与缩放/方向/播放上限、38帧Sprint计时共131组PASS；实际Blendspace、TwoWayBlend与Sequence消费值也一致。原消费者探针只识别GlobalF32而漏读蹲姿节点的前置失败已排除，未计缺陷。Sprint在约0.5秒关掉加速混合，pending与零delta对照通过。当前无新增差异，继续删除多余夹具等待并做最终身份Gate；通知和完整图/Rig仍后置。
  - 08:32 完成摘要：每种姿态的等待从18次更新减为1次，同target重编后131组全部PASS且逐行数值保持一致，四类运动消费者与Sprint消费者仍实际求值。9份原生读取、8份ALS/4份UE/17份目标闭包及116份基线source、160份生产Rust、累积原始/Build资产、reader/启动器库身份PASS，证据`69/summary.json`、`69/final/`。实耗13分22秒；没有新反例需要扩展动作场景，复用角色与公开节点状态显著缩短本批。累计仍149项（96P2/53P3），生产、转换器与reader未改，截图0、性能暂停。下一项InAir输入算法及地面预测查询。

- [x] BF144.70 InAir输入与地面预测：依赖BF144.69记录Gate，核对RefreshInAirOnGameThread/RefreshInAir、跳跃速率锁存、空中Lean曲线与阻尼，并沿GroundPrediction的实际location/胶囊/缩放/坡度和碰撞响应输入闭合到当前查询与图消费者。已登记输入与CMC几何根因只作归并，不展开无源链路的通用碰撞；Feet与通知后置。Gate：fresh配置/曲线/相关AnimBP及查询source、数值边界和实际消费者反例或对照、最小消融及身份记录。开始：2026-10-15 08:34 +08:00；预估结束：2026-10-15 09:19 +08:00；实际结束：2026-10-15 09:14:45 +08:00；偏差：−4分15秒；结果：审计记录Gate PASS，新增D152/P2与D153/P3，补充D10/D125，未修复。估时依据：源约150行InAir算法，target约250行writer/query及胶囊和查询helper；.69纯数值闭环实耗13分22秒，本项增加查询响应与geometry消费者，按45分钟独立验收。
  - 08:34 活动摘要：.69已提交，无新增差异。源地面预测用Visibility及WorldStatic/WorldDynamic/Destructible响应，并接受初始穿透的可行走命中；本批先核对这些有效条件和原生曲线，再构造实际查询对照。截图0、性能暂停。
  - 08:41 source/反例摘要：5份原生配置/曲线/图与原生胶囊sweep完成。横向scale=(2,1,1)、窄平台边缘距中心0.4m时，源查询半径仍0.3m且原生未命中，目标复用横向0.6m的物理形体，实际Fall预测权重为0.345407；只替换查询形体为源组件尺寸即恢复0，8组7PASS/1FAIL，确认D152/P2。原生源物理半径0.6m也命中，证明只修D11的物理形状不足以恢复本查询。平地斜向扫描另见小幅预测差，正在与既有D125分离，未重复编号。累计150项；下一步完成数值/坡面/初始穿透与consumer闭环。
  - 09:03 consumer时点摘要：InAir公式与10组原样/恢复对照已闭合，四处查询偏差恢复后全部对齐实际Fall混合输入；初始穿透与大型平面误差归D125，坡面资格归D10。宽泛探针同时读到了Default Overlay中的同名输入，其值早于RefreshInAir；本项仅沿该已证实读者补取Root/Default Overlay和UE缓存Pose更新顺序，核定是否为源同样的上一帧消费，其他Overlay与完整图仍后置。
  - 09:10 顺序闭环：fresh Layering原生缓存队列中Locomotion先于Overlay；两个输入各只连入自己的SaveCachedPose，UE在Root更新后依该队列调用PostGraphUpdate。目标按需求遍历即时求值，Default Overlay先读旧GroundPrediction，再由Locomotion刷新。当预测开关变化时，实际Default Overlay输入落后一帧；仅提前发布同一个当前字段便恢复，8组6PASS/2FAIL，确认D153/P3。此组方向与原生扫描已匹配，排除了D125；当前累计151项。此前将不同读者统一比作帧末值的断言已拆为实际Fall与Default Overlay两个独立consumer合同；继续精简后提交。
  - 09:14 完成摘要：最终44组34PASS/10预期FAIL，所有分离恢复控制通过；原生4个形状/4个平面/6个初始穿透查询均有效。删除2个重复unit恢复样本、额外contact/关闭几何探针和逐节点诊断输出，同target重编后保留的全部语义记录逐字一致。12份原生报告、7份ALS/10份UE/18份目标/5份依赖闭包与116份基线source、160份生产Rust、累积原始/Build资产、reader/启动器库身份PASS，证据`70/summary.json`、`70/final/`。实耗40分45秒，新增缓存顺序链占用取证与隔离时间；累计151项（97P2/54P3）。未改生产、转换器、reader或依赖，截图0、性能暂停；继续Feet，完整核心图、动作精确查询与公共Playground仍待闭合。

- [x] BF144.71 Feet输入与Foot Lock：依赖BF144.70记录Gate，核对NativeInitialize的腿轴、RefreshFeet输入有效性/曲线/骨骼目标，以及锁定权重、瞬移、基座切换和旋转约束的实际消费者。Rig地面偏移/IK求解另行验收，Dynamic Transition仅核对本批输出接口，动作选择后置；已确认时钟/缓存问题只作隔离。Gate：fresh配置/骨架/根图与C++合同、数值边界及完整角色消费者反例或对照、最小消融和身份记录。开始：2026-10-15 09:21 +08:00；预估结束：2026-10-15 10:11 +08:00；实际结束：2026-10-15 09:58:33 +08:00；偏差：−12分27秒；结果：审计记录Gate PASS，新增D154–D158五项，未修复。估时依据：源Feet与初始化约390行，target状态/helper与实际输入约600行；.70查询和缓存闭包实耗40分45秒，本项复用完整角色与Rig遥测，按50分钟独立验收。
  - 09:21 活动摘要：源先读取上次完成的mesh骨骼，再依有效性执行Feet和锁定；target入口及实际Rig输入已定位。先核对骨轴/组件空间和输入生命周期，再验证权重与重定位边界；截图0、性能暂停。
  - 09:27 source/consumer闭环：4份原生配置/骨架/mesh/根图有效。恢复前权重0.8、当前曲线0.5时，源保留并淡出到0.5，目标先清零，下一帧目标移动8cm后实际Rig输入偏移4cm；仅恢复该权重便通过，12组10PASS/2FAIL，确认D154/P3。mesh scale=(1.4,0.8,0.6)、yaw=0.7时，源Inverse QST先除scale再逆旋转，目标矩阵逆变换次序相反，实际Rig输入相差17.07cm；8组7PASS/1FAIL且单字段恢复通过，确认D155/P2。下一步完成本批权重、约束、瞬移/基座与曲线边界；地面偏移和IK求解继续后置。
  - 09:34 边界摘要：16组权重及18组基座历史通过；D154已追加真实隐藏/首帧pending/再有效的完整角色路径并保持相同分类。fresh Feet有效性早退还保留FootPlanted/FeetCrossing，而provider直接读新曲线；仅沿已定位的Jump脚选择条件补取Locomotion原生图并验证该读者。初始瞬移窗口候选的恢复对照暴露了夹具前一帧/当前组件位置混用，修正其输入历史后再判断，不登记未闭合结果。
  - 09:46 剩余接口摘要：初始瞬移窗口已由原生CDO、实际游戏时钟及时间0事件恢复闭合，确认D156/P3。Jump的pending首帧不会进入脚选择，改用源明确支持的identity pelvis无效条件验证曲线保留；探针已对齐实际global输入回退。另由`RefreshFeetOnGameThread→RefreshDynamicTransitions`证明pending帧仍采集当前足目标，而target整段跳过；仅追加Standing Idle直接消费者来核对锁定距离，完整Transition算法仍后置。基座切换夹具先隔离CMC携带触发的瞬移，再验Foot消费者。
  - 09:55 输入接口闭环：identity pelvis下实际Jump左右分支各选错一次，单字段恢复及有效Feet控制通过，确认D157/P3。按.68已有前置等待真实落地动画结束、读到Standing Idle回调后，pending帧移动20cm的source锁定距离平方0.04，target发布0并漏掉过渡；仅恢复距离便实际排队StandingLeft，6组5PASS/1FAIL，确认D158/P3。此前10/40帧固定预热未到Idle，只计前置失败。当前累计156项（98P2/58P3）；进入本批精简和最终身份Gate，未扩展完整过渡算法。
  - 09:58 完成摘要：最终10项focused为5PASS/5预期语义FAIL，98组91PASS/7FAIL，全部恢复控制通过。删除三个完整角色夹具中的180°约束覆盖，恢复默认90°/40°；删除Jump多帧等待循环，同target重编后全部判定保持一致。6份原生报告、10份ALS/10份UE/23份目标闭包与116份基线source、160份生产Rust、累积原始/Build资产、reader/启动器库身份PASS，证据`71/summary.json`、`71/final/`。实耗37分33秒，主要增量来自两个有效性接口的真实消费者前置；本项覆盖Feet目标/曲线/锁定/瞬移/基座/约束，完整Transition算法、Rig地面偏移/IK及核心图继续。仅提交审计记录，未改生产、转换器、reader或依赖；截图0、性能暂停。

- [x] BF144.72 Foot Offset Trace算法、盒体查询与输入：依赖BF144.71记录Gate，核对CR_Als中实际FootHeight/trace开关/距离/通道输入、源LineTraceSingleByChannel的复杂查询开关与过滤接口、盒体端点/初始命中、世界/VM变换和坡面校正，并沿当帧Rig输出进入偏移消费者。复杂mesh射线核、偏移弹簧、旋转约束、TwoBoneIK和其他Rig程序另行验收；既有Foot Lock与几何根因只作隔离或归并。Gate：fresh Rig/骨架/根图与查询source、同输入原生盒体查询、实际Rig反例或对照、工具缺口的最小自测/消融与身份记录。开始：2026-10-15 10:03 +08:00；预估结束：2026-10-15 10:43 +08:00；实际结束：2026-10-15 11:04:42 +08:00；偏差：+21分42秒；结果：审计记录Gate PASS，新增D159/P3、D160/P2；复杂mesh射线核独立接续。估时依据：源trace单元约60行及UE查询/坐标闭包，target单元与查询约150行，另有Rig实际输入绑定；.70查询闭包实耗40分45秒，本项按40分钟独立验收，必要时补通用原生ray读取。
  - 10:03 活动摘要：.71已提交，累计156项。fresh C++明确trace使用Visibility复杂查询、忽略owner，起止高度相对Rig根而非脚高度；命中normal经InverseTransformVector且不归一化。target同名单元已定位，先核对CR实际绑定与UE查询边界，未把缩放下的源行为当成可优化项。
  - 10:18 source/static闭环：3份原生Rig/mesh/根图已读取，完整Rig程序重编比较与已安装JSON一致；当前实际有两个FootOffsetTrace调用。fresh SQVisitor→RayAabb接受射线终点，零距离命中由FillHitHelper撤掉位置/法线标志，再由CollisionConversions使用起点和反向射线法线；target分别有严格小于端点和保留零法线的候选。现有原生GeometryQueryProbe只支持sweep，5个ray请求虽然reader结束成功却没有ray结果，不能当成源证据；本项只补通用box ray读取及一个边界自测，不改转换规则或生产实现。
  - 10:48 consumer闭环：新增原生ray边界及legacy sphere/capsule自测8组通过，旧报告明确因缺少ray结果失败。查询10组8PASS/2FAIL、空间/坡面数学10组全部通过；完整角色8组6PASS/2FAIL，端点丢失使偏移消费者从source−0.766336m变0，初始重叠零法线使source+0.5m变0，仅恢复当次偏移字段均通过，分别确认D159/P3、D160/P2。Full Rig探针的变量double/单元float读取已校正，并将组件根置于精确原点以排除端点夹具舍入；当前实际四条射线另作同输入原生复算。累计158项（99P2/59P3），继续最小精简和身份记录。
  - 11:01 范围拆分：四条实际Rig射线的原生复算全部匹配消费者oracle；删除重复恢复样本和未使用float变量兼容分支后同target重编，当前26组22PASS/4预期FAIL，恢复控制通过。复杂几何的开关/选择接口已定位，但trimesh有独立背面、三角形命中与法线算法，盒体样本不能验收该核；这部分独立接续下一项，保留原审计目标。当前项只收口trace算法和已核实的盒体查询闭包，完整偏移/IK仍后置。
  - 11:04 完成摘要：3项focused为1PASS/2预期语义FAIL，26组判定与精简前一致，全部恢复通过；5份有效原生报告、8组reader自测和4条实际射线复算有效，旧reader缺字段反例保留。5份ALS/15份UE/9份目标/2份依赖闭包、116份基线source、160份生产Rust及累积资产身份PASS，读取器源码/包/新二进制一致，启动器库未变；证据`72/summary.json`、`72/final/`。实耗61分42秒；首个记录Gate在11:02:23通过，随后删除自测请求的JSON往返复制，生成请求逐字节不变，11:04:42最终复核通过；该收尾跨过原容差上沿1分42秒，期间没有新runtime。额外时间来自原生ray读取扩展及Rig变量/单元标量类型、实际消费者前置校正；复杂mesh核拆为后续独立Gate。只新增读取能力和自测、登记差异，生产实现与转换规则未改；截图0、性能暂停。

- [x] BF144.73 Foot Trace复杂几何选择与背面查询：依赖BF144.72记录Gate，核对实际复杂几何选择、BodySetup单/双面配置、Chaos与Parry射线的背面规则及变换，并沿当前Rig偏移消费者验证。发现的共享边/顶点法线余差和mesh端点/零距离细节独立接续；不复开通用接触动力学，偏移弹簧/旋转/IK后置。Gate：fresh相关mesh/Rig与查询source、同输入原生网格射线、实际Rig消费者和必要消融、身份记录。开始：2026-10-15 11:12 +08:00；预估结束：2026-10-15 12:02 +08:00；实际结束：2026-10-15 11:33:20 +08:00；偏差：−28分40秒；结果：审计记录Gate PASS，新增D161/P2，8组法线余差接续下一项。估时依据：.72盒体与Rig闭环实耗61分42秒，主要额外成本为首次原生ray能力和消费者类型校正；本项复用该入口及Rig夹具，范围约150行mesh射线与生产复杂几何选择，按50分钟独立验收。
  - 11:12 活动摘要：.72已提交，累计158项（99P2/59P3）。当前publish_control_rig_inputs选用ComplexCollider并同步物理Collider缩放；源mesh visitor有背面剔除开关，实际BodySetup与cooked值尚需原生核对。先建立查询源闭包及三角形同输入控制，未将旧接触几何Gate当作ray Gate。
  - 11:21 source/static闭环：fresh 12种生产复杂网格共972条native射线，实际BodySetup均非双面，复杂形状均启用culls_back_face；源BodyInstance按复杂标志筛选后调用同一Chaos mesh ray并转换命中，target默认Parry ray不剔背面。原生入口增加通用mesh ray读取，旧reader的12个请求均缺结果作为阴性证据；当前只完成读取和静态候选，正以生产bridge网格和同输入culling控制核验，尚未计为新增缺陷。
  - 11:26 consumer闭环：972组查询173FAIL，仅恢复源culling后165组恢复，8组相同命中位置但棱边/顶点法线不同，继续独立核对。完整角色Barrel复杂网格8组6PASS/2FAIL：射线从桶内向下时，源无命中，target偏移−0.261963m/−0.542933m；仅移除当前ray背面三角形后实际偏移消费者恢复0。正面、非均匀缩放及简单/复杂选择控制通过，确认D161/P2；累计159项（100P2/59P3），未修复生产。剩余8组不计为无差异。
  - 11:30 范围拆分：fresh Arch01/Arch02/Fountain原生顶点和面索引复算确认，8组余差均位于共享边/顶点邻域；UE的double三角形计算、float当前长度与遍历覆盖，和Parry的float严格最小值选择属于独立闭包。该余差及mesh端点/零距离细节接续独立查询边界项，不用D161解释或放宽法线阈值。本项收口复杂形状选择和背面剔除；下一步复算实际Rig四条射线、做夹具精简和身份Gate，再继续余差。
  - 11:33 完成摘要：2项focused均记录语义FAIL，978组803PASS/175FAIL；其中165个query和2个完整Rig失败归D161，8组法线余差继续保留。精简每个世界的重复bridge更新、两个恢复样本和位置缓存后，972组输出逐字相同，六个保留Rig判定相同、两个恢复通过。8份有效native、12组mesh/8组旧query自测、4条实际Rig射线复算、4种metadata消融和Rig程序身份PASS；1份ALS/10份UE/30份目标/5份依赖闭包、116份基线source、160份生产Rust、累积资产及reader源码/包/二进制、启动器库身份PASS。证据`73/summary.json`、`73/final/`；实耗21分20秒，无超时。只扩展通用读取和自测，没有生产或转换规则修改；截图0、性能暂停。接续网格边界后再进入偏移/IK。

- [x] BF144.74 复杂网格共享边/顶点及射线端界：依赖BF144.73记录Gate，追踪8组法线余差的实际原生候选、三角形数值、距离收紧和遍历选面；核对未缩放/缩放mesh的终点、零距离和近边界行为，并将有差异的结果送入实际Rig输入消费者。背面规则使用已证实控制，原生接触动力学保持关闭。Gate：fresh源核/网格与同输入native取证、逐因素最小反例或排除、实际Rig消费、消融和身份记录。开始：2026-10-15 11:36 +08:00；预估结束：2026-10-15 12:21 +08:00；实际结束：2026-10-15 12:40:45 +08:00；偏差：+19分45秒；结果：审计记录Gate PASS，新增D162/P3并补充D159，余差完成本批消费者分类。估时依据：.73读取/矩阵/消费者实耗21分20秒；本项剩余8组集中在源约350行BVH和约60行triangle核，已定位公开GetBVH取证入口，另需独立数值与消费者控制，按45分钟验收。
  - 11:36 活动摘要：.73已提交，累计159项。8组source法线均匹配fresh原生面，三个Arch01位于共享边、两个Arch02位于边邻域、三个Fountain位于顶点邻域；部分样本存在多个有效候选，不能直接将全部余差归咎于相同距离的先后次序。先读取实际native候选和长度变化，再区分精度、容差和选面影响。
  - 12:02 source/consumer闭环：新增原生逐面记录与Core.Raycast逐值核对，8条源射线47次访问全部有效；缩放倒数必须读取实际FVec3f缓存，早先重算double倒数的探针不匹配已保留失败并纠正。Fountain完整Rig9组6PASS/3FAIL，三个顶点样本的法线进入实际足旋转输入，1cm内点控制全PASS；仅恢复当次trace的源输出均通过，确认D162/P3。已消除夹具在远处带缩放构造时的矩阵舍入输入干扰，未放宽阈值、未计为生产缺陷。累计160项（100P2/60P3），五个Arch余差与mesh端界继续核对。
  - 12:35 闭包与消融：83个逐面输入全部匹配独立源公式，17处Parry候选差异由容差、float计算与距离资格解释；两个Arch02选面在源厘米/Core输入控制中恢复，完整默认长度Rig另确认一处棱边法线差异，归D162。原五条长射线按实际50/80cm绑定映射到Rig空间后，坡度均低于45度门槛，source/target共10组均回退0/Up，不把raw差异直接计为额外运动缺陷；倒置Arch01的默认长度与内点控制通过。12组mesh端界中两处未缩放端点失败补充D159，缩放严格端界与零距离控制通过。去掉恢复选择字段及两个重复恢复样本后，同target重编139组101PASS/38语义/诊断FAIL，完整Rig16组12PASS/4FAIL，四个恢复全通过。读取器按实际float左操作数保留缩放结果，38条完整命中/位置/法线交叉核验、20条旧查询回归及5种字段消融通过；早先只比面编号漏掉的类型问题已由旧数据反例封住。继续身份记录后进入偏移/IK。

  - 12:40 完成摘要：5项focused为1PASS/4语义或诊断FAIL，139组101PASS/38FAIL；全部83个独立源三角形复算、38条完整wrapped查询及20条旧查询回归通过，5种metadata消融与旧提升精度反例均被拒绝。3份ALS/12份UE/34份目标/5份依赖闭包、116份基线source、160份生产Rust、累积资产、reader源码/包/二进制和启动器库身份PASS；证据74/summary.json、74/final/。实耗64分45秒，在允许容差内；额外耗时来自原生缩放缓存和左操作数结果精度校准，以及拱门实际消费者与walkability排除对照，已将这些漏检边界落实为读取器强校验。没有生产或转换规则修改，截图0、性能暂停；接续Foot Offset的弹簧、旋转与IK。

- [x] BF144.75 Foot Offset位置、骨盆偏移与弹簧状态：依赖BF144.74记录Gate，核对CR_Als实际RefreshPelvisOffset与两侧ApplyFootOffsetLocation的程序接线、默认参数、初始化/零delta/连续帧弹簧状态、腿长限制和位置消费者；已登记查询差异以共同trace输出隔离，旋转与TwoBoneIK留后续。Gate：fresh Rig/mesh原生记录及ALS/UE源闭包、独立源公式与实际程序/完整角色消费者、必要消融及身份记录。开始：2026-10-15 12:49 +08:00；预估结束：2026-10-15 13:39 +08:00；实际结束：2026-10-15 13:04:46 +08:00；偏差：−34分14秒；结果：审计记录Gate PASS，本项无新增差异。估时依据：.72/.73已建实际Rig参数与完整角色夹具，.74实耗64分45秒主要用于原生mesh类型校准；本项不新增查询能力，约300行位置/两类spring算法及一段Rig函数闭包，按50分钟独立验收。
  - 12:49 活动摘要：.74已提交，累计160项未修复差异。位置单元初读与ALS的上下限和初始化顺序一致；继续fresh原生Rig取证、弹簧核与实际骨盆/两脚消费者核对，不将旧位置单元测试当成完整状态闭包。截图0、性能暂停。
  - 12:59 source/focused闭环：fresh Rig/mesh/root三份原生记录PASS，165指令完整重编与已安装程序一致；骨盆先对两脚minimum限于−30/40cm，在2Hz临界阻尼弹簧前后各乘输入权重，脚部采用12Hz/2倍阻尼并保留两阶段初始化，PoseMoving把最小骨盆距离从20插值到50cm。独立厘米公式下240帧、466次脚位置计算和150次腿长裁剪通过；7帧输入无效时保留源图既有偏移，恢复后状态继续。32帧实际角色/碰撞查询消费者、零owner delta、骨盆写回和两侧IK目标接线通过；最大当前位置差低于0.1µm。尚未新增缺陷，继续消融及身份Gate；足旋转与IK求解算法未算入本项。
  - 13:04 完成摘要：正常272组全PASS，最大位置差0.068µm；去掉脚弹簧持久状态后223组失败，去掉腿长限制后110组失败，去掉骨盆第二次权重在0.25权重消费者被拒绝。按源码字节恢复并同target重编，272组再通过，240组程序输出逐字相同。fresh原生腿长82.7687225cm和FootHeight、6份ALS/8份UE/13份目标闭包、116份基线source、160份生产Rust、累积资产、reader与启动器库身份均PASS；证据`75/summary.json`、`75/final/`。实耗15分46秒，复用现有原生和角色夹具使耗时低于预估；没有生产/转换规则修改，累计160项仍未修复。截图0、性能暂停，继续足旋转/IK。

- [x] BF144.76 Foot Offset旋转、平滑与角度约束：依赖BF144.75记录Gate，核对ApplyFootOffsetRotation的初始足/小腿空间、法线连续帧平滑、BetweenVectors、旋转组合、Euler轴/符号与源约束区间，并沿完整Rig核对两脚IK之后的旋转写回。使用共同trace法线隔离D161/D162；TwoBoneIK求解及其他Rig单元另行验收。Gate：fresh Rig/mesh与ALS/UE旋转闭包、独立源公式和边界/连续帧/实际角色消费者、必要消融及身份记录。开始：2026-10-15 13:07 +08:00；预估结束：2026-10-15 13:52 +08:00；实际结束：2026-10-15 13:46:40 +08:00；偏差：−5分20秒；结果：审计记录Gate PASS，新增D163/P3。估时依据：.75复用原生与完整角色夹具实耗15分46秒；本项源单元约百行，另含double quaternion/Euler与近奇异边界的约200行数学闭包，预计需要额外坐标和组合顺序反例，按45分钟独立验收。
  - 13:07 活动摘要：.75已提交，累计160项。现有查询证据只证明法线进入足旋转输入，尚未验证该单元的转换、平滑和约束；本项重新读取实际源默认值与数学实现，不按旧姿态截图或上一项位置通过推定旋转一致。截图0、性能暂停。
  - 13:10 source闭包更新：fresh Rig指令证明顺序为位置偏移→TwoBoneIK→足旋转约束→SetRotation；因此本项以实际post-IK小腿/足旋转作为输入，验收到两脚加权写回，IK求解本身仍留下一项。Swing2由PoseMoving从[−15,5]插值到[0,0]，Swing1为[−20,40]、Twist为[0,0]；继续独立源坐标公式与边界矩阵。
  - 13:27 source/consumer摘要：三份原生资产与165指令身份通过；5720次常规/奇异邻域/半转单元步骤、288组实际程序加权写回与32帧真实角色消费者通过。900个精确四元数的原生Rotator与独立源公式全匹配，其中36个正负号样本在目标Euler的Roll ±180表示上不同；八个真实参考骨架的受控反向姿态又在组合舍入边界出现大角度限位差异，尚不直接归为同一根因。读取器仅补通用四元数连乘与BetweenVectors取证，继续核实安装引擎的实际乘法、参考旋转和当帧消费者；未新增缺陷编号，未展开IK求解。
  - 13:41 consumer闭环：原生参考旋转、当前/目标组合及完整角色实际float输入的六阶段采样，均确认同一源角度分支；真实参考骨架八个半转单元反例失败，±0.0001°的16个邻域控制通过。完整角色只控制IK之后的calf/foot输入，保留实际坡面查询、FootTarget输入和加权写回，左右脚最终分别偏离177.4759°/130.7921°；仅恢复源旋转输出，两侧都通过。确认D163/P3，累计161项（100P2/61P3）；这是受控post-IK边界证据，不宣称默认动作自然触发。独立复核排除了轴/旋转顺序、Inverse、奇异阈值和平滑公式的一般性错误；继续精简读取器与夹具、回归和身份记录。
  - 13:46 完成摘要：6项focused为3PASS/3语义或诊断FAIL；6968个核对样本6922PASS/46FAIL（900原生Euler中的36个表示差异、8个半转单元、2个受控完整角色），普通5720步骤/288程序帧/32角色帧全PASS。删除任意链长上限、复用注入权重常量后，同target重编保留数值逐字一致；6个新增原生自测前后相同，3种缺字段消融被拒绝，20个旧查询回归通过。3份ALS/9份UE/16份目标闭包、116份基线source、160份生产Rust、累积资产、reader源码/包/二进制及启动器库身份PASS；证据`76/summary.json`、`76/final/`。实耗39分40秒；已计入原生数学取证与受控消费者的额外成本，无超时。没有生产或转换规则修改，截图0、性能暂停；接续TwoBoneIK/pole及半转自然可达性。

- [x] BF144.77 Foot IK膝盖方向与平滑输入：依赖BF144.76记录Gate，核对CalculatePoleVector的当前骨骼/投影/退化与成功标志、40cm方向偏移、DamperExactVector的初始化和连续帧状态，闭合到实际两侧TwoBoneIK的pole输入。IK位置/旋转求解及D163自然可达性留下一项，不扩展无源调用的通用向量库。Gate：fresh Rig/mesh与源向量/平滑闭包、边界及生命周期反例或排除、实际程序与角色输入消费者、最小消融和身份记录。开始：2026-10-15 13:52 +08:00；预估结束：2026-10-15 14:27 +08:00；实际结束：2026-10-15 14:18:41 +08:00；偏差：−8分19秒；结果：审计记录Gate PASS，本项无新增差异。估时依据：.75位置与状态闭包实耗15分46秒、.76含原生半转数学取证实耗39分40秒；本项复用同一Rig入口和原生能力，限定约百行投影/平滑算法及其两侧调用，按35分钟独立验收。
  - 13:52 活动摘要：.76已提交，累计161项。源指令明确pole在TwoBoneIK之前读取当帧层级；位置单元只生成效应器目标，不把该目标写回层级。本项核对这一输入时点、投影与平滑状态；后续求解和半转可达性保持独立，截图0、性能暂停。
  - 14:12 source/focused闭环：三份fresh原生记录与165指令完整身份通过；source按各分量≤1e−4cm判断AB退化，按平方长度>1e−8cm²归一化AC/方向，两个单位换算阈值分别保留，不能合并成同一1µm判断。39组投影/退化/窄阈值区间、72组程序帧和32帧实际角色输入全部通过，含47次失败保留、4帧输入暂停及零delta。首次失败仍以默认Forward×40cm初始化平滑，是源图行为；成功标志/投影无下游读取，实际消费BLocation+Direction×40cm后经0.05秒半衰期平滑的结果。首版夹具误计同类型的两条手臂IK调用，已按原生脚部指令91/121修正范围；不计为生产缺陷。当前无新增差异，继续消融与身份Gate。
  - 14:18 完成摘要：3项focused、143组全部PASS，IK pole输入最大位置差5.96nm。去掉失败结果保留后64帧失败，每帧重置平滑状态后65帧失败；按源码字节恢复并同target重编，39组几何/72程序帧逐字一致，32角色帧全部恢复通过。4份ALS/4份UE/14份目标闭包、116份基线source、160份生产Rust、累积资产、reader及启动器库身份PASS；证据`77/summary.json`、`77/final/`。实耗26分41秒，范围中两条额外手臂IK仅作调用身份澄清，未展开其玩法或道具。未修改生产/转换规则/读取器，累计161项保持不变；截图0、性能暂停，接续共享TwoBoneIK求解。

- [x] BF144.78 TwoBoneIK输入与位置求解：依赖BF144.77记录Gate，核对当前四个TwoBoneIK调用的骨长来源、effector/pole空间、零权重入口，以及位置核的距离退化、弯曲平面、伸展与不可达目标分支；用权重1的实际消费者闭合位置结果。旋转对齐、部分权重的层级写回和D163自然可达性在位置闭包后独立验收；手臂只核对共享求解器现有调用数据，不扩展道具或玩法。Gate：fresh Rig/mesh与UE求解闭包、独立源几何公式/必要原生取证、边界与实际输入消费者、消融及身份记录。开始：2026-10-15 14:41 +08:00；预估结束：2026-10-15 15:31 +08:00；实际结束：2026-10-15 15:29:55 +08:00；偏差：−1分05秒；结果：审计记录Gate PASS，新增2项P3，未修复。估时依据：.77投影/状态闭包实耗26分41秒，.76原生数学与消费者实耗39分40秒；本项位置核约140行、单元入口约110行及四组调用数据，预留原生核验后按50分钟验收，将旋转和部分权重写回拆开。
  - 14:41 活动摘要：.77已提交，累计161项。fresh调用身份为两脚91/121、两臂139/146；本项沿同一共享类型核对输入和位置核，不把足部pole已通过当成IK求解已等价。当前无运行中的审计进程，截图0、性能暂停。
  - 14:55 source/static闭环：3份fresh原生资产与165指令完整身份PASS，4份ALS/9份UE/14份目标文件已固定。四个调用均LengthA/B=0、Stretch=false；脚部使用全局pole，手臂使用对应lowerarm空间且只施加旋转/平移。源和目标都以A的当前缩放计算第二骨段长度，不能当成错误单独修正；位置核保留double厘米计算、float骨长入口、默认轴和1e−4cm退化阈值。独立厘米公式、四组实际调用和完整角色位置消费者正在执行，旋转/部分权重仍按原拆分保留。首版夹具误用不存在的post-call字段和重名Name类型，编译已明确拒绝，改用现有hierarchy快照；未计为生产缺陷。
  - 15:15 focused/native闭环：位置核13824组中288组在1µm退化边界分叉；四调用480组中5组在接近完全伸直时超过10µm位置合同。288组核边界与160组有效单元输入均由新取证入口直接调用UE `SolveBasicTwoBoneIK`，独立厘米公式逐值相同；32帧实际角色和28组缺项/骨长守卫通过。原生工具6项新增自测、26项既有查询/旋转回归及4个缺失输出反例通过。完整角色受控输入与恢复正在核验；默认动作自然可达性尚未证明。工具消融移除对结果无影响的初始joint输入，再编译并重跑480组；源取证扩展不修改转换规则。首版完整角色夹具把接近1的动画scale误当精确1，现以明确1.25缩放固定输入，未将该前置失败计作生产问题。

  - 15:29 完成摘要：5项focused共14380组，14083PASS/297FAIL，失败为288个精确退化输入、5个近伸直骨长输入和4个完整角色反例；仅按两项根因登记D164/D165。464组原生核验与独立公式逐值相同，源坐标控制消除位置核分叉，四个实际角色位置恢复及八个邻域控制通过。去掉取证接口初始joint参数后480组结果不变，保留最小接口；6项新增/26项既有原生自测及4个缺失输出反例通过。6份ALS/9份UE/14份目标、116基线source、160生产Rust、累积资产、reader及启动器库身份PASS，证据`78/summary.json`、`78/final/`。实耗48分55秒；实际伸展开关关闭的分支只记共享核控制，未宣称默认动作自然触发。累计163项（100P2/63P3），生产与转换规则未改，截图0、性能暂停；接续IK旋转/部分权重写回与边界可达性。

- [x] BF144.79 TwoBoneIK旋转与部分权重写回：依赖BF144.78记录Gate，沿同一四个调用核对Primary/Secondary轴、FindBetweenNormals反向分支、完全伸直判定、次轴投影、骨旋转归一化，以及Weight部分混合后的局部位置重建与逐骨层级写回。使用共同位置输入隔离D164/D165，保留源当前A旋转初始化B的真实语义；仅核对既有调用，不展开道具表现。D163–D165的默认动作自然可达性在本项数学/写回闭合后独立验收。Gate：fresh Rig/mesh与UE旋转/层级源码、原生与独立公式、四调用/完整角色消费者及边界控制、消融和身份记录。开始：2026-10-15 15:32 +08:00；预估结束：2026-10-15 16:22 +08:00；实际结束：2026-10-15 15:55:25 +08:00；偏差：−26分35秒；结果：记录Gate PASS，本项未发现新增差异。估时依据：.78位置核及原生扩展实耗48分55秒，.76旋转原生数学与消费者39分40秒；本项旋转核约85行、混合入口约35行及直接层级消费者，沿既有工具扩展旋转证据，将自然动作矩阵另拆。
  - 15:32 活动摘要：.78已提交`41042ff8`，累计163项。当前生产/原始资产未改，无审计进程运行。只沿Forwards Solve现有四个IK的旋转与写回边核对，位置差异使用本轮原生共同输入隔离；截图0、性能暂停。
  - 15:55 完成摘要：3项focused共2144组PASS（1728旋转核、384四调用/权重/层级、32完整角色帧含128调用，其中96次部分权重）；2240组原生核验通过，32项既有/3项新增reader自测、2个缺失输出反例通过。伸直极向量、次轴投影、权重后位置重建删减分别产生368/1152/192个反例，reader删减4个反例及9项恢复一致。极小权重的初版完整角色预期遗漏源层级近似不写回规则，按fresh源码补齐后通过，未记移植差异。6份ALS/15份UE/14份目标、116基线source、160生产Rust、累积资产及二进制身份PASS；证据`79/summary.json`、`79/final/`。实耗23分25秒；只扩展原生旋转取证与自测，生产/转换规则未改，累计仍163项（100P2/63P3）。本项覆盖受控姿态与权重；接续D163–D165自然可达性，截图0、性能暂停。

- [x] BF144.80 足部IK边界的默认动作可达性：依赖BF144.76–79记录Gate，仅沿D163的post-IK旋转输入及D164/D165的实际effector/骨长链，核对足目标限位、spring状态、Pelvis/Thigh关系和四个IK权重入口。先推导source输入约束与target实际数据，再在未注入IK/骨变换的动作路径采样，按证据区分自然触发、已证明排除与仍未证明；不把有限采样未触发写成不可达。需要公共runtime时先完成静态闭环，并沿既有Playground入口关闭截图运行；不修生产、不展开道具表现或其他动画图。Gate：fresh输入闭包、源约束与采样对应、原生边界复核及记录完整性。开始：2026-10-15 15:58 +08:00；预估结束：2026-10-15 16:48 +08:00；实际结束：2026-10-15 16:38:30 +08:00；偏差：−9分30秒；结果：记录Gate PASS，本次矩阵未自然触发三项边界，未新增差异。估时依据：.75位置链实耗15分46秒、.79旋转/写回23分25秒；本项复用已验证数学，只新增实际输入约束和受限动作矩阵，公共入口及未闭合动作分支按事实拆分。
  - 15:58 活动摘要：.79已提交`00c346dc`，三个数学边界已有受控反例，尚未证明默认动作触发。只沿已登记输入边继续，累计163项，截图0、性能暂停。
  - 16:20 runtime前摘要：fresh Rig/mesh/root与10份ALS/15份UE/17份目标闭合，165指令产物逐值一致。源码确认位置限位在spring之后，但最小高度先Min再Max且有历史，不能据此宣布退化距离不可达。报告采集器只保留现有10个相关调用的double输入；focused检查PASS，降float消融FAIL、按源码字节恢复重编PASS，900组原生Euler自测及公共validator自测PASS。整包测试曾被无进展watchdog结束，已缩到单函数检查；同`target_game`真实入口预构建PASS。开始无driver startup，再检查站立方向/转身及蹲姿路径；采样缺口或既有Gate失败原样记录，不扩大到其他图修复。
  - 16:28 runtime后摘要：公共startup、standing_direction、standing_turn、crouching全部功能PASS，截图0；原报告保留暂停的evaluation性能超预算。四个入口共2895采样帧、5782次有效足IK/足旋转，未注入IK、骨变换或曲线；方向矩阵和动作切换沿既有输入驱动，初始化/重定位如实保留。样本最小髋足距离0.187m、最小伸直余量8.276mm、最小半转表示余量103.665°，三个边界均未触发；默认双臂权重为0，不据此评价有效手臂求解。正在原生复核全部有效足IK和32个最接近旋转边界的样本，结论只覆盖本次矩阵，不能泛化为所有动作/历史不可达。
  - 16:38 完成摘要：5782组原生IK逐一核对通过，最大位置差9.12e−8m、下段骨旋转差2.21e−7rad；32个旋转样本经128步原生初始关系/连乘/Euler复核一致，最终足旋转复算通过。完整source/资产/生产Rust身份、reader及实际入口二进制身份PASS，证据`80/summary.json`、`80/final/`及公共`bf144_80_*`报告。实耗40分30秒；新增内容仅为已有报告的double调用取证和focused自测，生产算法未改。累计仍163项（100P2/63P3）。未采样动作/历史与有效手臂IK继续保留未证明边界；公共功能结论不代表性能验收。

- [x] BF144.81 Dynamic Transition资格与选择队列：接续.71已证实的`RefreshFeetOnGameThread→RefreshDynamicTransitions`消费者，依赖.80记录Gate，核对RefreshTransitions许可曲线、动态过渡延迟/资格、左右脚距离/权重、站蹲动画选择和队列字段的生命周期，闭合Standing/Crouching直接图调用与实际队列消费者。已登记D158等输入历史问题隔离，不复开Foot Lock；蒙太奇播放实现、Quick Stop和通知调用另项验收，不展开无关姿态子图。Gate：fresh配置/直接AnimBP调用与C++合同、focused边界及实际角色队列/消费者、消融与身份记录。开始：2026-10-15 16:44 +08:00；预估结束：2026-10-15 17:34 +08:00；实际结束：2026-10-15 17:03:41 +08:00；偏差：−30分19秒；结果：记录Gate PASS，新增D166/P3，未修复。估时依据：fresh源索引显示RefreshTransitions至动态过渡选择约105行，直接调用来自.71已定位的Standing Idle及其站蹲对应链；.71输入/消费者实耗37分33秒、.79数学闭环23分25秒，本轮纳入配置与图入口后按50分钟独立验收。
  - 16:44 活动摘要：.80已提交`0d3e7d0d`，四公共入口功能通过；有限自然采样未触发D163–D165，其他动作/历史随对应闭包继续保留。当前转向已证明的动态过渡输入边，累计163项，生产算法未改，截图0、性能暂停。
  - 16:58 source/static与consumer闭环：fresh配置/root/Standing/Crouching原生读取通过，直接Idle回调2/2已覆盖；8cm阈值、两帧延迟、许可/相关权重、同帧去重、站蹲四资源和队列参数一致。560组普通资格与两组24帧生命周期通过；681组数值边界出现68处判定/选脚差异。完整角色84组受控输入中6处错误进入实际播放请求，只替换距离资格/排序后42组恢复全通过，确认D166/P3，累计164项（100P2/64P3）。首版夹具误读强制输入后的旧delay缓存已修正，未记作生产问题；下一步去掉无效夹具字段、消融和身份Gate。蒙太奇时间/混合、停止/通知顺序仍后置。
  - 17:03 完成摘要：3项focused为1PASS/2预期FAIL，共1373组1299PASS/74FAIL；68个数值边界与6个实际角色失败均由D166解释。去掉两个无用原点字段并显式按源坐标顺序求和后，同target重编的156行语义结果全部一致。8份ALS/5份UE/14份target、116基线source、160生产Rust、累积资产、reader及.80实际入口库身份PASS；证据`81/summary.json`、`81/final/`。实耗19分41秒，源设置路径查找及静态资源名解析的夹具错误已改正并通过；生产、转换器、原始资产均未改，没有重启公共runtime。后续为动态蒙太奇的时间/混合/停止及通知调用，D166自然可达性仍未证明；截图0、性能暂停。

- [x] BF144.82 Dynamic Montage播放时钟与自然结束：依赖.81记录Gate，沿PlayQueuedTransitionAnimation→PlaySlotAnimationAsDynamicMontage核对四个动态过渡资源的时长/RateScale、起播位置/速率、逐帧时钟、片段边界及自动blend-out触发，闭合到实际角色Transition槽的采样时间。只核对当前运动过渡；AlphaBlend权重、组内中断、停止请求及通知顺序另项验收，不复开已完成Roll/Mantle或扩展道具蒙太奇。Gate：fresh四资源与UE调用闭包、独立时钟/边界及实际采样消费者、必要消融和身份记录。开始：2026-10-15 17:06 +08:00；预估结束：2026-10-15 17:56 +08:00；实际结束：2026-10-15 17:47:15 +08:00；偏差：−8分45秒；结果：记录Gate PASS，新增D167/P3并扩展D118/D120，未修复。估时依据：.81选择与队列实耗19分41秒；fresh索引定位UE动态构造约80行、实例Play约60行及Advance中的片段推进/自然停止分支，预计需要四资源提取和首末帧对照，按50分钟拆分，混合权重与中断另行验收。
  - 17:06 活动摘要：.81已提交`6ad869db`，累计164项（100P2/64P3）；生产、转换规则未改。当前只沿已证明的动态队列→播放→采样边核对时间语义，距离选择使用已确定的共同输入隔离D166；截图0、性能暂停。
  - 17:31 source/static摘要：四个Sequence原生时长均1.5333333015秒、RateScale=1且root motion关闭；现有时钟探针增加通用瞬态动态Montage构造和可选起播参数，10068帧与独立源推进公式逐值相同，旧GetUp的1094帧回归不变。确认Transition也遗漏已登记D120的section末端偏移，正在四资源实际槽采样恢复；另将六位小数时长和elapsed反算两个临界分支分离，24个新增原生边界帧已通过公式复核，实际触发消费者仍在验证。仅沿共同源Montage规则补充现有开放差异，未复开GetUp完成项；混合权重/停止请求仍后置。
  - 17:36 consumer摘要：四资源与半速/倍速的实际Transition槽确认D120尾段时间差，七组原生位置恢复全部通过（含零速控制）；半速的末端前累计误差另由D118的反算时钟解释。六位小数时长在实际站立播放状态提前混出，单独恢复时长即通过，登记D167/P3；蹲姿时长本已正确，仍在另一临界输入晚一帧，归并D118。普通124条轨迹的停止帧全部一致，不将受控边界泛化成常见故障。累计165项（100P2/65P3），继续读取器参数消融、源槽消费与身份收尾。
  - 17:47 完成摘要：4项focused均按已确认差异失败；190条普通/边界/恢复轨迹为100PASS/90FAIL，包含124条单元时钟和26条实际角色轨迹；10092帧原生参考与独立公式逐值一致。实际根Transition槽节点确已消费采样；时长恢复将控制反例约0.05975的后续权重偏差归零。读取器6组验证、旧1094帧不变、4个参数消融、5个恢复控制、3个报告缺失/篡改反例与float溢出输入拒绝通过。删去参考公式无用循环上限及一条重复边界后，同target重编结果不变。4份ALS/8份UE/8份target、116基线source、160生产Rust、累积资产、reader源码/package/DLL与入口库身份PASS；证据`82/summary.json`、`82/final/`。实耗41分15秒，仅扩展取证工具及自测，没有生产/转换器修改或公共runtime重启。AlphaBlend完整权重、显式停止/组替换和通知仍依赖后续闭包。

- [x] BF144.83 Dynamic Montage单实例混合权重与停止：依赖.82记录Gate，核对FAlphaBlend初始化、HermiteCubic值域与连续更新、自然混出、显式停止/零时长/重复停止/缩短淡出的状态和清理条件，并验证实际Transition槽消费的单实例权重。时钟与时长使用原生共同序列隔离D118/D120/D167，多个实例的组内混合及停止请求/通知调度另项验收。Gate：fresh源混合/停止合同、当前动态资源原生帧、focused状态边界与实际权重消费者、消融和身份记录。开始：2026-10-15 17:51 +08:00；预估结束：2026-10-15 18:41 +08:00；实际结束：2026-10-15 18:31:20 +08:00；偏差：−9分40秒；结果：记录Gate PASS，新增D168/P3，未修复。估时依据：fresh AlphaBlend更新约110行及Montage Stop约110行，.82原生时钟和消费者实耗41分15秒；本项复用原生probe及四资源事实，限定单实例混合/停止，以50分钟独立验收，组内混合和通知另拆。
  - 17:51 活动摘要：.82已提交`137a2024`，累计165项（100P2/65P3），取证工具已可直接运行动态Montage。当前只沿单实例权重→实际Transition槽消费边推进，生产算法未改；截图0、性能暂停。
  - 18:07 source/static摘要：四资源184条轨迹8020帧的原生位置/权重/valid/stopped与独立公式逐值一致；目标权重在1e−6合同内全部通过，20条零权重停止轨迹仅有晚清理标志，尚无运动影响证明，不先计缺陷。fresh `SlotEvaluatePose`在单槽权重≤1e−5时直接保留基础曲线，而目标在>1e−6时预混后以槽权重1消费；只沿这一已证明的直接边补实际根槽/AllowTransitions消费者。Montage资产样本合并对montage有免重复乘权的分支，排除“权重平方”猜测；ALS停止入口仅接受IsActive实例，重复Stop求解器控制不代表实际调用可达性。
  - 18:23 consumer闭环：实际站立/蹲姿×两种共同基础曲线×8权重×原始/恢复共64组，8个原始小权重样本曲线不符，其中4个进入下一帧RefreshTransitions许可；仅在预混前恢复源总槽权重门槛后32组恢复通过，确认D168/P3，累计166项（100P2/66P3）。初次夹具把Idle回调出现当成稳定Idle，导致许可前置失败；改为等实际根曲线全权重后控制成立，未计为产品缺陷。零权重停止的8个实际输出均与基础Pose/全部曲线完全相同且无montage样本，active差异只留在当前诊断状态，未作运动缺陷；停止调用资格和多实例总权重仍后置。
  - 18:31 完成摘要：3项focused为1PASS/2预期FAIL；256组比较228PASS/28FAIL，20组只是不影响已核对单槽运动输出的零权重状态差，8组运动失败归D168。8020原生帧精确复算，所有目标混合权重在1e−6合同内；8个零权重实际Pose/全部曲线逐值一致，32个门槛恢复通过。删除无用位置字段、合并两个总是同时变化的参考重置标志后，同target重编的256行结果不变。5份ALS/13份UE/11份target、116基线source、160生产Rust、累积资产、reader和入口库身份PASS；证据`83/summary.json`、`83/final/`。实耗40分20秒；本项仅更新审计记录，没有生产、转换器或读取器改动。接续多实例总权重与组内混合、调用资格/停止顺序，截图0、性能暂停。

- [x] BF144.84 Transition槽多实例权重与加性混合：依赖.83记录Gate，沿GetSlotWeight→SlotEvaluatePose核对当前动态过渡同时存在时的权重合计、相关性/归一化门槛、加性姿态顺序和曲线合成，闭合到实际根槽。使用共同输入隔离既有时钟差异；本项只审计同槽合成，实例创建/替换、跨槽Grounded组停止和通知调度另项验收。Gate：fresh源累计/归一化与四资源事实、边界和非交换旋转控制、实际多实例槽消费者、消融及身份记录。开始：2026-10-15 18:36 +08:00；预估结束：2026-10-15 19:26 +08:00；实际结束：2026-10-15 19:38:00 +08:00；偏差：+12分00秒；结果：记录Gate PASS，新增D169/D170并扩展D168，未修复。估时依据：fresh SlotEvaluatePose有效加性闭包约130行、GetSlotWeight约60行，target预混与应用约100行；.82时钟闭包41分15秒、.83单实例闭包40分20秒，复用现有角色与原生数学工具，按50分钟将合成与调度分开验收。
  - 18:36 活动摘要：.83已提交`e71edc12`，累计166项（100P2/66P3）。源按槽总权重判断相关性，归一化仅在总值>1+1e−5时执行；当前核对完整多实例消费，不以提高单实例阈值代替该规则。生产未改，截图0、性能暂停。
  - 19:20 source/consumer摘要：6资产fresh读取与50个原生加性累积样本通过，独立公式最大差1.11e−16；新接口调用UE公开姿态累积入口，覆盖实际内部路径，旧35个几何/旋转/IK样本逐值不变。普通双层次序、缩放组合和常规超额权重控制通过；微小超额总权重提前归一化确认为D169/P3，近全权重旋转遗漏源full-weight分支确认为D170/P3。原一般姿态容差未拒绝约0.7–1.6e−6的近全权重旋转差，新增由源“旋转已满权重”规则约束的专项合同及1/0.99998控制后拒绝，未改生产阈值。双9e−6层的逐层Pose/Curve相关性也不一致，沿相同过滤根因归并D168；继续最终消费者许可、消融和身份记录。累计168项（100P2/68P3）。
  - 19:38 完成摘要：2项focused均按已证实差异失败；236组为193PASS/43FAIL，其中176个实际槽比较含28FAIL（D168/D169各8、D170为12），88个源恢复全部通过。双小权重的8组均使未额外修改的基础曲线在下一帧丢失过渡许可。删除参考输出中无用旧骨骼数据及16个重复零第二层配对后，同target重编保留用例的判定不变；原生顺序/负四元数/空层控制及3个结果缺失/篡改反例通过。2份ALS/10份UE/9份target、116基线source、160生产Rust、累积资产、reader源码/package/DLL与入口库身份PASS；证据`84/summary.json`、`84/final/`。实耗62分钟，偏差在允许窗口内；额外时间用于公开原生累积入口与近全权重专项合同。私有接口调用及将primitive校验器用于混合报告的错误均已定位，分别改用公开入口与旧35样本逐值回归；没有生产/转换器修改或公共runtime重启。接续实例替换/停止调度与通知，截图0、性能暂停。

  - 2026-10-16 01:40 有限复核完成：fresh AnimInstance.cpp:4648–4669证明weight≤1e−5的实例在MontageEvaluationData生成时已被剔除，旧摘要关于保留这些权重参与总和的解释作废。补齐该上游资格后，原236组逐项判定仍为193PASS/43FAIL，88个恢复全部通过；D168–D170仍成立，无新增缺陷。旧50个原生加性样本在读取器源码按字节恢复、同target重新构建后逐值不变。证据`91/recheck84.json`、`91/recheck84_final/`；开始01:36，预估01:51，实际01:40:44，偏差−10分16秒，实耗4分44秒。初次复测只暴露夹具在空层时返回稀疏基础Pose却强取完整通道，补全既有参考骨架默认值后完成复测；未改生产或阈值，重新勾选并继续.91。

- [x] BF144.85 Grounded组替换与槽停止资格：依赖.84记录Gate，沿动态过渡/原地转向共组播放调用核对SlotGroup身份、替换时对旧实例的停止策略、已混出实例的相等/更短/更长时长请求，以及显式StopMontagesWithSlot的IsActive资格；验证实际Transition与Turn栈及槽权重消费者。Turn只作为已证明的同组消费者，不展开其方向/延迟/资产选择算法；队列与动画通知的跨阶段调用顺序另项验收。Gate：fresh组映射/设置/直接源调用、原生停止轨迹、actual actor入口与恢复/控制、消融及身份记录。开始：2026-10-15 19:44 +08:00；预估结束：2026-10-15 20:34 +08:00；实际结束：2026-10-15 20:49:38 +08:00；偏差：+15分38秒；结果：记录Gate PASS，新增D171/P2，未修复。估时依据：fresh Montage_PlayInternal/StopAllMontagesByGroupName约100行、ALS槽停止约40行，target两栈及请求桥约160行；.83单实例40分20秒、.84合成62分钟，本项复用停止取证接口，将通知调度另拆后按50分钟验收。
  - 19:44 活动摘要：.84已提交`8edba94c`，累计168项（100P2/68P3）。已确认组替换会调用同组所有有效Montage的Stop，而ALS显式槽停止只接受IsActive；两者不能共用“已混出就忽略”的假设。当前只沿这两条调用边核对，生产未改，截图0、性能暂停。
  - 20:24 source/consumer闭环：fresh Skeleton确认Transition及两种Turn均属Grounded；源Turn混出模式为Inertialization，但组替换使用新Montage的Standard混入设置。两种原生Turn共572帧与独立公式逐值相同。实际图启动旧Turn后，两次受控Dynamic请求经真实dispatcher进入同组替换；第二次把已进行0.15秒的0.2秒淡出重新启动，后续三帧保留0.13183594/0.078125/0.024414063权重，source均为0。站蹲均复现，只恢复旧进度即通过，确认D171/P2，累计169项（101P2/68P3）。单次替换控制通过；不将受控请求时点当作默认动画自然可达证明，继续资格/时长边界、工具回归和消融。
  - 20:49 完成摘要：2项focused均按已确认边界失败，76组比较68PASS/8FAIL；实际60帧中的6个失败及提取原方法的相等/更长两例均归D171，20帧恢复与Transition栈8个时长控制通过，显式槽停止资格8组通过。572原生帧精确复算，旧动态2517帧不变；3个参数消融、4个恢复控制及3个报告缺失/篡改反例通过。删去夹具两个无用状态变量并改为直接断言后，同target重编的76个结果不变。7份ALS/6份UE/8份target、116基线source、160生产Rust、累积资产、reader源码/package/DLL与入口库身份PASS；证据`85/summary.json`、`85/final/`。实耗65分38秒，偏差仍在允许窗口内；原生停止模式/资格拆分和实际请求路线核对超过初估，后续同类任务应把调用者取证与数值控制进一步分开。仅扩展取证与自测，生产/转换器未改。源惯性混出请求的图消费、队列/通知阶段及Turn其余初始化/参数路径继续保留；不以legacy epoch或权重清零代替惯性姿态Gate。

- [x] BF144.86 Turn惯性混出请求与生成图消费：依赖.85记录Gate，沿显式StopMontagesWithSlot和Turn自然结束发出的惯性化请求，核对Montage组请求的保存/转发、实际Slot及Inertialization节点位置、target请求桥与最终姿态消费。只闭合已证实的停止→惯性化边，不扩展Turn选择/通知调度或其他状态机惯性切换。Gate：fresh根图/站蹲图与源请求合同、请求和姿态的实际消费者、恢复/负控制、消融及身份记录。开始：2026-10-15 20:57 +08:00；预估结束：2026-10-15 21:47 +08:00；实际结束：2026-10-15 21:16:30 +08:00；偏差：−30分30秒；结果：记录Gate PASS，新增D172/P2，未修复。估时依据：.85停止入口65分38秒，其中已闭合Montage停止模式；本项复用该取证，新增组请求到Slot约100行及生成图/姿态路径，以50分钟独立验收，惯性化求解器其余边界另拆。
  - 20:57 活动摘要：.85已提交`1098326c`，累计169项（101P2/68P3）。legacy epoch增加只证明请求产生，不证明生成图已消费；本项同时核对原生图位置和实际姿态。生产未改，截图0、性能暂停。
  - 21:15 source/consumer闭环：4个fresh原生资产确认两种Turn Slot只在Idle，接收惯性化的父节点位于整个站蹲状态机之后。源只由当帧更新的Slot转发Grounded组请求并逐帧清空；目标绑定为父节点直接消费全局epoch。56帧Idle显式/自然停止及请求移除控制通过；移动时Slot未更新、FSM无请求，受控停止仍改变姿态，站蹲各连续3帧失败，仅屏蔽epoch后14帧恢复，确认D172/P2。首次移动夹具只切状态而未推进动画，信号无效；改用实际移动输入后复现。随后删除冗余移动字段覆盖及无用suppression状态，同target重编的84帧判定不变；正在完成身份/记录Gate，不声明默认动作自然可达。
  - 21:16 完成摘要：2项focused为1PASS/1预期FAIL，84帧78PASS/6FAIL归D172；56帧正常停止/移除控制和14帧非活动Slot恢复全通过，88次当前RotationYawSpeed过滤断言通过。3份ALS/6份UE/14份target、116基线source、160生产Rust、累积资产、读取器源码/package/DLL及入口库身份PASS；证据`86/summary.json`、`86/final/`。实耗19分30秒；本轮只改审计记录，无生产、转换器、读取器或资产变更。累计170项（102P2/68P3）；默认动作可达性、完整惯性求解器数值、队列/通知阶段和Turn其余选择路径继续保留，截图0、性能暂停。

- [x] BF144.87 Transition/Turn队列仲裁与停止参数：依赖.86记录Gate，核对当前原生通知/回调的队列写入、同帧停止与播放优先级、被停止阻挡后的队列保留、多个请求覆盖及停止时长到实际栈的消费。以线程安全队列及当前实际game-thread调用分支为边界，通知采样/跨帧派发时序和Turn选择算法另项验收。Gate：fresh通知参数与源队列合同、最小事件序列和actual dispatcher/栈/槽消费者、恢复/负控制、消融及身份记录。开始：2026-10-15 21:19 +08:00；预估结束：2026-10-15 22:09 +08:00；实际结束：2026-10-15 21:46:36 +08:00；偏差：−22分24秒；结果：记录Gate PASS，新增D173/P2，未修复。估时依据：fresh ALS NativePostUpdate和三种队列flush约110行、target dispatcher相应分支约180行，.81队列选择实耗19分41秒、.85组停止65分38秒；本项复用已验证停止核并限定仲裁/参数，按50分钟独立验收。
  - 21:19 活动摘要：.86已提交`69f017ba`，累计170项（102P2/68P3）。源Stop默认参数为−1，表示沿用旧Montage混出时长，不能把缺少显式0.2字段直接认定为差异。先提取当前通知实参和实际调用顺序；生产未改，截图0、性能暂停。
  - 21:40 source/consumer闭环：60帧实际dispatcher队列控制通过，涵盖重复停止、覆盖待播放请求、解除阻挡后消费一次及Transition→Turn顺序；当前两种stance停止包装的原生参数均为−1。沿LandToGrounded直接调用边补取Locomotion原生图后，确认该事件为0.1/0.2混入混出、1.4倍速、起点0，目标却复用Roll的0.2/0.2、1.5倍速、起点0.2且固定站姿。24帧单事件对照为12原样FAIL/12源参数恢复PASS，实际采样和旧动画混出均受影响；同帧多事件的首个猜测被该参数错误混淆，不另登记“调用合并”缺陷。首次自然启动夹具未保证可见图经历落地，现改为稳定角色后实际跳跃，继续核对自然触发与最终Gate。
  - 21:46 完成摘要：3项focused为1PASS/2预期FAIL，85组比较72PASS/13FAIL；60帧队列控制、12帧参数/stance恢复及20次根Slot实际采样核对通过，实际角色跳跃落地另行复现D173参数错误。删除两条含混的多时长原生轨迹后，保留的20帧与原记录逐值相同，同target重编的85项判定不变。5份ALS/3份UE/14份target、7资产及116基线source、160生产Rust、累积资产、读取器和入口库身份PASS；证据`87/summary.json`、`87/final/`。实耗27分36秒，累计171项（103P2/68P3）；仅改审计记录，没有生产、转换器、读取器或资产修改。通知采样/派发时序、Turn其余选择、显式过渡/QuickStop和完整惯性求解器继续保留，截图0、性能暂停。

- [x] BF144.88 显式Transition与Quick Stop选择/参数：依赖.87记录Gate，核对PlayTransition左右/stance/StandingIdleOnly资格、当前状态函数的实参，以及Quick Stop的输入/目标yaw来源、角度规整、左右选择和速率插值，闭合到实际dispatcher、Sequence和槽采样。既有Land参数D173隔离，通知采样/派发时序与Turn资格另项验收；不展开道具Overlay表现。Gate：fresh源函数/角度工具/设置/直接通知调用、独立边界和实际消费者、恢复控制、消融及身份记录。开始：2026-10-15 22:10 +08:00；预估结束：2026-10-15 22:55 +08:00；实际结束：2026-10-15 22:47:46 +08:00；偏差：−7分14秒；结果：记录Gate PASS，新增D174/P3，未修复。估时依据：fresh QuickStop及显式Transition选择约85行，target选择和构造/通知映射约150行；.81选择闭包19分41秒、.87队列参数27分36秒，本项复用消费者入口，预留角度边界取证后按45分钟独立验收。
  - 22:10 活动摘要：.87已提交`d86587fb`，累计171项（103P2/68P3），工作区干净。只沿当前运动停止/过渡调用读取源实参和yaw语义，不把旧构造器自测当作参数已等价。截图0、性能暂停，生产未改。
  - 22:45 source/consumer闭环：fresh设置、4个直接状态函数和6个Sequence确认当前参数/资源类型；45个共同纯yaw旋转由UE直接输出double Rotator。405组直接规则通过，但3726组有效角度输入链有70次左右分叉；真实dispatcher/root Slot的64组中12次选错Side，28个只恢复源index/rate的控制全通过，邻域和非VelocityDirection控制通过，确认D174/P3。显式过渡左右/stance/StandingIdleOnly与当前命名调用21组通过。已删除1944组超出当前源yaw范围的额外输入，保留判定及64组消费者逐值不变；显式夹具缺少空播放栈的前置错误已修正，未计为产品差异。当前数值边界均为受控输入，不声明默认动作自然可达。
  - 22:47 完成摘要：4项focused为2PASS/2预期FAIL，4216组4134PASS/82FAIL（70输入链、12实际选择）归D174；45个原生旋转及21个当前显式调用控制通过。5份ALS/3份UE/11份target、10资产及116基线source、160生产Rust、累积资产、读取器和入口库身份PASS；证据`88/summary.json`、`88/final/`。实耗37分46秒，累计172项（103P2/69P3）；仅更新审计记录。Turn资格/选择、通知时序、任意外部过渡参数及已知共享时钟/末端边界、完整惯性求解器仍保留；截图0、性能暂停。

- [x] BF144.89 Turn In Place资格、延迟与选择：依赖.88记录Gate，核对IsTurnInPlaceAllowed、逐帧刷新守卫、许可/视角/视角速度门槛、ActivationDelay生命周期、90°/180°和左右/stance选择，以及八项设置的Sequence/PlayRate/曲线速率缩放到实际Turn Slot。已验证的组替换/惯性请求与Quick Stop边界隔离，通知时间线和完整惯性求解器另项验收。Gate：fresh C++/设置/直接站蹲调用、独立状态序列及边界、实际生成图→队列→Slot消费、消融和身份记录。开始：2026-10-15 22:51 +08:00；预估结束：2026-10-15 23:36 +08:00；实际结束：2026-10-16 00:17:00 +08:00；初轮偏差：+41分00秒，已按下述复盘重估；结果：记录Gate PASS，未新增差异。估时依据：fresh Turn资格/刷新/播放闭包约150行，target detector/回调/请求播放约160行；.81状态选择19分41秒、.88边界及消费37分46秒，复用当前原生读取和实际角色入口，按45分钟独立验收。
  - 22:51 活动摘要：.88已提交`c52d00e3`，累计172项（103P2/69P3），工作区干净。源ActivationDelay按当前设置从0映射到0.75秒，且只在严格超过映射值后触发；先核对实际规则，不按直觉改变延迟方向。截图0、性能暂停，生产未改。
  - 23:57 超时复盘：本轮开始22:51、预估结束23:36、检查时23:57:32，已耗66分32秒，尚未通过记录Gate，偏差+21分32秒。4560组检测边界、96帧一次/三次刷新生命周期及站蹲资源/曲线速率已有通过证据；原生蓝图加载曾异常退出，临时日志和禁用编译实验均无效且已恢复源码，改为取崩溃报告后获得完整站蹲原生记录，恢复原命令也已成功。主要漂移来自读取器诊断及资格夹具：晚改组件造成Tag不一致，修正后仍见许可true但Idle刷新帧停留；此前反复调整注入顺序而没有先观测活动FSM，延长了定位。不能把“许可”直接等同于“当前状态正在执行该回调”。下一轮先核对真实FSM状态与源进入/退出条件，再修正验证前置、消融及收口；不据此新增缺陷，也不终止任务。新一轮开始：2026-10-15 23:57:32 +08:00；预估结束：2026-10-16 01:05:02 +08:00（按上一窗口的1.5倍推导，剩余工作限于上述未闭合边界与记录）。
  - 00:08 source/consumer闭环：fresh原生记录确认迟迟未调用Idle是因夹具进入了Rotate Right，源也使用动画结束的自动规则返回Idle；过渡混出期间仍有Idle回调，因此必须以“当前活动状态为Idle且本帧刷新”为前置。修正后4560组规则、96帧生命周期、28个实际选择/速率/RotationYawSpeed缩放及12个模式/视角资格均通过，未新增差异。两种stance的初始化、刷新、曲线修改、Slot共8个关键原生节点与旧记录逐值一致；设置恢复重读也逐值相同。工具诊断改动已按字节恢复并重建，原路径Standing读取成功；加载异常根因未宣称修复。正在删除夹具无用状态和诊断分支后完成最终记录Gate。
  - 00:17 完成摘要：3项focused、4696组全部PASS；合并夹具冗余allowed/enabled状态并删去旧诊断后，同target重编结果不变。5份ALS/6份UE/12份target、3资产及116基线source、160生产Rust、累积资产、恢复后的读取器源码/package/DLL与入口库身份PASS；证据`89/summary.json`、`89/final/`。本轮总实耗86分钟；重估轮开始23:57:32、预估01:05:02、实际00:17:00，偏差−48分02秒，结果PASS。后续同类取证应先观测真实活动状态，再区分许可、回调发生和状态已完成，避免重复调整无关注入顺序。仅更新审计记录，生产/转换器未改；读取器源码已恢复，二进制因诊断重建，5次失败报告与有效原生输出均保留。累计172项（103P2/69P3）；剩余Turn时钟/完整混合、通知时序、完整惯性求解器和核心图继续审计，截图0、性能暂停。

- [x] BF144.90 Turn Montage时钟、混入与自然结束：依赖.89记录Gate，沿八个当前转向资源核对原生动态Montage时长/RateScale、逐帧位置/混入权重、末端停止与惯性请求时点，闭合到实际Turn Slot及姿态消费者。已有组替换、选择、惯性请求路由保持隔离；多实例非加性混合、通知事件时间线另项验收。Gate：fresh资源与UE Advance/Stop合同、原生共同轨迹、实际时钟/权重/停止与恢复控制、消融及身份记录。开始：2026-10-16 00:26 +08:00；预估结束：2026-10-16 01:16 +08:00；实际结束：2026-10-16 01:10:47 +08:00；偏差：−5分13秒；结果：记录Gate PASS，新增D175/P2、D176/P3，未修复。估时依据：当前共享Advance/Stop闭包在.82/.85已取证，新增8资源和Turn自身累加/停止实现约90行；.82时钟41分15秒、.88数值与消费37分46秒，按50分钟拆分单实例时钟与后续多实例混合。
  - 00:26 活动摘要：.89已提交`8e506ae5`，累计172项（103P2/69P3），工作区干净。当前Turn使用累加时钟，与D118所指的反算时钟不同，不能直接归并；先读取原生停止帧，再判断是否有实际姿态影响。截图0、性能暂停，生产未改。
  - 01:08 source/consumer闭环：8资源56条原生轨迹11520帧的位置/权重/有效性/停止状态与独立float公式逐值一致；目标56条中26条不符，分为首次零delta删除实例及六个2秒资源遗漏末端停止窗口。真实角色中，零倍率首帧后恢复时间仍丢失Turn，64帧保留实例控制全部恢复，姿态/RotationYawSpeed也实际受影响，确认D175/P2。采用真实1/60世界步长、owner倍率1后，六资源均晚一帧发惯性请求；826组停止前姿态历史一致，恢复源停止帧消除约1mm/0.025°的后续差异，另两资源控制通过，确认D176/P3。删去无用Rust时长字段后全部判定不变；两项数值问题只登记，未修生产。Crouch180的元数据精度仍属D167同一格式化边界，本次未产生独立新增反例。
  - 01:10 完成摘要：3项focused按已确认差异失败；496组轨迹/帧/姿态比较346PASS/150FAIL，失败归并D175/D176，64帧首帧保留恢复及128帧末端恢复全部通过。11520原生帧精确复算，实际owner/世界delta与原生输入逐值核验；自然结束在真实1/60步长的复验与原结果完全相同。2份ALS/6份UE/7份target、9资产及116基线source、160生产Rust、累积资产、读取器与入口库身份PASS；证据`90/summary.json`、`90/final/`。实耗44分47秒，累计174项（104P2/70P3）；仅改审计记录，未改生产、转换器或读取器。多实例Turn非加性混合、通知时序、完整惯性求解器和核心图继续保留，截图0、性能暂停。

- [x] BF144.91 Turn Slot非加性姿态与曲线混合：依赖.90记录Gate，沿当前Turn Slot核对基础姿态剩余权重、单/多实例合成、相关性/总权重归一化及非加性旋转/位置/scale/曲线运算，闭合到实际站蹲槽输出。使用共同已采样姿态与权重隔离已登记时钟/停止问题；不扩展其他Slot或通知派发。Gate：fresh Slot/AnimationRuntime与target合成合同、必要原生混合取证、常规和边界实际消费者、消融及身份记录。开始：2026-10-16 01:16 +08:00；预估结束：2026-10-16 02:06 +08:00；实际结束：2026-10-16 02:04:27 +08:00；偏差：−1分33秒；结果：记录Gate PASS，新增D177/P2、D178/P3、D179/P3，未修复。估时依据：fresh SlotEvaluatePose非加性分支约100行、BlendPosesTogether及target预混/Slot约150行；.84加性混合62分钟、.90时钟44分47秒，本项复用采样和消费者入口，仅新增非加性合成，以50分钟独立验收。
  - 01:16 活动摘要：.90已提交`e8a898f6`，累计174项（104P2/70P3），工作区干净。源把非加性Montage和基础姿态交给统一混合；目标先合成Turn列表再进入Slot。先核对实际插值运算和输入权重，不由“两阶段”本身推断不等价。截图0、性能暂停，生产未改。
  - 01:36 source修正检查点：fresh AnimInstance.cpp:4648–4669证明MontageEvaluationData在Slot之前按weight>1e−5过滤，.84关于保留无关小权重参与总和的解释不成立。症状为.91双小权重源公式会输出微量姿态，而真实上游根本不提交这两个实例；同一闭包也覆盖.84。已有限复开.84，暂停.91新的消费者测试，先修正取证夹具并复核旧判定；不修生产、不扩大到其他图。复核开始：2026-10-16 01:36 +08:00；预估结束：2026-10-16 01:51 +08:00，依据旧236组可直接重跑与单一上游资格修正。.84于01:40:44完成并以`beb60a3b`单独提交后继续本项。
  - 01:54 source/consumer闭环：11资产及2个实际站蹲Slot重新读取；64个UE原生BlendPosesTogetherIndirect样本与独立公式最大差2.22e−16。当前目标预混明确使用slerp，权重0.3/0.7的20组实际槽中14组旋转不符，仅改nlerp全部恢复，新增D177/P2；近满权重0.999995的16组误升为全权重，源姿态/曲线权重恢复全部通过，新增D178/P3。原始权重均相关的三实例在归一化后又触发独立曲线门槛：小权重置于中项20组均失败、置于首项20组全过，新增D179/P3，不能由D168上游过滤修复。仅改nlerp仍在含基础姿态的40组中留下28个失败，归并已有D123；原始小权重及微超额总权重分别补充D168/D169，不重复计数。累计177项（105P2/72P3），生产未改。
  - 02:04 完成摘要：3项focused为1PASS/2预期语义FAIL；1032组比较767PASS/265FAIL，含400原始槽、400完整源恢复、80部分nlerp控制、88局部混合及64原生对照。400个完整恢复及152个常规原始控制全过。沿本次原生Slot直连ModifyCurve再核对112个输出，D178/D179均传到实际RotationYawSpeed缩放结果，56个源恢复全过；倍率使用共同正输入1.2，选择/倍率推导仍由.89验收，不冒称本项覆盖自然触发。初版夹具强取满权重时不存在的基础缓存、手工栈未提供播放倍率均已按源生命周期纠正，不计产品缺陷。删去无用import和280个无诊断作用的部分控制后，同target重编的保留判定、全部原始/完整恢复结果不变。3份ALS/12份UE/9份target、2份依赖、11资产及116基线source、160生产Rust、累积资产和入口库身份PASS；读取器源码按字节恢复并在同target重建，旧50个原生加性样本逐值一致。证据`91/summary.json`、`91/final/`；实耗48分27秒（含.84有限复核4分44秒），仅提交审计记录，生产/转换器/读取器源码未改。剩余通知时序、外部参数边界、完整惯性求解器与核心图继续保留，截图0、性能暂停。

- [x] BF144.92 运动点通知采样、资格与GroundedEntryMode消费：依赖.91记录Gate，以当前运动点通知SetGroundedEntryMode为入口，沿A_Als_Roll→Montage片段/通知队列→ALS通知→模式写入，核对原始触发时刻、前后端点、零步长、资格权重/LOD与中断条件；以共同播放时钟隔离已登记动作/时钟差异。Footstep/CameraShake效果保持排除；持续状态通知Begin/Tick/End及图更新前后的派发顺序另项验收，不复开Roll自身运动算法。Gate：fresh序列/蒙太奇/图与Notify采样/队列合同、局部边界和实际模式消费者、恢复/消融及身份记录。开始：2026-10-16 02:13 +08:00；预估结束：2026-10-16 02:58 +08:00；实际结束：2026-10-16 02:33:51 +08:00；偏差：−24分09秒；结果：记录Gate PASS，补充D121，未新增差异。估时依据：当前manifest索引中的非表现点通知仅此一个，source GetAnimNotifies及队列过滤约190行，target点事件和实际动作播放采样约200行；.88实际选择边界37分46秒、.91多实例混合48分27秒，本项把持续状态与调度先拆出，以45分钟独立验收。
  - 02:13 活动摘要：.91已提交`92bedd09`，累计177项（105P2/72P3），工作区干净。当前manifest索引只用于确定入口，源参数重新通过原生读取器取得；fresh ALS Notify直接调用SetGroundedEntryMode，UE点通知前端排除、后端包含，队列另判权重/LOD。先逐项核对，已登记D121精度和既有Roll时钟问题只作已知边界，不重复计数；截图0、性能暂停。
  - 02:29 source/consumer闭环：fresh Roll序列/蒙太奇、根图和Grounded图四份原生读取通过，点通知GetTriggerTime精确为0.9333333373069763，manifest的f32为0.9333329796791077。646个局部端点/权重/leader及原生非循环采样对照通过；208个实际派发/过滤组中12个原始时间失败，100个时间恢复及8个过滤对照全部通过，600次真实T2条件查询一致传递差异。实际Roll输入启动后、保持满权重PostLocomotion槽、只控制共同播放区间的12组又复现3个提前通知，6个时间恢复全过；补充D121，不新增编号。初版角色不可见导致后续图更新被停用，已在夹具设置实际mesh可见并重编；这属于测试前提错误。接下来完成常规自然播放控制、夹具消融和身份Gate；槽当前/前帧相关性及跨阶段调度保留下一项。
  - 02:33 完成摘要：3项focused为1PASS/2预期语义FAIL，870组比较856PASS/14FAIL，全部失败归D121；105个实际时间恢复组和6个自然播放组全部通过，另完成600次真实T2条件查询。移除未使用的enabled字段/分支与Clone，以及两组映射为相同实际时刻的重复producer样本后，同target重编的全部保留输出逐字相同。4份ALS/9份UE/11份target、4资产及116基线source、160生产Rust、累积资产、读取器源码/package/DLL和实际入口库身份核验PASS；证据`92/summary.json`、`92/final/`。记录脚本首次受Windows默认GBK解码影响，改为明确UTF-8后通过；没有生产、转换器、读取器或原始资产修改。实耗20分51秒，低于预估；复用现有原生时钟和消费入口缩短了本批取证。累计177项（105P2/72P3），继续持续状态通知和调度，截图0、性能暂停。

- [x] BF144.93 Roll排队状态通知与RootMotionScale生命周期：依赖.92记录Gate，沿当前SetRootMotionScale核对区间重叠、Begin/Tick/End、零步长、资格消失/中断/实例清理和scale写入/恢复，闭合到实际Roll根运动消费者。即时branching的SetLocomotionAction/EarlyBlendOut及槽相关性历史另项验收；已完成的Roll根运动算法不因猜测复开。Gate：fresh ALS状态通知、UE队列生命周期和原生资产、局部轨迹与实际消费者、必要恢复/消融及身份记录。开始：2026-10-16 02:36 +08:00；预估结束：2026-10-16 03:16 +08:00；实际结束：2026-10-16 02:57:56 +08:00；偏差：−18分04秒；结果：记录Gate PASS，确认D180–D182，语义仍不等价。估时依据：当前运动排队状态只有Roll的TranslationScale=1.3，UE采样/TriggerAnimNotifies直接闭包约240行，target状态生命周期及缩放消费者约200行；.92点通知20分51秒、.15根运动41分11秒，复用实际角色与原生采样工具，以40分钟独立验收。
  - 02:36 活动摘要：.92已提交`80557ff5`，累计177项（105P2/72P3），工作区干净。fresh源将SetLocomotionAction/EarlyBlendOut标为native branching，SetRootMotionScale使用排队的NotifyBegin/NotifyEnd；本批只审后者。源排队状态依赖本帧区间重叠，End在该状态不再排入队列时调用；target从采样队列与独立lifecycle恢复状态。先核对共同区间和尺度恢复条件，未登记新差异；截图0、性能暂停。
  - 02:47 source/consumer闭环：两份原生Roll资产确认有效RootScale区间为0.0001→1.5001000166，manifest与源f32相同；566个局部资格/生命周期对照通过。但实际GraphClip仍是0→1.5，四个边界帧违反有效区间；沿其唯一编译入口与当前轨道补查，未修改规则。实际中断后状态未End，三个owner倍率的自然Roll/Ragdoll共同队列比较有42个失败帧，最终删除时才恢复倍率；同一通知跨实例接替还额外End/Begin。下一步分别恢复轨道、队列清理及源通知身份并验证实际根运动消费；尚未把Ragdoll期间的非零采样量当作实际位移。
  - 02:57 完成摘要：5项focused为2PASS/3预期语义FAIL，1290组比较1225PASS/65FAIL，分别归D180/P3、D181/P3、D182/P2；360个实际恢复组全部通过。真实允许的Roll重入确认倍率2被重复Begin改为1.3并进入根运动驱动，默认1.3仅多回调、位移保持一致；四个实际player移除/替换与倍率保留控制在下一派发点均正确清理。删去未参与当前范围的原生循环标志/分支及冗余轨道复写分支后，同target重编的全部比较输出逐字一致。3份ALS/12份UE/8份target、两资产、一依赖及116基线source、160生产Rust、累积资产、读取器源码/package/DLL与入口库身份PASS；证据`93/summary.json`、`93/final/`。实耗21分56秒，低于预估；复用已有原生采样和目标入口避免新增工具分支。累计180项（106P2/74P3），未改生产、转换器或原始资产；native branching及跨阶段派发继续保留，截图0、性能暂停。

- [x] BF144.94 即时动作状态通知与跨实例动作保护：依赖.93记录Gate，沿九个当前Roll/GetUp/Mantle montage的SetLocomotionAction核对native branching起止、跨越/零步长、权重与LOD绕过、中断/终止、实例身份及外部动作保护，闭合到实际动作组件和当前消费。EarlyBlendOut的条件Tick/停止请求、槽相关性与整体派发阶段另项验收；既有D121精度和动作时钟差异以共同输入隔离。Gate：fresh原生montage及ALS/UE branching合同、局部轨迹与实际动作消费者、恢复/消融及身份记录。开始：2026-10-16 03:01:41 +08:00；预估结束：2026-10-16 03:46:41 +08:00；实际结束：2026-10-16 03:25:43 +08:00；偏差：−20分58秒；结果：记录Gate PASS，新增D183/D184并扩展D121，语义仍不等价。估时依据：manifest索引有九个动作montage、同一native类三种动作标签，UE UpdateActiveStateBranchingPoints/EventHandler及Terminate约180行、target生命周期/派发约220行；.92点通知20分51秒、.93排队状态21分56秒，额外保留九资产fresh提取和跨实例保护验证窗口，以45分钟独立验收。
  - 03:01 活动摘要：.93已提交`53d637db`，累计180项（106P2/74P3），工作区干净。fresh ALS通知Begin设置Character动作，End只在当前动作仍相等时清空；其native branching语义必须逐条核对，不能沿用上一项排队状态的中断清理结论。九资产索引只用于确定入口，原生有效时刻重新读取；截图0、性能暂停。
  - 03:15 source摘要：九资产fresh原生提取全部通过；SetLocomotionAction由native构造器启用branching，即使资产tick字段为Queued也不走排队状态生命周期。源中断后跳过HandleEvents/Tick但在Terminate结束活跃状态；正常推进先按起止marker拆步，执行区间校正后再发marker，最终仅Tick仍活跃状态。target直接按最终时间计算状态并在Roll旧实例清理时抑制同名新实例的动作结束；正在实际角色中逐项验证。GetUp/Mantle八个end仍有已知六位舍入，Roll端点精确；标记算法比较先统一精确端点隔离D121。4份ALS/6份UE/7份target已冻结；不扩展EarlyBlendOut条件或槽派发阶段。
  - 03:25 完成摘要：5项focused为1PASS/4预期语义FAIL，3176组比较2786PASS/390FAIL；345个局部与28个实际帧归D183，1个跨实例帧归D184，16个精度帧归D121，新增均P3。1125个局部和308个实际恢复全部通过；6组正常/中断自然Roll共310帧一致。删除未使用的Input端点状态和Asset复制派生后，同target重编的全部比较输出逐字一致；精度夹具进一步使用零步长位置校正隔离marker算法，输出保持相同。4份ALS/6份UE/7份target、9资产及116基线source、160生产Rust、累积资产、读取器源码/package/DLL和入口库身份PASS；`94/summary.json`、`94/final/`。实耗24分02秒，低于预估；初始High精度夹具误要求两相邻f32之间仍有可表示值，已纠正且不计产品失败。累计182项（106P2/76P3）；未改生产、转换器或原始资产，未重复公共Playground Gate，截图0、性能暂停。下一项只接EarlyBlendOut条件Tick和停止消费者。

- [x] BF144.95 EarlyBlendOut条件Tick与当前Montage停止：依赖.94记录Gate，沿六个当前Mantle montage的native EarlyBlendOut核对输入/运动模式/旋转模式/姿态条件及默认字段、仅Tick语义、实例选择、BlendOutDuration/模式/曲线/重入停止和实际montage消费者。已知D121端点精度、D183标记边界以共同输入隔离；不展开整个槽相关性和跨阶段派发。Gate：fresh六资产与ALS/UE停止合同、条件矩阵及实际消费者、必要恢复/消融和身份记录。开始：2026-10-16 03:28 +08:00；预估结束：2026-10-16 04:08 +08:00；实际结束：2026-10-16 03:51:33 +08:00；偏差：−16分27秒；结果：记录Gate PASS，新增D185/P3，语义仍不等价。估时依据：当前manifest仅六个Mantle资产包含此状态，ALS Tick约40行、UE Stop约100行，target条件函数与实际停止消费者约100行；.93/.94通知批次实耗21分56秒/24分02秒，额外留六资产提取和混出请求验证窗口，以40分钟独立验收。
  - 03:47 source/focused摘要：六资产fresh参数及72组原生通知时钟样本已读取，四项条件均默认开启；High为Cubic，五个Low为HermiteCubic，全部Standard且无Profile/CustomCurve，目标配置一致。366个实际条件/阶段/实例控制与90条1260帧原生Stop权重/清理对照通过。源InputBlocked先把CMC输入清零，Character再由当前加速度生成bHasInput；target EarlyBlendOut直接读取未阻止的命令。96个共同状态控制24FAIL、8个真实攀爬尾段控制2FAIL，52个输入恢复全过，确认D185/P3。初始真实夹具误把较晚刷新的ActualMotion快照当作本阶段当前输入，已改由当帧受阻止的controller basis核验，不计为新差异；当前仅做消融和身份核对。
  - 03:51 完成摘要：4项focused为2PASS/2预期语义FAIL，1730组比较1704PASS/26FAIL，全部失败归D185；52个实际输入恢复全部通过。删除未使用的资产端点、复制状态的id/pose_ready及派生、自然夹具的冗余快照读取，并去掉与源默认相同的显式参数写入后，同target重编的全部比较输出逐字一致。6份ALS/6份UE/11份target、6资产及116基线source、160生产Rust、累积资产、reader源码/package/DLL和入口库身份PASS；证据`95/summary.json`、`95/final/`。实耗23分33秒，低于预估；复用原生Stop采样与现有角色入口，无需增加读取器分支。累计183项（106P2/77P3）；仅记录，未改生产/转换器/原始资产，未重复公共Playground Gate，截图0、性能暂停。下一项为槽相关性历史与排队通知派发。

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

BF144-D08 已撤销（2026-10-13用户决定）：作为Bevy移植项目，重力加速度统一使用Bevy的值；不把源9.80与当前Bevy/Avian默认9.81的区别视为缺陷，不据此修改运行时。原取证保留在`target/bf144/03/gravity/`，仅作本次决策依据；D08从有效缺陷和失败反例集合移除。

- [ ] BF144-D09 / P2 — 地面射线回退丢失源贴地高度保护。
  - Source：fresh ALS `ComputeFloorDist:781–796` 保留sweep FloorDist/穿透数据并另存LineDist；UE `AdjustFloorHeight:6305–6319` 在line fallback、FloorDist<MIN且LineDist≥MIN时立即返回，避免错误沿墙调整高度，后续PhysWalking仍独立做侧向脱离穿透。
  - Target：`src/als/character/movement/motion.rs:3905–3910` 只保留line距离；`4011–4074` 与`adjust_walking_floor_height:3608`按这个单一距离调整高度，无法执行源guard。
  - 反例：完整加载角色和最小运动插件从中心高1m、侧墙面距中心0.1m进入普通物理帧；floor consumer前中心高0.9950988m，retry sweep仍穿透、FloorDist=−0.3m，中心射线LineDist≈0.0951m。source应保留这一阶段高度，target侧向脱离后降到0.9215m，多下移约7.36cm。`bf144_floor_line_fallback_preserves_adjust_height_guard`在8组中有2个语义FAIL；无侧墙以及0.9215m贴地位置的6个对照PASS。证据`target/bf144/04/line_final/`。
  - 漏检：旧`actual_walkable_line_floor_keeps_source_side_penetration_recovery`仅验证X方向脱离，初始Y已在源贴地带内，因此没有触发错误高度调整。当前反例读取真实consumer前的query，排除了将Avian较早阶段位移误归本guard的可能；仅登记，不改生产实现。

- [ ] BF144-D10 / P2 — 陡边的接触法线被误当成源几何面法线，建立错误地面支撑。
  - Source：fresh UE `IsWalkable:6987–7032`检查ImpactNormal；`CollisionConversions:499–540,445`、`PhysicsInterfaceUtilsCore:32–37`对正距离sweep取最反向几何面，`Chaos Convex.h:552–564`返回该面法线，不能直接使用胶囊接触Normal。凸体60°上表面的竖直法线分量0.5小于源WalkableFloorZ≈0.71，相邻侧面为竖直面，同样不可行走。
  - Target：`src/als/character/movement/motion.rs:3854–3862`仅在原始接触normal1已不可行走时尝试短射线重建；若凸体边缘的接触normal1较竖直，就直接交给`3880`判断walkable。实际floor consumer随后据此保持Grounded并调整高度。
  - 反例：60°凸体斜面以竖直侧面收边，边缘距角色中心0.1m、顶高0.08m。实际加载角色从中心高1.02m检查地面，consumer前sweep非穿透、距离0.1161m、接触normal1.y=0.9449，中心射线无命中，impact在默认0.15m有效perch半径内；source应无可行走floor并进入Air，target保持Grounded并拉到0.98437m。`bf144_floor_steep_edge_does_not_create_ground_support`语义FAIL，30°同形体对照PASS；独立query的0.1/0.14m边缘同样仅60°两组失败。证据`target/bf144/04/normal_loaded/`、`normal_query/`。
  - 漏检：已有坡度测试检查面内命中，边缘测试检查水平平台或原始normal已过陡的情况；没有覆盖“接触normal可行走、实际几何面不可行走”的边缘组合。仅登记，不新增补偿射线或修改通用碰撞库。
  - BF144.70补充地面预测消费者：fresh `RefreshGroundPrediction:1191`同样检查ImpactNormal；当前`locomotion_layer.rs:142`直接用normal1。30°/60°同一凸体边缘，接触normal.y均0.942273，源60°全部候选面均不可行走，目标仍给Fall预测权重0.377691；只恢复源资格便为0，活动Fall混合节点同步归零。四组3PASS/1FAIL，30°控制通过；`70/full_initial/`与`bf144_animation_phase70_prediction_face_normal`，归并同一面法线遗漏。

- [ ] BF144-D11 / P2 — 非均匀水平缩放改变了角色物理胶囊的源圆截面。
  - Source：fresh `CapsuleComponent.cpp:192–214`生成单一Sphyl；`BodySetup.cpp:2144–2181`、`ChaosInterfaceUtils.cpp:174–190`创建圆胶囊，半径取水平绝对尺度最大值并受半高约束；`BodyInstance.cpp:2132–2140,2377–2421`动态缩放保持同一规则。`PrimitiveComponent.cpp:3354`→`WorldCollision.cpp:545–560`的真实移动使用该物理形体，不是floor查询中取水平小轴的尺寸helper。
  - Target：`src/als/character/character.rs:197–199`直接建立Avian Capsule，自动非均匀缩放走当前依赖的`Capsule::scaled`凸体近似；`character/movement/motion.rs:3527`真实移动sweep直接消费这个形体，没有移植源圆截面的尺度规则。
  - 反例：完整角色世界scale=(2,1,1)，source物理半高0.9m、水平圆半径0.6m；target形体半extent=(0.6,0.9,0.285317)m。面对距中心1m的−Z墙，源形体sweep距离0.4m，target初始sweep为0.714679m，实际前进后停在z=−0.700251m，越过源允许位置约0.30m。直接缩放与父级缩放均失败；scale=1/2的4个uniform对照通过。`bf144_floor_capsule_scale_keeps_source_circular_horizontal_shape`语义FAIL，证据`target/bf144/04/scale/`。
  - 漏检：旧world-scale测试主要比较uniform与相同world-scale的父级路径，两边一致不能证明source形体一致；AABB高度和floor helper也不能验证实际碰撞圆截面。当前默认unit scale不受此分支影响。仅登记，不复开或改写已完成的uniform parent-space修复。

- [ ] BF144-D12 / P2 — 角色时间倍率为零时仍执行步行贴地高度调整。
  - Source：fresh UE `PerformMovement:2788–2808`在角色被外部移动后仅标记下一次检查floor；`3034`调用`StartNewPhysics`，后者`3531–3536`在owner delta<MIN_TICK_TIME时直接返回，ALS `PhysWalking:345–348`也有同一前置返回。因此当前Grounded角色在owner=0时不执行AdjustFloorHeight。
  - Target：`src/als/character/movement/motion.rs:3978`的floor同步只检查世界暂停，`4011–4074`继续按世界帧运行；CMC已正确发布simulation_delta_seconds=0也不能阻止此阶段移动角色。
  - 反例：完整加载的Grounded角色在静态平地被外部置于中心高1m、owner倍率0；物理后、floor consumer前仍为1m且CMC delta=0，consumer后却变成0.9215m。source该阶段应保持1m；`bf144_floor_frozen_owner_does_not_run_walking_height_adjustment`语义FAIL。owner=1及初始已贴地的3个对照PASS，证据`target/bf144/04/frozen/`。
  - 漏检：既有零倍率测试在稳定贴地高度或空中检查速度/位移，不包含Grounded且需要高度修正的输入；只保护CMC积分没有覆盖后置floor consumer。仅登记，不把世界暂停与owner倍率0混同，不修改源码。

- [ ] BF144-D13 / P2 — 坡面步行先作水平碰撞与跨步，丢失源切向移动顺序。
  - Source：fresh UE `ComputeGroundMovementDelta:5489–5512`由当前floor的ImpactNormal生成沿坡位移，native CMC的bMaintainHorizontalGroundVelocity=True，保持所请求的水平距离；`MoveAlongFloor:5560–5589`先sweep该切向位移，真正被阻挡后才尝试第二坡面或StepUp。
  - Target：`src/als/character/movement/motion.rs:3513–3552`直接对水平delta作sweep，碰到坡面就先向上抬升；低顶限制该抬升后，剩余位移再被slide投影缩短，实际可通过的坡道因此减速。
  - 反例：40°平面与平行顶面构成保持1.2cm头部余隙的通道，胶囊沿source切向0.25m试扫无阻挡；默认ViewDirection/Running起步，90帧source应水平前进5.304167m，target仅4.343863m。完整加载角色与最小运动插件均FAIL；20°坡以及无顶/10cm余隙的10个对照PASS。`bf144_walk_ramp_under_parallel_ceiling_keeps_source_tangent_motion`及证据`target/bf144/05/ramp_loaded/`。
  - 漏检：旧坡面与低顶分别验证，未组合陡但可行走的坡面和有正余隙的平行顶面；水平末位移相等的20°对照不能覆盖触发前向坡面阻挡后的路径。仅登记，不添加坡道补偿或改物理参数。

- [ ] BF144-D14 / P2 — 角色胶囊未携带源默认的禁止跨步权限。
  - Source：fresh UE `Character.cpp:81`及B_Als_Character的CollisionCylinder均为CanCharacterStepUpOn=ECB_No；`PrimitiveComponent.cpp:5287–5298`返回false，经CMC `CanStepUp:7525–7542`拒绝跨步。Playground实际7个角色实例也保留相同属性。
  - Target：`src/als/character/character.rs:361–425`的角色bundle没有移植该属性；`character/movement/motion.rs:3516–3518`在collider及body都无`AlsMantleTargetPermission`时默认true。
  - 反例：完整加载角色以50ms帧向uniform缩放0.2的被动角色前进；首次胶囊命中非穿透且normal.y≈0.0025，不属于可行走坡面。默认target把对方当可跨步prop，中心最高升至1.31152m并越过；source禁止跨步应停下。同几何普通prop允许跨过（PASS），仅给被动角色补入现有显式No权限的受控对照最高0.92562m并停在z≈−0.635m（PASS）。`bf144_walk_character_capsule_keeps_source_no_step_permission`语义FAIL，证据`target/bf144/05/permission/`。
  - 漏检：旧permission测试手动插入true/false，验证了consumer分支，却没有从fresh角色CDO核对生产角色默认组件；相同大小的站立胶囊又通常因步高限制掩盖这个缺口。仅登记，不修改角色构造或扩展AI行为。

- [ ] BF144-D15 / P2 — 跨步被拒后的沿墙剩余位移未sweep，多子步可直接进入第二面墙。
  - Source：fresh UE CMC `SlideAlongSurface:3649–3686`约束接触normal后调用基类；`MovementComponent.cpp:725–774`对slide执行SafeMoveUpdatedComponent，命中第二面再TwoWallAdjust并sweep，不能直接叠加未检查的投影。StepUp因No权限或上半球命中被拒后也必须走该路径。
  - Target：`src/als/character/movement/motion.rs:3535–3543`在拒绝跨步时直接返回剩余slide；`walking_movement_transaction:3636`把它加入Transform，下一子步已从墙内开始，后置物理解穿透不能恢复源顺序。
  - 反例：角色半径0.3m，初始(x,z)=(0.12,−0.155)m，两面墙使合法中心界限为x≤0.13、z≥−0.18m。0.1s帧分2子步后，物理前已到(0.17914,−0.28758)m；最小夹具最终z=−0.24835m，完整角色一次记录z=−0.20452m，均仍穿入第二墙。默认Yes权限但首墙只命中胶囊上半球的受控几何也复现，无须No权限。`bf144_walk_wall_slide_sweeps_the_remaining_substep`共24组20PASS/4FAIL，证据`target/bf144/05/wall_full/`。
  - 漏检：旧slide测试关注跨步内部已经sweep的剩余路径，或在单物理帧内让Avian处理第二墙；没有组合跨步早退、多CMC子步和第二阻挡面。记录的墙界限来自胶囊几何，未把“子步后的切向速度恒定”当作source oracle。仅登记，不补循环或改solver参数。

- [ ] BF144-D16 / P2 — 地面射线从实体内部出发时，误把出口底面当成可行走地面。
  - Source：fresh `ChaosCore/AABB.cpp:114–116`调用`Raycasts.cpp:12–86,175`，起点在box内部时LatestStartTime保持0；ALS `ComputeFloorDist:763–769`只接受Time>0的line hit，保留原sweep穿透供PhysWalking后续恢复，不把实体出口设成walkable floor。
  - Target：`src/als/character/movement/motion.rs:3899–3909`使用`solid=false`的ray cast，内部起点会得到正距离出口，当前返回normal又朝上，因此通过floor过滤并进入height adjustment/base支持路径。
  - 反例：完整角色被外部置入厚1m的平地内部；consumer前中心y=−0.20159m，retry sweep仍穿透。相同目标几何上的solid=true对照返回距离0，符合fresh源公式；生产non-solid ray距离0.79841m、normal.y=1，返回impact_y=−1m的假floor。`bf144_walk_inside_floor_ray_does_not_accept_the_exit_surface`语义FAIL；正常贴地与移除实体的两个对照PASS。证据`target/bf144/05/inside_ray/`。
  - 漏检：旧line-floor测试的角色中心均在实体外，只覆盖胶囊侧部穿透；未比较源零时间命中与target non-solid出口语义。本反例证明错误floor有效性，未把双方第一帧仍为Grounded或未经UE执行的最终恢复位置当作另一缺陷。仅登记，不修改射线或恢复逻辑。

- [ ] BF144-D17 / P2 — 空中撞陡坡后未限制朝坡内的输入，错误抵消部分下落速度。
  - Source：fresh UE `PhysFalling:5081–5122`先从OldVelocity重算VelocityNoAirControl，再由`LimitAirControl:5257–5286`去掉朝向向上斜面的水平法线输入，最后ComputeSlideVector；源`HandleSlopeBoosting:3749`仅限制向上偏转，不能替代这一步。
  - Target：`src/als/character/movement/motion.rs:2770–2778,2742–2746`把输入加速后的完整velocity直接交给`tnua_ext/motion.rs:3`做slide，没有源no-air-control与碰撞后输入限制阶段。
  - 反例：实际完整角色以−1m/s下落、1/60s帧，在60°光滑坡面发生普通非穿透接触。朝坡输入时源应得velocity约(−0.50381,−0.87263,0)m/s，目标实际为(−0.47885,−0.82923,0)m/s，少下落约0.0434m/s；CMC碰撞阶段缓存与最终velocity一致。`bf144_air_contact_limits_input_before_slope_deflection`共8组，完整/最小角色各1FAIL；无输入、沿坡、离坡的6个对照PASS。证据`target/bf144/06/air_control/`。
  - 漏检：旧falling_slide单元测试只核对给定delta的几何投影/向上限位，steep_contact测试只核对保持Air；没有比较接触后的输入限制与速度。仅登记，不改空中算法或坡度阈值。

- [ ] BF144-D18 / P2 — 空中胶囊sweep把局部旋转当成世界旋转。
  - Source：fresh UE `PhysFalling:4909–4910,5044`以UpdatedComponent的GetComponentQuat构造PawnRotation，并交给SafeMoveUpdatedComponent；实际移动查询使用component世界形体。
  - Target：`src/als/character/movement/motion.rs:3712–3722`复制local Transform，只把translation替换成世界Position；`falling_movement_transaction:2777–2778`把仍为local的rotation交给world spatial query。外层虽已计算world transform，却只使用其scale。
  - 反例：世界pitch=60°、uniform胶囊半高0.9m/半径0.3m，朝平墙空中运动。源世界形体前向半径0.81962m，相同目标shape的world-quat查询也验证此值。直接world rotation时中心停在z=0.83009m；挂在60°父级、local rotation=identity时，碰撞阶段走到z=0.31142m，完整角色后置solver一次结果仍只有z=0.53624m。`bf144_air_sweep_uses_world_capsule_rotation`共8组6PASS/2FAIL，证据`target/bf144/06/world_rotation/`。
  - 漏检：旧父空间碰撞回归主要覆盖平移/yaw或uniform尺度；圆截面胶囊对yaw不敏感，因此没有揭示局部quat误用。此反例保持相同世界形体且无非均匀缩放，区别于D11；仅登记，不修改父子变换代码。

- [ ] BF144-D19 / P2 — 漏掉空中两陡坡夹缝的ditch着地分支。
  - Source：fresh UE `PhysFalling:5198–5224`在两个向上、相互法线dot<0的坡面之间，经TwoWallAdjust后竖直delta近零时设置bDitch；随后的sweep不改变这个资格，直接以remainingTime=0调用ProcessLanded。`6373–6440,1445–1456`切到Walking并通知角色，0剩余时间不再运行PhysWalking。
  - Target：`src/als/character/movement/motion.rs:2719–2749`只迭代slide且仅接受普通walkable floor；两个不可行走坡面将velocity投影到近零，但没有ditch状态转换。
  - 反例：以−2m/s下落、50ms帧，完整角色位于两片60°光滑坡组成的V形槽上方；独立sweep确认先后命中两个坡面，normal约(±0.866,0.5,0)，沿交线竖直delta为0，满足源bDitch。target当帧velocity约0但仍Air，源应Grounded。`bf144_air_two_steep_slopes_apply_source_ditch_landing`共4组2PASS/2FAIL，证据`target/bf144/06/ditch/`。
  - 漏检：旧两墙测试只断言几何投影/限位结果，单陡坡测试只要求保持Air；没有对两坡夹缝的源特殊着地资格核对实际状态。仅记录已验证的当帧差异，不宣称源后续帧如何稳定或修正。

- [ ] BF144-D20 / P2 — 空中撞击移动物理刚体时丢失其法向速度。
  - Source：fresh UE `CharacterMovementComponent.cpp:290–293`默认UseTargetVelocityOnImpact=1；`PhysFalling:5127–5133`对simulated base计算`Velocity−ImpactNormal·dot(Velocity−ContactVelocity,ImpactNormal)`。fresh `Character.cpp:819–822,891–959`确认ContactVelocity包含物理线速度和接触点角速度贡献；本地源/项目配置未覆盖此CVar。
  - Target：`src/als/character/movement/motion.rs:2742–2746`统一从投影slide/remaining_time写回velocity，没有读取hit实体的动态body速度；`3982–3986`最终又用这个缓存覆写velocity，后置物理响应没有补回该合同。
  - 反例：完整角色以5m/s水平空中撞击竖直动态壁，壁速度分别+1/−1m/s。实际首次命中非穿透且normal≈−X，consumer前壁速度保持输入；源应保留角色法向速度+1/−1m/s，target最终均约0。`bf144_air_impact_velocity_uses_simulated_body_motion`共10组6PASS/4FAIL，静态/静止动态/运动学壁对照通过，证据`target/bf144/06/moving_body/`。
  - 漏检：旧Air碰撞夹具以静态几何为主，移动平台回归核对的是站立/落地后的支持速度，未覆盖空中首次impact的simulated-body分支。本项不扩展平台携带或场景物件行为，未改生产实现。

- [ ] BF144-D21 / P2 — 空中移动缺少源SafeMove的初始穿透恢复与重试。
  - Source：fresh `MovementComponent.cpp:547–584`默认MoveIgnoreFirstBlockingOverlap=0，SafeMove收到初始穿透时先GetPenetrationAdjustment/ResolvePenetration再重试原位移；`608–658`加入0.125cm pullback并以膨胀形体检查恢复位置。fresh CDO的MaxDepenetrationWithGeometry=500cm足以处理本例。
  - Target：`src/als/character/movement/motion.rs:2721–2749`空中仅cast并投影slide，未执行MTD恢复；现有`4031–4060`的类似恢复只在Grounded floor consumer中运行。后置Avian解穿透不能补齐source先恢复再移动的顺序。
  - 反例：角色空中被外部置为侧壁穿透0.2m，向内/向外速度各1m/s。源MTD恢复后的膨胀胶囊查询无阻挡，正常移动后中心必须x≤−0.2m；target CMC阶段仍x=0/−0.01667m，完整角色最终仅到−0.06607/−0.08044m，仍穿透约0.12–0.13m。`bf144_air_safe_move_resolves_initial_penetration_before_motion`共8组4PASS/4FAIL，墙外两方向对照通过；证据`target/bf144/06/penetration/`。
  - 漏检：旧空中碰撞回归从无重叠位置开始，穿透回归只核对Grounded floor路径；没有在Air首个SafeMove前放置可完全恢复的初始重叠。仅登记，不调整物理解穿透强度或增加补偿。

- [ ] BF144-D22 / P3 — 空中子步尾部未清零源阈值内的极小水平残余速度。
  - Source：fresh UE `PhysFalling:5249–5253`在每个非着地子步末尾检查水平速度平方≤`UE_KINDA_SMALL_NUMBER*10`，满足时只保留重力方向速度；按cm/s转m/s后阈值约0.00031623m/s。
  - Target：`src/als/character/movement/motion.rs:2827–2829`直接保留deflected velocity并扣除时间，未移植该阶段。
  - 反例：完整加载角色无输入、空中水平初速0.0001/0.0002m/s，source子步末尾应清零，target分别原样保留；0、0.0004、0.001m/s对照PASS。`bf144_air_tiny_horizontal_velocity_uses_source_final_zeroing`共5组3PASS/2FAIL，证据`target/bf144/06/tiny_velocity/`。
  - 漏检：旧速度回归以较大运动量或全零为输入，未覆盖这个源厘米平方阈值。实际影响为亚毫米每秒残余漂移，优先级P3；只登记，不扩大其他近零阈值调整。

- [ ] BF144-D23 / P2 — 落地滚翻启动读取本帧物理速度，偏离源保留的Character速度快照。
  - Source：fresh `MovementComponent.cpp:35,186–188`让CMC先于owner Tick；ALS `NotifyLocomotionModeChanged:518–527`用LocomotionState.Velocity判断滚翻，该值直到后续`RefreshLocomotion:1376`才刷新。UE `PerformMovement:2871–2885`的外力/launch可在当前CMC运动前改变实际velocity，而不回写这个角色快照。
  - Target：`src/als/character/movement/motion.rs:3697–3701`的落地仲裁读AlsActualMotion快照；`state/air.rs:337–344`另记录本帧LinearVelocity，`character/actions/roll_drive.rs:2185–2193`却用后者决定是否真正启动Roll，两个消费者不一致。
  - 反例：完整角色从0.94m中心高落地，保留下降速度3m/s而当前速度被更新为8m/s时，source应普通落地，target启动Roll；保留8m/s而当前3m/s时，source应Roll，target没有启动。两份速度相同的3/3、8/8对照PASS。`bf144_landing_roll_uses_retained_character_velocity`共4组2PASS/2FAIL，证据`target/bf144/07/retained_velocity/`。
  - 漏检：旧实际落地测试及阈值测试通常让pre-impact和Character快照保持一致，或只验证ragdoll分支；未组合当前帧速度更新与保留快照分离。仅登记，不修改recording或动作实现。

- [ ] BF144-D24 / P2 — 蹲下保持脚底的下移漏掉源碰撞扫掠。
  - Source：fresh UE `CharacterMovementComponent.cpp:3292–3327`先缩短胶囊，然后以实际世界quat调用`MoveComponent(...,bSweep=true)`沿重力下移；ALS `AlsCharacterMovementComponent.cpp:233–237`在地空模式切换后始终保持base-location策略。倾斜胶囊的缩短并不能保证完整向下位移仍无阻挡。
  - Target：`src/als/character/movement/motion.rs:3361–3370,3440–3455`直接写入缩短形体与目标Position/Transform，没有对这次重定位做sweep；后置物理解穿透及floor调整无法保留先扫掠后运动的源阶段合同。
  - 反例：完整角色站姿倾斜60°/90°，实际胶囊底部距地2.15cm，请求蹲下。源缩短形体的向下sweep在0.34m请求量之前命中地面；target在物理前中心却到y=0.28150/−0.01850m，低于蹲姿实际形体不穿地所需0.43/0.30m。`bf144_crouch_base_shift_keeps_source_swept_move`共6组4PASS/2FAIL，无倾斜和无地面4组对照通过；证据`target/bf144/08/tilted_initial/`。
  - 漏检：旧父空间/尺度蹲起测试核对世界位置与脚底偏移，但角色仍直立，缩短+下移扫过的体积包含在原形体中，不能触发阻挡。只记录倾斜角色边界，不宣称默认直立蹲下必然穿地；没有扩大为通用物理修复。

- [ ] BF144-D25 / P2 — 倾斜角色站起净空误用actor姿态。
  - Source：fresh UE `CharacterMovementComponent.cpp:3377–3396,3440–3457`按scaled尺寸构造站起query shape，并用`GetWorldToGravityTransform()`做OverlapBlockingTest；默认竖直重力时查询胶囊直立，与actor倾斜独立。站起后的实际组件移动仍保留actor quat，这两个空间不能混用。
  - Target：`src/als/character/movement/motion.rs:3356,3382–3394`所有站起shape-intersection都传`world.rotation()`，把actor姿态套入源gravity-space净空查询。
  - 反例：完整角色空中、蹲姿、pitch60°；距离中心z=0.65m的侧墙让target拒绝站起而source直立query无阻挡；相对蹲姿中心y=1.1m的顶板让target允许站起而source直立query应阻挡。同一目标几何上的source姿态查询确认阴阳边界，`bf144_crouch_stand_clearance_uses_source_gravity_orientation`共6组4PASS/2FAIL，直立3组与倾斜无障碍对照通过；证据`target/bf144/08/tilted_initial/`。
  - 漏检：旧站起阻挡/父空间测试只使用直立或yaw旋转，因此gravity orientation与actor quat相同。只登记未修复差异，未改站起容差或缩放公式。

- [ ] BF144-D26 / P2 — 可配置蹲姿半高低于半径时，只夹形体而未夹中心和mesh偏移。
  - Source：fresh UE `CharacterMovementComponent.h`的CrouchedHalfHeight为EditAnywhere/BlueprintSetter且仅ClampMin=0；`CharacterMovementComponent.cpp:3247–3254,3294–3299,3335–3341`在实际Crouch先取max(0,radius,requested)，再用这个值更新center和OnStartCrouch。`Character.cpp:514–527`把实际半高差用于mesh相对偏移。
  - Target：`src/als/character/character.rs:197–199`生成形体时将圆柱长夹到0，但`movement/motion.rs:3425–3439`、`character_assembly.rs:66–68`和transaction仍直接用未夹到radius的配置；物理形体和位置/mesh/floor尺寸状态不一致。
  - 反例：默认R=0.3m、standing H=0.9m，把公开crouched H配置成0.2m。完整角色蹲下的实际shape H=0.3m正确，但center从100到99.3m（source99.4），mesh相对y=−0.22m（source−0.32），缓存H=0.2m（source0.3）。`bf144_crouch_configured_height_keeps_source_radius_clamp`共5组4PASS/1FAIL，0.3/0.56/0.9/1.1m对照通过；证据`target/bf144/08/final/`。
  - 漏检：现有参数测试只核对默认56cm大于30cm半径，缩放测试也维持此关系；没有测试源公开配置边界的形体与实际consumer一致性。默认配置不触发，本项只登记，不扩大配置规范化设计。

- [ ] BF144-D27 / P2 — 配置为更高蹲姿时缺少源净空拒绝。
  - Source：fresh UE `CharacterMovementComponent.cpp:3301–3319`明确处理crouch H>当前H的分支，在目标center做blocking overlap；遇阻恢复旧capsule尺寸并返回，不提交bIsCrouched/OnStartCrouch。该配置由源公开CrouchedHalfHeight Setter支持。
  - Target：`src/als/character/movement/motion.rs:3361–3370`对所有Crouching目标直接提交Collider、Stance和Position，净空查询只存在于Standing分支。
  - 反例：完整角色初始center y=100m/H=0.9m，顶板底面100.98m，初始无重叠；将crouched H设为1.1m后请求蹲下。source目标center100.2m、H1.1查询被顶板阻挡，应保持Standing/旧形体；target提交Crouching/新形体并被后置solver大幅推下。`bf144_crouch_growing_capsule_checks_source_clearance_before_commit`共4组3PASS/1FAIL，普通56cm两组和1.1m无顶板对照通过；证据`target/bf144/08/final/`。
  - 漏检：已有headroom测试都围绕UnCrouch，Crouch夹具默认只缩小胶囊；没有覆盖源明确写出的增高分支。默认配置不触发，未修改生产代码或强行调整尺寸。

- [ ] BF144-D28 / P2 — 缺少顶点子步，跨过顶点的整步位移可能漏掉头顶接触。
  - Source：fresh UE `CharacterMovementComponent.cpp:248–253,4970–5006`默认ForceJumpPeakSubstep=1，跨越竖直速度零点时先把timeTick缩到TimeToApex、归零竖直速度，并退回剩余时间；随后5031的SafeMove先扫过真实上升段。fresh CDO允许每轮2次apex切分，项目/引擎配置没有覆盖默认开关。
  - Target：`src/als/character/movement/motion.rs:2765–2782`直接以整子步前后速度的平均值构造一条位移，未在apex拆分。无碰撞时中点积分仍精确，不能据自由抛物线一致排除碰撞路径差异。
  - 反例：完整角色空中向上0.2m/s，顶部与平顶板仅1.5mm间隙，统一使用Bevy重力9.81。源先上升至约2.039mm顶点，必撞顶；target在50ms帧只向下2.263mm，100ms同样跳过首次上升接触。独立源pullback/分段公式的末位置应99.99619/99.96941m，实际99.99773/99.97095m；`bf144_jump_apex_split_keeps_source_ceiling_contact`共6组4PASS/2FAIL，证据`target/bf144/09/apex/`。
  - 漏检：旧跳跃轨迹/帧长测试验证无障碍曲线，普通顶板碰撞测试未组合零速点位于子步内部；无顶板3组和16.7ms未越顶点撞顶的对照都通过。当前反例影响为毫米级接触路径差，不宣称默认跳跃高度存在大误差；bNotifyApex在CDO为False，未把未使用的callback另列缺陷。

- [ ] BF144-D29 / P2 — 基座携带的阻挡查询固定使用直立胶囊。
  - Source：fresh UE `UpdateBasedMovement:2578–2580,2667–2677`取组件当前世界quat并用于实际MoveUpdatedComponent sweep；本次native IgnoreBaseRotation=True只禁用基座对actor朝向的改变，不把actor已有pitch/roll清零，FastAttachedMove=False要求保留碰撞查询。
  - Target：`src/als/character/movement/motion.rs:2870–2874`基座跟随的shape cast硬编码`Quat::IDENTITY`，与actual actor世界形体不同；已完成的普通基座阻挡测试只使用直立角色。
  - 反例：完整角色在运动学平台上，actor pitch60°，平台向z正方向移动1m，前方墙面z=1.2m。源形体z半宽0.819615m、安全中心≤0.380385m，独立同几何world-quat查询在0.380361m命中；target携带阶段中心z=0.888910m，物理结束仍z=0.847278m，约0.467m穿墙。`bf144_base_carry_sweep_uses_actual_world_capsule_rotation`共8组6PASS/2FAIL，直接和父级倾斜均失败；直立阻挡与各无墙对照通过，证据`target/bf144/10/carry_rotation/`。
  - 漏检：`movement_base_rotation_keeps_the_capsule_bottom_anchor`旋转的是平台，actor保持identity；`component_transport_precedes_the_character_movement_sweep`也只检查直立形体。该分支与D18的Air局部quat错误分别位于基座跟随和空中运动入口，不能用修好其中一处作为另一处证据。仅登记，不改生产实现。

- [ ] BF144-D30 / P2 — 世界暂停时基座跟随仍执行位移并刷新缓存。
  - Source：fresh B_Als_Character的actor/CMC PrimaryTick均bTickEvenWhenPaused=False；UE `LevelTick.cpp:1782–1784`在pause进入`TickTaskManager.cpp:1574–1640`，只执行bTickEvenWhenPaused的tick，CMC的post-physics tick也继承False默认。因此暂停期间外部修改平台Transform不应运行角色UpdateBasedMovement/SaveBaseLocation；owner自定义倍率0并非世界暂停，仍执行这些阶段。
  - Target：`src/als/character/movement/motion.rs:2558–2559,2580,2831–2895`无world-pause条件，直接按基座Transform差值携带并保存；delta=0无法挡住这个与delta无关的分支。
  - 反例：完整角色稳定站在运动学平台，暂停world后从外部把平台x平移0.1m。source角色x及保存基座x均应保持0；target角色和缓存均立即变为0.1。`bf144_base_follow_respects_world_pause_and_owner_zero_distinction`共4组3PASS/1FAIL，未暂停、owner倍率0和暂停静止平台对照通过；证据`target/bf144/10/final/`。
  - 漏检：旧暂停测试主要冻结时间驱动的物体，平台Transform也不再变化，未触发follow的delta-independent路径。此反例要求暂停期间有外部基座位姿变化，不宣称正常冻结的平台会自行漂移；仅登记，不改通用调度或暂停策略。

- [ ] BF144-D31 / P3 — 有效离地距离带内仍由Tnua悬浮弹簧缓慢抬升角色。
  - Source：fresh ALS `PhysWalking:393–394,525–526`先保持水平velocity，再由UE `AdjustFloorHeight:6295–6330`仅在floor距小于1.9cm或大于2.4cm时调整；静止、无碰撞、距离在带内时应保留当前位置，没有固定悬浮高度的持续弹簧。
  - Target：`src/als/character/movement/motion.rs:151–168`把Tnua float height固定成standing H+0.019+0.005m；当前依赖Tnua Walk的默认spring strength=400，`walk.rs:410–419,624–642`持续施加offset×strength，gravity compensation同时保留。后置floor consumer虽然实现了源距离带，却没有去掉前置motor已产生的带内位移。
  - 反例：完整默认角色静止在4m平面，离地2.0/2.15/2.35cm，实际floor sweep确认仍在源允许带内；30帧后分别抬升3.464/2.165/0.433mm，末velocity.y都被回写为0。`bf144_base_height_residual_is_checked_without_platform_motion`共9组6PASS/3FAIL；owner0三组及夹具只将spring strength改0的三组均保持位置，证据`target/bf144/10/final/`。该反例不需要移动基座，也不依赖U01的大箱体查询误差。
  - 漏检：旧floor夹具多验证一次查询、Grounded状态、horizontal velocity或允许整个离地距离带；没有在带内对静止Position持续不变做断言，velocity.y=0也会掩盖抬升。影响目前为毫米级，因此P3；额外抬升由源条件、实际consumer和数据消融共同定位，未据观察加入补偿或修改已完成项。

- [ ] BF144-D32 / P2 — 角速度继承按未缩放胶囊半高取脚底。
  - Source：fresh UE `GetImpartedMovementBaseVelocity:1204–1209`以当前组件位置减GetScaledCapsuleHalfHeight沿重力求脚底，`Character.cpp:930–965`以该点相对基座transform的向量与实际body angular velocity求叉积；native bImpartBaseAngularVelocity和三个轴均True。
  - Target：`src/als/character/movement/dynamics.rs:96–99,520–522`只减params里的未缩放半高；跳离时的support helper实际消费这个点，缺少actor世界scale。
  - 反例：完整角色uniform scale=2，站在动态平台上，以绕X轴1rad/s的平台角速度跳离，源脚底采样的z继承约0.273666m/s，target约1.173666m/s，多0.9m/s。直接scale和父级scale均失败；scale1、无旋转及绕Y旋转对照通过。`bf144_inherit_angular_velocity_uses_scaled_capsule_foot`共12组10PASS/2FAIL，证据`target/bf144/11/jump_and_permissions/`。
  - 漏检：旧angular support测试直接给接触点，绕Y的水平旋转也不会暴露Y采样点错误；缩放跳跃测试只验证JumpZ。最初含0.2m z偏心的组合同时触发D34，已将z偏心去掉分离最小反例；未改生产实现。

- [ ] BF144-D33 / P2 — 基座速度来源忽略已提供的组件速度。
  - Source：fresh `Character.cpp:891–923`对movable base优先GetPhysicsOwnerVelocity，再按零值回退actor/root和body velocity；`PrimitiveComponentPhysics.cpp:1333–1344,1662–1665`证明非simulated组件返回ComponentVelocity，simulated组件返回body velocity，不能统一视作同一来源。
  - Target：`src/als/character/movement/dynamics.rs:101–123`始终读取owner LinearVelocity，未读取项目已有`AlsComponentVelocity`；同文件`component_velocity:74–76`在其他运动入口已经区分这两种来源，但本consumer未使用。
  - 反例：完整角色从运动学平台跳离，component x速度1/physical0时source应继承1m/s、target为0；component3/physical1时source应3、target仅1。`bf144_inherit_linear_velocity_uses_source_component_priority`共6组4PASS/2FAIL，零速、物理fallback、Dynamic优先physics和Static不继承对照通过，证据`target/bf144/11/jump_and_permissions/`。
  - 漏检：旧基座夹具仅设置LinearVelocity，未给component与physical不同的合法输入；本项只证明已提供组件速度时的选择遗漏，不宣称所有默认平台都有非零ComponentVelocity，也未展开平台脚本实现。

- [ ] BF144-D34 / P2 — 跳离和走离基座丢弃竖直支持速度。
  - Source：fresh UE `DoJump:1163–1177`先设置自身起跳速度，Falling的`OnMovementModeChanged:1459–1470`随即调用ApplyImpartedMovementBaseVelocity；`1194–1230,2744–2758`把线速度和脚底角速度的各开启轴完整相加。native bImpartBaseVelocityZ=True，StayBasedInAir=False，包含上升和下降平台的有符号速度。
  - Target：`src/als/character/movement/dynamics.rs:517–531`把离地继承强制with_y(0)且只写XZ motor；`movement/motion.rs:2662–2666`的起跳boost没有补入平台竖直速度，`3769–3787`的walking→falling剩余预算也只加入with_y(0)的support。
  - 反例：完整角色从竖直速度+2/−2m/s的动态平台跳离，源首帧vy应6.0365/2.0365m/s，target均4.0365m/s；静止平台对照通过。`bf144_inherit_vertical_base_velocity_keeps_source_signed_value`共3组1PASS/2FAIL，初始角速度偏心组合也复现丢失−0.2m/s竖直贡献；证据`target/bf144/11/jump_and_permissions/`与`11/initial/`。
  - 走离反例：1m宽平台，x=0.55m、水平3.75m/s，100ms帧的首个50ms步走过perch范围；源StartFalling应在剩余50ms前继承平台vy=−2/0/+2，末vy为−2.4905/−0.4905/+1.5095m/s，target三者均−0.4905。`bf144_inherit_walk_off_keeps_vertical_support_velocity`共3组1PASS/2FAIL，记录在`target/bf144/11/final/`；这与跳键路径共享丢Y语义，但消费者需分别核对。
  - 漏检：旧平台跳离回归覆盖水平速度改变，直接Tnua默认起跳测试与scale测试都没有竖直移动基座。仅登记该轴和阶段遗漏，不改JumpZ或重力。

- [ ] BF144-D35 / P2 — 允许站在禁止作为基座的Pawn上，未执行源JumpOff。
  - Source：fresh `Character.cpp:1263–1265,1452–1461`在SetBase时通知BaseChange；`Pawn.cpp:241–249`按根primitive的CanCharacterStepUpOn判断是否可作为基座，No时拒绝。`CharacterMovementComponent.cpp:1251–1285`随后JumpOff提供侧向脱离并切Falling；ALS原生角色胶囊为ECB_No，JumpOffJumpZFactor=0只去掉上弹，不取消侧向脱离。
  - Target：`src/als/character/movement/motion.rs:2784–2801,4125–4132`落地及保存基座直接接受floor实体，未调用Pawn基座许可/JumpOff；现有allows_step_up仅用于步阶和Mantle，params中的jump_off_jump_z_factor没有运行时消费者。
  - 反例：完整角色落到另一角色胶囊顶部，目标已显式提供allows_step_up=false以排除D14默认metadata遗漏。源应拒绝作为基座并侧向离开、保持Air；target为Grounded、base=该角色、velocity全零。`bf144_inherit_forbidden_pawn_base_runs_source_jump_off`共3组2PASS/1FAIL；同形普通prop即使NoStep仍可站立（AActor默认许可），显式Yes的Pawn也可站立，两对照通过，证据`target/bf144/11/jump_and_permissions/`。
  - 漏检：旧CanStepUp反例只从侧面撞步阶，没有从上方落地触发SetBase→BaseChange。即使修好D14的默认No元数据，这个独立消费者仍缺失；本项不涉及AI控制、网络或随机方向的精确复刻。

- [ ] BF144-D36 / P2 — 当帧起跳前仍跟随旧基座，偏离源检查顺序。
  - Source：fresh UE `ControlledCharacterMove:6441–6461`先CheckJumpInput；成功后DoJump→Falling→OnMovementModeChanged在`1463–1472`继承速度并清除base，之后`PerformMovement:2840`才MaybeUpdateBasedMovement。因此起跳当帧不再使用旧基座尚未消费的Transform位移。
  - Target：`src/als/character/movement/motion.rs:2554–2559`把follow放在Tnua Sensors之前，accepted_jump_state则在Logic之后；角色仍Grounded时先被携带，再清base起跳。
  - 反例：完整角色稳定站在运动学平台；同帧把平台以保持速度为0的外部位姿更新平移x=0.2m，并按跳键。源角色不应再跟随这次旧base位移，x仍0；target先移到x=0.2再起跳。owner1和0均失败，平台不动起跳及平台移动但不跳两组通过。`bf144_inherit_jump_clears_base_before_same_frame_transform_follow`共4组2PASS/2FAIL，证据`target/bf144/11/final/`。
  - 漏检：旧“component transport precedes character movement”回归只核对普通Walking；跳离测试改变velocity却没有同时改变尚未处理的base Transform。此项限定已经接受的Jump时序，与D30世界暂停门槛和D34速度轴遗漏独立；未重排生产调度。

- [ ] BF144-D37 / P2 — 站在动态基座上时未提交源下压力。
  - Source：fresh native CMC bEnablePhysicsInteraction=True、Mass=100、StandingDownwardForceScale=1；UE `TickComponent:1801–1805`在普通运动后调用`ApplyDownwardForce:11784–11795`，有效blocking floor且base simulated时在ImpactPoint提交Gravity×Mass×Scale。owner倍率0仍执行该tick；力的积分使用world physics时钟。
  - Target：`src/als/character/character.rs:363–429`的动态胶囊及`src/als/character/movement/motion.rs`的floor/base consumer未提交对应下压力，默认Tnua悬浮间隙使接触求解也没有产生等价反作用。
  - 反例：完整角色稳定站在GravityScale0、质量100/1000kg的动态平台，胶囊离地0.023667m，控制首帧平台速度为0。按用户采用的Bevy重力9.81，16.667ms后源平台vy应−0.1635/−0.01635m/s，target均0；owner0仍应−0.1635，target0。`bf144_force_standing_downward_uses_character_mass`6组3PASS/3FAIL，Static、Kinematic和角色gravity0对照通过；证据`target/bf144/12/impact_downward/`。
  - 漏检：旧基座回归主要核对角色跟随与继承，没有断言被站立的动态body受力；角色velocity.y=0和正确floor距离均不能证明下压力已移植。仅登记，不更改物件质量、场景或重力政策。

- [ ] BF144-D38 / P2 — blocking sweep命中动态物体后缺少源推挤力。
  - Source：fresh UE `HandleImpact:7783–7811`进入`ApplyImpactPhysicsForces:7814–7930`；native InitialPushForce500、PushForce750000、PushScaledToMass=False、UsingZOffset=False。本机CVar `p.ApplyCharacterCollisionForceAsImpulse=False`，静止阈值1cm/s内用500×30 kg·cm/s²，移动body用750000 kg·cm/s²，均AddForceAtLocation，不能按旧版本把500当一次冲量。
  - Target：`src/als/character/movement/motion.rs`空中/地面sweep和slide只调整角色位移/速度，未把命中的simulated body与源力参数提交给物理；实际air blocking命中后目标body保持原速度。
  - 反例：完整角色以5m/s撞向距胶囊前缘7cm的动态竖面，质量100kg，16.667ms首次命中。静止物体源应增加0.025m/s，target增加0；初速+0.1/−0.1m/s时源应到+1.35/+1.15，target仍+0.1/−0.1。`bf144_force_blocking_impact_keeps_source_force_branch`8组2PASS/6FAIL，同时覆盖质量200、初速0.005和20ms帧；Static/Kinematic不施力对照通过，证据`target/bf144/12/impact_downward/`。
  - 漏检：旧动态墙测试只检查角色slide及被撞物体的输入速度，未核对HandleImpact向body输出的force。D20是角色保留动态接触速度的独立消费者；补齐其中一条不能作为另一条证据。本批仅登记，不修改生产实现。

- [ ] BF144-D39 / P2 — simulated overlap物体首次触碰时缺少CapsuleTouched冲量。
  - Source：fresh native胶囊GenerateOverlapEvents=True，CMC PhysicsInteraction=True、TouchForceFactor1、TouchScaledToMass=True、Min−1/Max250；UE `SetUpdatedComponent:1037–1042`绑定BeginOverlap，`CapsuleTouched:11674–11717`以角色水平速度×body mass后夹到MaxTouchForce，按几何与运动方向混合得到方向并AddImpulse。`PrimitiveComponent.cpp:224–230,4100–4140`要求双方生成事件且pair response=Overlap。
  - Target：完整角色没有对应overlap事件施力消费者；动态Sensor提供了实际重叠但仅作为query pair，没有转成源Touch impulse。
  - 反例：完整角色以3.75m/s横移，与相侧0.45m处的半径0.2m模拟物体开始重叠；fixture明确使用Overlap语义，COM始终在0.36m排斥半径外，分离D40。质量100/10/0.001kg时源冲量产生速度0.025/0.25/3.75m/s，target均0。`bf144_force_overlap_touch_keeps_source_impulse`5组2PASS/3FAIL；静止角色和Kinematic物体对照通过，ContactGraph确认实际重叠且无接触约束；证据`target/bf144/12/overlap/`。
  - 漏检：旧碰撞筛选只证明Sensor不阻挡角色，未核对源允许的simulated Overlap仍应收到交互冲量。本项限定双方开启overlap事件的模拟物体，不宣称普通Block物件都触发CapsuleTouched；仅登记，不改场景profile。

- [ ] BF144-D40 / P2 — 持续overlap物体缺少源排斥和近距离停速。
  - Source：fresh UE `ApplyRepulsionForce:11798–11892`在普通tick遍历movable simulated overlap body；native RepulsionForce2.5、Mass100，半径=scaled capsule radius×1.2。body线段到胶囊表面有hit、未penetrate且水平DistanceNow²<2.5（源厘米数值）时清速度；否则靠近/静止或penetrate且COM在半径内时施加250 kg·cm/s²径向力。`ChaosEngineInterface.cpp:976–1028`确认按COM距离筛选及Constant力，非直接速度或加速度。
  - Target：普通角色没有该overlap body consumer；Avian Sensor pair只保留重叠，既不径向施力也不执行源停速分支。
  - 反例：静止完整角色，1kg球体与胶囊Overlap、重力关闭，源Touch因角色水平速度0而无冲量。球体中心距0.2/0.34m且静止时源首帧vx应+0.041667m/s、target0；距0.31m初速+1时源清0、target+1；距0.32m初速−0.1时源变−0.058333、target−0.1。`bf144_force_overlap_repulsion_keeps_source_radial_and_stop_branches`7组3PASS/4FAIL，向外离开、半径外和Kinematic对照通过；证据`target/bf144/12/overlap/`。
  - 漏检：阻挡接触和Sensor筛选测试都没有验证CMC对已知overlap列表的持续交互；近距离条件还包含源的厘米平方比较，不能以“2.5cm范围”粗略替换。body/bone选择在当前普通刚体成立，Ragdoll多body归其后续批次；仅登记。

- [ ] BF144-D41 / P2 — 地面查询不可站立时丢弃初始穿透命中，提前切到Air。
  - Source：fresh ALS `ComputeFloorDist:797–800`在无可接受floor时只清bWalkableFloor，保留HitResult的start penetration；`PhysWalking:529–559`对该命中执行ResolvePenetration并ForceNextFloorCheck，当前迭代有start penetration时不进入CheckFall。UE `GetPenetrationAdjustment/ResolvePenetration:3621–3646`与MovementComponent的MTD路径不要求walkable floor。
  - Target：`src/als/character/movement/motion.rs:3932`对`!valid`返回None，`4003–4007,4016–4067`只在validated floor存在时使用penetration数据；没有可站立floor时跳过该恢复，随后把None解释为Air。
  - 反例：完整角色Grounded、静止，中心y=100m，墙面x=0.1m，脚下无近地面；retry胶囊实际产生distance0、深度0.198499m、normal≈−X。源应本迭代脱离穿透并保持Grounded，target仅由solver侧推至x=−0.066073m，仍重叠却转Air、vy=−0.1635。`bf144_recovery_invalid_floor_keeps_source_penetration_result`4组3PASS/1FAIL；有walkable地面、无墙和完全无floor对照分别按源保持/落空，证据`target/bf144/13/penetration/`。
  - 漏检：旧恢复测试提供可站立的line floor，只覆盖目标保留penetration的分支。D16是inside ray错误接受exit面，D21是Air SafeMove入口；本项限定Grounded无walkable floor时的数据丢失，不以修复相邻入口代替。

- [ ] BF144-D42 / P2 — 静止角色失去支撑时提前积分空中重力。
  - Source：fresh ALS `PhysWalking:430–438`在本步MoveDelta≈0时将remainingTime置0，再FindFloor；UE `CheckFall/StartFalling:5401–5456`对零DesiredDist同样保留remainingTime0，切Falling但`StartNewPhysics`不执行空中积分。没有动态基座继承时，当前帧vy及高度保持，下一帧才积分gravity。
  - Target：`src/als/character/movement/motion.rs:3507,3635–3645`对零水平位移提前跳过FindFloor，常规Tnua/Avian仍先执行重力，`4070–4108`物理后才发布Air；缺少源的零剩余时间约束。
  - 反例：完整角色静止在高处静态平台，下一帧删除该平台。站立/蹲伏×16.667/50ms下源首帧vy应0且高度不变，target分别vy=−0.1635/−0.4905m/s、下降约1.59/14.31mm；各模式都正确切Air，差异在积分阶段。`bf144_recovery_stationary_floor_loss_defers_falling_integration`8组4PASS/4FAIL，保留平台的4组对照均Grounded、vy0；证据`target/bf144/13/stationary_loss/`。
  - 漏检：旧基座销毁与floor失效测试只断言切Air和没有水平瞬移；移动走离的remaining-time测试有非零MoveDelta，没有触发该早退。反例使用静态基座，排除D34竖直支持速度继承；仅登记，不改重力值。

- [ ] BF144-D43 / P2 — Roll启动未立即提交动作和蹲伏，依赖随后动画通知。
  - Source：fresh `AlsCharacter_Actions.cpp:90–100`在Montage_Play成功后立即SetLocomotionAction(Rolling)，`AlsCharacter.cpp:1080–1099,801–827`随即ApplyDesiredStance请求Crouch；同帧CMC BeforeMovement消费请求。AM_Als_Roll的action notify从0.0001s开始，不替代这个显式启动步骤；owner时间倍率0也不取消输入调用或SetLocomotionAction。
  - Target：`src/als/character/actions/roll_drive.rs:2122–2244`只启动roll state，`src/als/notifies/animation_events.rs:1003–1013,1048–1060`通过随后notify设置action/physical stance override。该PostUpdate阶段晚于Update里的`movement/motion.rs:2559–2562,3270`胶囊reshape；clip时间不前进时notify甚至尚未开始。
  - 反例：完整站立角色按Roll，owner1首帧roll.active=True、action=Rolling，但实际stance仍Standing、胶囊H0.9，第二帧才变Crouching/H0.56；源首个运动帧已应蹲伏。owner0从Standing或Crouching启动，target虽active但action连续两帧仍None，源应立即Rolling。`bf144_roll_lifecycle_start_commits_action_and_crouch_before_movement`6组3PASS/3FAIL，已有蹲伏的owner1及两组无Roll输入对照通过；证据`target/bf144/14/start/`。
  - 漏检：旧Roll蹲伏和root-motion回归观察已运行多帧的动作，notify首区间测试只验证事件本身，没有比较源显式StartRolling提交先于CMC的关系。本项限定启动动作/胶囊时序，根运动位移与碰撞另行审计；未改生产实现。

- [ ] BF144-D44 / P2 — Roll输入使用运动后的mode判定，改变同帧地空切换的接收结果。
  - Source：fresh `AlsCharacterExample.cpp:197–202`输入回调立即`StartRollingGrounded(1.3)`；`AlsCharacter_Actions.cpp:34–42`当场检查Grounded，然后接受或返回，没有等待本帧movement结束。CS bInterruptRollingWhenInAir=False，所以在Grounded已接受的Roll不会仅因随后进入Air而取消。
  - Target：`src/als/character/actions/roll_drive.rs:2122–2216`在PostUpdate读取locomotion再调用`roll.rs:144–154`，晚于Update内可能改变mode的falling/walking transaction。输入命令被留到运动之后重新判断。
  - 反例：完整角色以vy=−2m/s处于离地约2cm的Air时按Roll，50ms内普通落地，源输入应拒绝且低落地速度不自动Roll，target切Grounded后启动Roll。反向边界：在1m平台x=0.55、Grounded、水平3.75m/s时按Roll，100ms多子步先走离平台，源已接受且应保留Roll，target在Air拒绝；同几何50ms单子步对照仍接受。
  - `bf144_roll_lifecycle_acceptance_uses_input_time_locomotion`6组4PASS/2FAIL，普通Grounded启动、持续Air拒绝、无Roll输入的低速落地与50ms走离均通过；证据`target/bf144/14/lifecycle_core/`。旧入口测试分别在稳定地面/空中按键，未把输入与当帧碰撞模式变化组合；本项只登记许可阶段，不把同帧根位移差异归入此项。

- [ ] BF144-D45 / P2 — 空中Roll根运动写速时清掉原有竖直速度。
  - Source：fresh UE `PerformMovement:2962–2972`和`ApplyRootMotionToVelocity:4514–4521`均调用`ConstrainAnimRootMotionVelocity:2170–2186`；Falling必须保留当前重力轴速度，随后继续PhysFalling重力积分。fresh CS允许Roll进入Air后继续，A_Als_Roll根轨道的竖直增量恒0，也不代表可以把角色竖直速度置0。
  - Target：`src/als/character/actions/roll_drive.rs:2327–2332`直接以world root translation/world delta覆盖整个LinearVelocity，没有Falling轴约束；`movement/motion.rs:3981–3986`虽在末尾保留physics y，但此时先前速度已经被drive清除。
  - 反例：完整角色先接受Roll，再进入Air、原有vy为−2/0/+2m/s；16.667ms源应为−2.1635/−0.1635/+1.8365，target均−0.1635；100ms源为−2.981/−0.981/+1.019，target均−0.981。`bf144_roll_root_falling_keeps_prior_vertical_velocity`12组8PASS/4FAIL，六组无Roll与两组原vy0对照通过；证据`target/bf144/15/initial/`。
  - 漏检：旧`animation_root_velocity_restoration_preserves_falling_physics_axis`只单独执行drive后手工写入正确vy，再测末尾sync，因此掩盖了drive提前覆盖的问题。此项核对整帧实际consumer，独立于D34基座速度继承、D43启动蹲伏；仅登记，未改生产代码。

- [ ] BF144-D46 / P2 — 物理封顶与CMC单步窗口内，Roll首帧沿用普通运动预算而丢失根位移。
  - Source：fresh UE `PerformMovement:2962–2972`先把本帧动画root换成速度；ALS `PhysWalking:406–418`在HasAnimRootMotion时跳过普通CalcVelocity，并由ApplyRootMotionToVelocity覆盖速度，再MoveAlongFloor。因此已接受Roll的首个运动帧必须消费当前root增量。
  - Target：`src/als/character/movement/dynamics.rs:557–569`在输入尚未启动Roll时保存普通grounded_movement_delta；`roll_drive.rs:2327–2332`换成root速度时未清这个预算。`movement/source_physics_timing.rs:179–181`对超过33.333ms的完整帧补偿优先消费旧delta，覆盖新的root速度；CMC超过50ms时另一路多子步清delta，因此又恢复。
  - 反例：完整已蹲伏角色静止按Roll，frame delta34/40/50ms时原生根位移应16.366/22.077/31.595mm，实际水平位移均0；同帧已正确采样root且写入相应非零速度，物理前runtime仍Some(Vec3::ZERO)。`bf144_roll_root_first_frame_replaces_ordinary_movement_budget`8组5PASS/3FAIL，16.667/33.333及51/60/100ms对照通过；证据`target/bf144/15/first_frame/`。
  - 漏检：旧大帧Roll回归先播放到动作中段，再测试时钟，绕开了Roll开始前普通CMC已经写预算的首帧。夹具提前完成Crouch，排除D43；本项是物理消费优先级，不是原始轨道或播放权重问题，仅登记。

- [ ] BF144-D47 / P2 — Roll叠加Tnua角速度，重复应用转向。
  - Source：fresh native CMC bOrientRotationToMovement=False、bUseControllerDesiredRotation=False，UE `PhysicsRotation:6632–6636`提前返回；ALS `AlsCharacterMovementComponent.cpp:313–321`仍广播其手动rotation，由`AlsCharacter_Actions.cpp:116–140`执行一次half-life0.1的yaw阻尼。原生A_Als_Roll根旋转全identity，不存在另一次动画root旋转。
  - Target：`src/als/character/movement/motion.rs:3081–3088`把Roll目标方向继续喂给Tnua Walk，产生最多500°/s的angular boost；`roll_drive.rs:2304–2320`又直接写入正确的阻尼rotation，之后物理积分仍应用上述angular velocity。
  - 反例：完整已启动的Grounded Roll，当前yaw0.7rad、目标0，owner1的源单次阻尼应到0.6234437；物理前target也是0.6234436，物理后却到0.4779995，多转约8.333°。`bf144_roll_root_physical_turn_is_applied_only_once`9组6PASS/3FAIL，owner0.5/1/2均多转同一世界帧角度；同方向零角误差对照通过，仅在夹具物理前清AngularVelocity的三组也恢复源结果，证据`target/bf144/15/root_core/`。
  - 漏检：旧旋转单测直接执行roll drive验证Transform，未继续物理积分；整帧root运动测试主要令actor已朝向目标。消融只改变每个fresh App的一次数据，生产源码与二进制未改，后续owner分组重新使用原始完整路径复现；仅登记。

- [ ] BF144-D48 / P2 — Rolling动作通知结束后，root drive仍执行Roll转向。
  - Source：fresh `AlsCharacter_Actions.cpp:118–122`要求LocomotionAction=Rolling才执行RefreshRollingPhysics；action在montage有效时间0.933433s结束，而root montage此时仍active。`AlsCharacter.cpp:1516–1525`在HasAnyRootMotion时只刷新目标角并返回，因此该区间应保持actor朝向，不继续Roll阻尼。
  - Target：`src/als/character/actions/roll_drive.rs:2286–2288,2304–2313`仅按roll.active、非Ragdoll和根提取尾段门槛判定，未要求Rolling action；clip约0.933433–1.11s期间，通知已经结束仍然阻尼到旧Roll方向。
  - 反例：完整角色自然播放到0.75/0.816667s后，实际action=None、root motion仍active；当前yaw0.7、原Roll目标0，源应保持0.7，target在物理前已改为0.6234436。`bf144_roll_root_rotation_stops_with_action_notify`4组1PASS/3FAIL，也覆盖0.716667s下一帧跨过notify结束的边界；0.3s仍Rolling的对照正确。证据`target/bf144/15/root_core/`；这里在物理前取样，明确排除D47叠加角速度。
  - 漏检：旧自然结束测试核对action/stance/weight/clock，没有检查动画仍提根但action已结束时的rotation gate。初始组合观察同时包含D47，已按消费阶段拆开；未修改生产实现。

- [ ] BF144-D49 / P3 — 空中Roll在短帧下使用末速度积分，偏离源midpoint位置。
  - Source：fresh UE `PhysFalling:4910,4962,5018–5019`保存OldVelocityWithRootMotion，施加gravity后以新旧速度平均乘timeTick求位移；animation root motion也不例外。竖直初速度0且未碰撞时，下落距离为0.5×gravity×dt²。
  - Target：`src/als/character/movement/motion.rs:3703–3711`把活动Roll排除在普通falling transaction外；`movement/source_physics_timing.rs:132–147`只有world delta超过33.333ms才启用完整帧midpoint。短帧落入Avian `integrator/mod.rs:503–521`的末速度积分，当前6子步仍保留数值差。
  - 反例：完整Air Roll、vy初值0，16.667/25/33.333ms下源下降约1.3625/3.0656/5.45mm，实际多下降约0.223/0.511/0.908mm；速度都正确。`bf144_roll_root_falling_position_keeps_source_midpoint`12组9PASS/3FAIL；六组无Roll及34/50/100ms三组Roll对照均匹配源midpoint，证据`target/bf144/15/midpoint/`。
  - 漏检：旧Roll空中回归只核对velocity.y或大帧位移，没有核对短帧的竖直位置公式。本反例从vy0开始，排除D45清除已有速度；源重力输入按用户采用Bevy值9.81，差异只在积分方法。当前影响为亚毫米量级，记录P3，不添加补偿。

- [ ] BF144-D50 / P2 — Roll已结束root motion的blend tail仍阻止正常朝向更新。
  - Source：fresh UE `AnimMontage.cpp:1558–1597`在自然Stop时启动blend并通知AnimInstance，`AnimInstance.cpp:3722–3725`立即清RootMotionMontageInstance；主AB_Als的MontagesOnly模式下`SkeletalMeshComponent.cpp:4762–4782`因此不再IsPlayingRootMotion。ALS `RefreshGroundedRotation:1516–1525`在action=None且无root时恢复常规rotation，Aiming路径`1698–1706`使用1000°/s目标角限速和0.1s半衰期。
  - Target：`src/als/character/movement/motion.rs:416–440`把roll.active一直当作action_active，直到整段1.153846s播放结束才解除；自然root约0.853846s停止之后约0.3s的blend仍阻止普通朝向输入。
  - 反例：完整角色自然Roll到0.9/1.05s，action=None、root_motion_active=False；输入移动并瞄准view yaw1.2rad，源下帧应从0转到0.0318133rad，target仍0。`bf144_roll_root_blend_tail_restores_normal_rotation`6组4PASS/2FAIL；1.2s自然结束对照及夹具仅清已结束root的roll.active后三组均到0.03181338，证据`target/bf144/15/blend_tail/`。
  - 漏检：旧blend tail回归检查重新启动、权重和动作tag，没有验证正常Character rotation恢复的时点。D48是root尚活动但action结束时继续Roll专用转向；本项是root已停止后普通rotation仍被阻挡，两个独立消费者。消融仅改夹具数据，生产实现未改。

- [ ] BF144-D51 / P3 — Grounded Roll未按owner最小运动时间阻止根位移。
  - Source：fresh UE `CharacterMovementComponent.cpp:97,3531–3536`定义MIN_TICK_TIME=1e−6s，StartNewPhysics对owner delta低于该值直接返回；`TickCharacterPose:12027–12033`也不在该CMC阶段提取新root。静态基座和零外力下，这种tick不应移动胶囊。
  - Target：`src/als/character/actions/roll_drive.rs:2270,2285–2288`对普通Grounded/Air只检查world dt>1e−5，没有检查actor_delta；随后仍按动画微小增量写物理速度。Custom在`2338–2340`另有正确的actor最小时间门槛。
  - 反例：完整已启动Grounded Roll、world dt16.667ms、owner=1e−5，actor delta0.166667µs；源水平位移0，实际前移约0.834µm。`bf144_roll_root_owner_subminimum_does_not_move_capsule`8组7PASS/1FAIL；owner0冻结、owner1e−4/1正常移动及Custom四组对照符合源，证据`target/bf144/15/owner_minimum/`。只核对XZ，已有D12/D31的离地高度问题未重复记入。
  - 漏检：旧最小tick回归压低world时间或只覆盖Custom，没有用正常world时间×微小owner倍率穿过Roll的两种不同门槛。影响量级极小但条件与源不同，记P3，未改阈值或引入补偿。

- [ ] BF144-D52 / P2 — Roll落地没有进入源Walking交接，漏掉当帧floor高度调整。
  - Source：fresh UE `ProcessLanded:6373–6409`先SetPostLandedPhysics再StartNewPhysics(remainingTime)；`6412–6441`切Walking后，`OnMovementModeChanged:1441–1452`立即FindFloor、AdjustFloorHeight及SetBaseFromFloor。该交接在animation root motion期间同样执行，源允许离地带为1.9–2.4cm。
  - Target：`src/als/character/movement/source_physics_timing.rs:199–205`对Root Air完整帧调用falling_movement_sweep却传landing_cmc=None，使`movement/motion.rs:2731–2742`的landing remaining分支不可达；余步继续用Air slide，`4098–4114`仅在物理完成后发布Grounded，未执行该帧Walking入口的高度调整。
  - 反例：完整已播放Roll的Air角色、初始vy0，在4m静态平台上方5/20mm，50/100ms内实际落地。源应调整到1.9–2.4cm带内，target已Grounded但离地仅0.040/0.021mm；无Roll两组为21.499mm。`bf144_roll_root_landing_enters_walking_before_remaining_move`6组2PASS/4FAIL；夹具只关闭角色solver约束、仍保留floor sweep的两组也仅0.046/0.138mm，证据`target/bf144/15/landing_walking/`。
  - 漏检：旧Roll落地检查mode/velocity，没有比较切Walking的当帧floor合同；模式对照8组全通过也掩盖了这里的缺口。消融证明缺失高度不是单靠接触摩擦产生，D45以初始vy0排除；附带横向轨迹偏移尚不单独归因或登记。生产实现未改。

- [ ] BF144-D53 / P2 — Mantle的速度许可读取父刚体，丢失命中组件身份。
  - Source：fresh ALS `AlsCharacter_Actions.cpp:235–242`直接读取ForwardTraceHit的TargetPrimitive.GetComponentVelocity；UE `PrimitiveComponentPhysics.cpp:1333–1344`对非simulated组件返回该组件自己的ComponentVelocity，`SceneComponent.cpp:3022–3024`没有转取父刚体。`MovementComponent.cpp:382–388`也将velocity写给实际UpdatedComponent。
  - Target：`src/als/character/actions/mantle.rs:560,570–573`先把命中实体转成ColliderOf.body，再只在body上读取AlsComponentVelocity。显式提供在子碰撞组件上的速度被忽略，父body的组件速度被错误继承。
  - 反例：完整角色面对同一0.8m ledge，父Kinematic component速度0、命中child component速度0.2m/s时，source应拒绝，target probe Success且实际开始Mantle；父0.2、child0时反向误拒绝。`bf144_mantle_hit_component_velocity_keeps_component_identity`6组4PASS/2FAIL，独立组件及父子速度相同的4个对照PASS；证据`target/bf144/16/initial/`。
  - 漏检：旧compound-body测试把AlsComponentVelocity统一插在body，并将body值作为oracle；没有为命中child提供不同的合法组件速度。本项只确认已提供组件速度时的消费者身份，不假定所有移动平台都有非零ComponentVelocity；独立于D33基座继承消费者，仅登记。

- [ ] BF144-D54 / P2 — Mantle在箱体边角用接触法线偏转目标与朝向。
  - Source：fresh `CollisionConversions.cpp:115–137`对capsule/sphere sweep构建ImpactNormal；Chaos `Box.h:196–198`→`ChaosCore/AABB.h:249–283`从接触涉及的面选择最反向trace的几何面，正向扫箱体角应取前墙面。ALS `AlsCharacter_Actions.cpp:256–270,402`用该ImpactNormal生成TargetDirection、目标XY偏移和目标朝向。
  - Target：`src/als/character/actions/mantle.rs:557,582–588`直接把Avian normal1的水平接触法线当ImpactNormal；实际MantleIntent及已开始状态消费这个倾斜方向。
  - 反例：2.4m宽、0.8m高箱体前，actor x=±1.3m以−Z正向探测；命中侧边x=±1.2m。source目标x应保持命中x、yaw0，target目标横移约5.2cm、yaw约±20°；完整角色实际开始Mantle且保存同方向偏差。`bf144_mantle_corner_uses_geometric_wall_normal`4组2PASS/2FAIL，x=0/1.1m面内对照PASS；证据`target/bf144/16/initial/`。
  - 漏检：旧trace几何测试主要取面内命中，未覆盖侧向越过边缘但capsule仍能接触前墙的组合。与D10共享contact/impact语义差别，但属于独立的Mantle方向消费者；不将同一反例重复计入D10，不补射线或改查询库，仅登记。

- [ ] BF144-D55 / P2 — Mantle期间关闭了胶囊对其他查询的碰撞。
  - Source：fresh B_Als_Character CollisionCylinder为QueryAndPhysics/Pawn，Mantle启动、RMS PhysCustom及结束路径均保留该碰撞状态；`AlsCharacterMovementComponent.cpp:624–626`的MoveUpdatedComponent(...,false)仅跳过自身移动sweep。native角色回调只清理/恢复Overlay道具，没有关闭胶囊碰撞；动作文件中的NoCollision只属于后续Ragdoll路径。
  - Target：`src/als/root_motion_sources/mantle_drive.rs:684–698`在每帧物理前将Mantle collision owner设为true，`character/character.rs:101–105`使CollisionLayers=NONE；物理完成后才在`mantle_drive.rs:761–787`恢复，其他运动/物理查询因此看不到胶囊。
  - 反例：完整实际攀爬角色，在Physics StepSimulation之后、Writeback之前，从角色中心侧面使用Default mask仅查询该角色；源应命中保留的Pawn胶囊，target无命中且layers全0。`bf144_mantle_lifecycle_keeps_capsule_query_collision_during_physics`3组2PASS/1FAIL；普通角色及夹具只恢复该阶段CollisionLayers的两组均命中，证据`target/bf144/17/initial/`。
  - 漏检：旧Mantle测试验证自己可穿过障碍、帧后layers恢复及多个动作owner并存，没有检查物理阶段其他消费者的query。无sweep不能等价为关闭全部query/physics；本项不推测尚未测试的碰撞冲量数值，仅登记。

- [ ] BF144-D56 / P2 — 自动Mantle在安装RMS的同帧提前推进根运动。
  - Source：fresh CMC native bTickBeforeOwner=True；`MovementComponent.cpp:186–188`建立CMC先于角色的依赖，`AlsCharacter.cpp:292`在角色Tick才AutoStartMantling。`RootMotionSource.cpp:184,1809–1829`新RMS时间为0并加入PendingAdd，下一次CMC Prepare才推进；安装当帧不应推进RMS时间或产生其位移。
  - Target：`src/als/character/movement/motion.rs:3114–3116,3229–3245`在Update提前检测并启动自动Mantle，`root_motion_sources/mantle_drive.rs:730–757`在同帧PostUpdate已按owner delta推进并写回位置；手动与自动入口共用同一开始阶段。
  - 反例：完整Air角色中心(0,1.1,0)，面对0.8m ledge，无跳键而自动开始Mantle；world16.667ms、owner1/2时source新RMS时间0，target分别已到0.016667/0.033333s，并向前移动3.92/10.56cm。`bf144_mantle_lifecycle_auto_start_waits_for_next_movement_tick`3组1PASS/2FAIL；owner0保留时间与位置的对照PASS，证据`target/bf144/17/initial/`。
  - 漏检：旧自动Mantle Gate只检查成功开始、持续推进和暂停，未用source真实Tick依赖区分安装帧与首次CMC消费帧。已确认的3个Sequence均bEnableRootMotion=False，不存在动画root tick替代RMS首次推进的解释；仅登记。

- [ ] BF144-D57 / P3 — Mantle尾段对当帧新Aiming的提前淡出晚一帧。
  - Source：fresh `AlsCharacter.cpp:289–293`先RefreshRotationMode，Mesh受`124`的owner tick依赖约束；原生Mantle EarlyBlendOut的bCheckRotationMode=True、RotationModeEquals=Aiming。`AlsAnimNotifyState_EarlyBlendOut.cpp:44–66`在Mesh branching tick读取当前模式，匹配后立即停止实例，使用原生Low的0.35s HermiteCubic与High的0.4s Cubic淡出。
  - Target：`src/als/root_motion_sources/mantle_drive.rs:352–401`只约束在动画Dispatch之后，没有依赖当前帧aim refresh；`state/aim.rs:117–123`却在物理后的gait刷新之后更新RotationMode。本次真实入口执行中，提前淡出先读到旧ViewDirection，下一帧才停止。
  - 反例：完整low/high Mantle自然退出后，在原生early notify有效区按aim；该帧结束已为Aiming，但发布的Montage sample仍interrupted=false。再过0.1s，low/high实际权重0.856927/0.887876，source应0.801749/0.843750。`bf144_mantle_lifecycle_early_blend_conditions_and_duration_controls`12组10PASS/2FAIL；夹具在该帧开始前提供Aiming状态的两组恢复正确权重，其他input/crouch/air/idle共8组通过，证据`target/bf144/17/final/`。
  - 漏检：旧early条件测试直接传入已经更新的枚举/Tag，未跨真实aim producer与Montage consumer阶段核对同帧变化。这里限定为一个帧的淡出延迟，记P3；不据此推测其他图/通知均有相同问题，生产未改。

- [ ] BF144-D58 / P2 — Mantle初始化丢失actor scale及完整rotation，起始root消除结果错误。
  - Source：fresh `AlsCharacter_Actions.cpp:521–527`把初始root inverse与完整`GetActorTransform()`相乘；UE `Transform.cpp:98–113`及VQS乘法保留actor scale和完整quat对初始位移的影响。RMS随后只保存start location/rotation，并在`AlsRootMotionSource_Mantling.cpp:97–118`混合后取twist；不能提前丢掉初始pitch/roll或scale。
  - Target：`movement/motion.rs:3191–3205,3232–3243`仅传origin与yaw；`actions/mantle.rs:810–813`以单位scale、纯yaw重建初始Transform，`855–867`再据此消除root。数据在进入root driver前已经丢失。
  - 反例：完整角色手动Mantle到0.6m按比例缩放的ledge，actor scale0.75/1.5首帧位置相对独立source公式偏离约11.4/22.1cm；单位scale对照通过。同一0.6m台阶，初始pitch或roll为0.2rad及组合倾斜偏离约8.7–12.5cm，组合yaw另差约1.15°。入口前实际global rotation已取样确认仍保留倾斜。两个focused共12组7PASS/5语义FAIL，夹具只补回初始scale偏移或完整quat的5组恢复源结果；证据`target/bf144/18/initial_controls/`。
  - 漏检：旧root复合与scaled-base测试使用单位scale、纯yaw的初始actor；基座缩放、root旋转覆盖不能代替actor自身Transform。7种动作的正常actor轨迹168采样全部匹配，排除通用轨道/mesh basis错误。本项合并同一初始Transform传递边界下的两种缺失，未重复计数，未修复生产实现。

- [ ] BF144-D59 / P2 — Mantle缺少RMS前的基座携带，将平台位移计入角色自身速度。
  - Source：fresh UE `CharacterMovementComponent.cpp:2840,2941,2992`先MaybeUpdateBasedMovement，再准备并消费RMS；`AlsRootMotionSource_Mantling.cpp:122–124`用目标位置减已携带后的actor位置再除owner delta。`AlsCharacter.cpp:1376–1383`把该CMC自身速度交给LocomotionState。
  - Target：`movement/motion.rs:2846–2885`在Custom只保存基座快照，实际位移仅在Grounded分支执行；`root_motion_sources/mantle_drive.rs:740–742`因此从未携带的位置计算速度，`snapshot_actual_velocity_system`在Custom直接发布它，进入`AlsActualMotion`。
  - 反例：实际活动Mantle中，kinematic基座在该运动帧前横移2cm，owner0.5/1/2时RMS和发布动画速度分别多出2.4/1.2/0.6m/s；最终位置仍正确。`bf144_mantle_physics_velocity_excludes_pre_movement_base_carry`5组2PASS/3FAIL；静止基座及夹具先携带actor的对照通过。证据`target/bf144/19/initial/`。
  - 漏检：旧移动Mantle速度测试以target自身RMS之前的actor位置计算预期，未先复算源CMC基座携带；这个预期把同一错误带入oracle。仅登记实际速度消费差异，不把位置正确误当速度等价。

- [ ] BF144-D60 / P2 — Mantle物理后重算warp，偏离源基座脚底携带及绝对朝向。
  - Source：fresh UE `PostPhysicsTickComponent:1827–1836`对simulated base调用UpdateBasedMovement；`2574–2632`以胶囊脚底锚点应用基座location/quat变化，不使用scale。native bIgnoreBaseRotation=True，`SaveBaseLocation:3221–3224`保存absolute rotation，因此这次post携带不再改变actor朝向；源RMS已在物理前完成。
  - Target：`root_motion_sources/mantle_drive.rs:790–821`物理后刷新完整base transform并重新计算同一时刻的整个warp，直接覆盖actor位置与rotation；同时把scale变化也带入，并在owner delta=0时仍执行。
  - 反例：完整Mantle的simulated base在物理阶段后变化，owner0/1各测静止、平移、yaw、pitch和scale，共10组4PASS/6FAIL。基座yaw0.2rad时target额外旋转actor0.2rad；pitch0.15rad时位置偏离源脚底携带约5.62/4.45cm；scale变化时偏离约6.95/6.87cm。静止和纯平移均通过。`bf144_mantle_physics_post_base_uses_feet_carry_without_second_warp`，证据`target/bf144/19/initial/`。
  - 漏检：旧平台测试把基座变化直接代入target warp作为预期，没有区分物理前RMS与物理后UE基座携带。测试以受控物理后base transform作为共同输入，未推测UE/Avian平台刚体本身是否相同；只登记角色消费者，生产未改。
  - 同一无条件post写回还绕过了`mantle_drive:744`的最小时间门槛：静态目标、owner delta=0.5µs时，物理前位置正确不动，物理后却推进约3.4µm；源`StartNewPhysics:3533`在1µs以下不积分位置。4组中0、1、2µs对照通过，0.5µs失败；归入D60同一根因，不另增缺陷。

BF144-D61 已撤销（2026-10-14底层source复核）：初版oracle误认为`FRootMotionMovementParams::Clear()`清空transform；fresh `AnimationAsset.h:781–785,812`证明它只清bHasRootMotion/BlendWeight，getter仍返回旧transform，`RootMotionSource.cpp:1689–1691,1719–1724`不检查该标志而继续读取旧速度。目标在静态小delta场景保留旧速度与源相同，原失败属于oracle错误；证据保留于`target/bf144/19/clock/`，从有效缺陷和失败集合移除。

- [ ] BF144-D62 / P3 — Mantle在无地面支撑处中断时，同帧过早发布InAir。
  - Source：fresh `RefreshMantling:653–657`在owner Tick检测action结束，`StopMantling:707–708`解锁并切Walking，发生在该帧CMC之后；UE `OnMovementModeChanged:1442–1452`去掉重力轴速度并查floor/base，但没有在该回调中改为Falling。正常物理下落检查在下一CMC帧执行，当前动画帧应读Grounded。
  - Target：`mantle_drive.rs:603–614`在物理前先结束Mantle并置Grounded；同帧物理后的`movement/motion.rs:4073–4098`再次按无支撑改为Air，`ab_als_root`立即把它发布为`Als.LocomotionMode.InAir`。
  - 反例：完整low Mantle到clip约0.41936s、脚底未受地面/台阶支撑时清动作，源当前帧应Grounded，target及实际SourceAnimationInputs已经InAir；该帧水平退出速度仍与源公式一致。`bf144_mantle_physics_interruption_keeps_source_exit_velocity_and_mode`3组2PASS/1FAIL，起始地面段与已登上台阶段的中断对照通过。证据`target/bf144/19/`；限定一个帧的模式提前，不推测尚未验证的后续轨迹影响。

- [ ] BF144-D63 / P3 — Mantle的owner零/极小时钟未保留源重复应用旧rotation delta的行为。
  - Source：fresh `AnimationAsset.h:781–785,812`证明Clear保留旧transform；`RootMotionSource.cpp:1744–1749`仍返回其中的旧rotation delta，不检查bHasRootMotion或delta。UE `PerformMovement:3090–3097`在位置物理之后把它乘到当前actor quaternion，也没有owner delta门槛；component Tick与ShouldSkipUpdate不会仅因delta0而跳过。这里是源自身的时钟行为，不能以“暂停应该不转”替换其事实。
  - Target：`mantle_drive.rs:704–706`在owner delta≤1e−8直接跳过，随后`update_mantle_movement_base`重写固定RMS时刻的绝对rotation，所以不会累乘上一帧的delta。
  - 反例：完整角色以0.35rad朝向进入Mantle，运行到time约0.21667s，上一帧源rotation delta约−0.01751rad；下一帧owner delta0/5e−9/1e−8时源应再应用这一delta，target保持原角，差约1°。独立native-root/source-warp oracle的`bf144_mantle_physics_tiny_delta_reuses_source_rotation_delta`8组5PASS/3FAIL；无转向的4组和普通16.667ms推进对照通过。与已撤销D61的速度猜测分开记录，未改生产实现。

- [ ] BF144-D64 / P3 — Mantle冻结开始时的相对空间选择，未跟随目标当前可移动性。
  - Source：fresh RMS `PrepareRootMotion:83–92`每次调用`UseRelativeLocation`；`Character.h:218–220`转发到`Character.cpp:814–816`，`PrimitiveComponentPhysics.cpp:1644–1647`读取当前Mobility是否Movable。源的start/target虽在开始时已存储，运行中仍使用当前选择，因此Mobility切换确实可能造成源空间突变。
  - Target：`mantle_target_base:825–837`及物理后刷新用`base.relative_location`保留旧选择，从不重读当前对应的Static/Kinematic状态。
  - 反例：完整活动Mantle、目标位置(0,0.4,−1.4)m保持不动，Static→Kinematic或反向切换后，源公式分别按新primitive/world空间解释已存warp，target仍按旧空间，位置差约1.456m。`bf144_mantle_physics_live_mobility_controls_relative_space`4组2PASS/2FAIL，同状态两组通过；证据`target/bf144/19/mobility/`。这是外部改变目标可移动性的少见合同边界，记P3；不把源空间突变擅自改写成平滑行为。

- [ ] BF144-D65 / P2 — Ragdoll启动遗漏pelvis与spine_03刚体许可检查。
  - Source：fresh `AlsCharacter_Actions.cpp:717–723,759–764`要求两处`GetBodyInstance`都非空，否则在修改montage、mesh/capsule物理、movement mode和action之前拒绝启动；fresh PA_Als确认实际命名与默认实例存在。
  - Target：`state/transition.rs:359–370`只检查当前action不是Ragdolling，直接提交Ragdolling/None；`ragdoll_link.rs:787–830`随后关闭capsule collision并禁用Tnua，rig同步遇到缺失body只是continue，没有回退许可。
  - 反例：完整角色的physics body实例中分别移除pelvis或spine_03，再请求Ragdoll；源应拒绝，target却进入Ragdolling/None、capsule变Kinematic且layers=NONE。`bf144_ragdoll_lifecycle_requires_pelvis_and_spine_body_instances`4组2PASS/2FAIL，完整rig与只缺非必需foot_l的许可对照通过，实际图没有求值错误。旧入口测试默认完整PA实例，未覆盖源显式前置条件；生产未改。

- [ ] BF144-D66 / P2 — Ragdoll退出在动画实例不可用时提前恢复角色物理和模式。
  - Source：fresh `StopRagdollingImplementation:1071–1074`要求AnimationInstance有效且component-space pose非空，失败则保持Ragdoll；只有通过后才snapshot并恢复capsule/mesh、mode/action，最后尝试GetUp。
  - Target：`ragdoll_exit_route_system:671–704`仅在枚举自身碰撞体时可选读取player；player缺失仍清action、改Grounded/Air和velocity。之后restore parent/start_get_up虽然会因player缺失退出，但前面的物理/action提交已发生。
  - 反例：完整Ragdoll落地后令AnimationGraphPlayer不可用，再请求退出；source应保持Ragdolling，target已为None/Grounded、恢复capsule碰撞，却没有GetUp。`bf144_ragdoll_lifecycle_exit_requires_live_animation_instance`2组1PASS/1FAIL，正常player退出并成功GettingUp的对照通过。旧恢复测试始终保留可用动画实例；此项只登记退出前置条件，不把后续pose数值列为已审计。

- [ ] BF144-D67 / P3 — Jump恢复Ragdoll的请求与退出消费者未定序，可能晚一帧。
  - Source：fresh `AlsCharacterExample.cpp:156–163,189–194`的Jump与Ragdoll输入都同步调用StopRagdolling；有效落地恢复在该次调用内完成GettingUp和输入阻塞。
  - Target：Jump由`movement/motion.rs:3185–3187`写ragdoll_exit，而`ragdoll_exit_route_system`只排在input_state_request之后；两者都在state_transition之前，却没有彼此的依赖。Ragdoll键在更早的input_state_request写请求，因此没有同一缺口。
  - 反例：未调整调度的完整App曾同帧通过、后续又复现Jump仍Ragdolling/未阻塞且request pending，下一帧才GettingUp。固定当前合法的两种顺序后，direct/Ragdoll/Jump×顺序共6组5PASS/1FAIL，只有退出消费者先于Jump producer时延迟。`bf144_ragdoll_lifecycle_jump_and_ragdoll_input_exit_in_current_frame`，证据`20/jump_order/`；记录可复现的单帧调度差异，不靠增加等待帧让Gate通过。

- [ ] BF144-D68 / P2 — Ragdoll→GetUp未保留中间None动作触发的desired stance提交。
  - Source：fresh `StopRagdolling:1143–1163`先切地空mode，再把action置空，最后播放GetUp并置GettingUp。`NotifyLocomotionActionChanged:1091–1098`在置空时立即ApplyDesiredStance；`801–824`调用Crouch/UnCrouch设置CMC的bWantsToCrouch，随后GettingUp不会取消这个请求，当前CMC按`UpdateCharacterStateBeforeMovement:3500–3521`提交物理姿态。
  - Target：`get_up.rs:177–181`直接写GettingUp；`capsule_reshape_system:3299–3317`仅轮询当前action，GettingUp生效后不再消费Ragdoll期间积累的desired stance，未保留None阶段的源副作用。
  - 反例：实际Ragdoll退出前，Standing期望Crouching或反向，允许的GetUp先于capsule reshape顺序均保留旧姿态/半高0.9或0.56m；先让reshape看到None则正确应用。16组交接矩阵中此根因对应4处FAIL，包含rig碰撞已先清除的控制，排除D69干扰；相同期望姿态8组通过。`bf144_ragdoll_lifecycle_exit_applies_pending_desired_stance`，证据`20/stance_order/`；旧单动作姿态测试遗漏同帧None→GettingUp。

- [ ] BF144-D69 / P2 — Ragdoll退出的站立净空查询把自身rig当作障碍。
  - Source：fresh UE `UnCrouch:3391–3393`以CharacterOwner构造CapsuleParams，忽略整个本角色；Ragdoll的mesh仍属于该actor。ALS退出也先恢复mesh/capsule查询，再交给CMC执行待处理站立。
  - Target：`apply_capsule_for_stance:3375–3384`只排除actor实体，`character_blocks_collider:3840–3842`只识别同一capsule body；独立rig body没有被当作角色自身排除。`ragdoll_link_system`的rig layer停用与reshape未定序，旧rig仍可参与这次净空查询。
  - 反例：Crouching Ragdoll期望Standing，GettingUp尚未阻止reshape时，净空查询命中的阻挡全部是本角色thigh_l/r、pelvis、spine_01/02/03，导致仍为Crouching；只把rig layer停用排在查询前，阻挡为空并正确Standing。与D68同一16组矩阵中这是一处独立FAIL，反向缩小胶囊不受影响；证据包含实际阻挡归属。未修改普通Crouch实现或物理profile。

- [ ] BF144-D70 / P2 — 活动Ragdoll没有保留mesh对Pawn的Ignore响应。
  - Source：fresh B_Als_Character CharacterMesh的Pawn/Visibility/Vehicle为Ignore；`BodyInstance.cpp:4789–4819`把PA body响应与当前mesh响应取Min，`StartRagdollingImplementation`仅将mesh object改为PhysicsBody、启用QueryAndPhysics，Pawn仍Ignore。普通Pawn胶囊对该mesh的移动/净空查询以及物理接触都应忽略。
  - Target：`ragdoll_physics_profile.rs:373–376`把所有Enabled body转为Default↔Default，`ragdoll_link.rs:1214–1220`在活动时恢复这些层；`character_blocks_collider:3833–3844`接受另一个角色的rig，collision hook也没有Pawn响应过滤。
  - 反例：同一完整App生成两个普通角色，第一角色进入Ragdoll后，用第二角色实际胶囊和生产查询predicate在pelvis位置查询，17个实际shape命中被接受；21个非root shape均通过当前层与collision hook。非Ragdoll对照为0，地板在两组均阻挡。`bf144_pa_character_query_preserves_mesh_pawn_ignore`2组1PASS/1FAIL，证据`target/bf144/21/initial/`。此项是角色之间的channel响应，独立于D69忽略整个自身actor；未把尚未运行的双方接触轨迹称作已验证。

- [ ] BF144-D71 / P3 — Ragdoll惯量矩阵分解丢失缩小rig的非对角耦合项。
  - Source：fresh `BodyUtils::ComputeMassProperties`与Chaos `MassProperties.cpp:17–98`在kg·cm²中按相对off-diagonal/trace门槛分解；新增原生body probe读取实际principal/frame，不用闭式理想矩阵替代源数值。scale=0.2时spine_03/head/hand_l/r仍保留耦合。
  - Target：`ragdoll_physics_profile.rs:1424–1427`把kg·m²矩阵交给`AngularInertia::from_mat3_unchecked`；当前glam_matrix_extras 0.3.0 `symmetric_eigen3.rs:96`以绝对off-diagonal平方和<1e-10认定对角矩阵。这些缩小后的有效项被清零，随后该AngularInertia进入真实rig物理参数。
  - 反例：完整rig scale=0.2的head源Ixz=4.0892246e-6，target为0；以Ix x列量级的局部X角冲量复算，源角速度响应含z=−0.22336152，target z=0。10组native frame对照5PASS/5FAIL，只将实际body的principal/frame改用native转换后全部恢复。默认head另有同一分解入口的约1.43e-6矩阵误差，记录为数值偏差，不冒称已比较整段布娃娃轨迹。既有PA测试只测少数主轴或较大尺寸，未覆盖SI量级触发的绝对门槛。

- [ ] BF144-D72 / P3 — 初始极小uniform rig的质量属性绕过最终形状尺寸下限。
  - Source：fresh `FKSphylElem::GetFinalScaled`对半高/半径及圆柱长度施加源下限，再由实际Chaos形状计算质量、质心与惯量；原生InitBody+UpdateMassProperties验证scale=0.01的pelvis Ixx=1.2151629e-9kg·m²。
  - Target：`scaled_ragdoll_mass_properties:1297–1301`对uniform scale直接缩放未截断的原始质心/惯量，虽然`scaled_collider:123–126`已经将实际capsule圆柱长度截到1mm。目标pelvis Ixx=1.0713182e-9，约低11.8%；spine_03因各shape体积占比改变还保留了错误质心。
  - 反例：全新scale=0.01角色的21 body中18组不符，包含D71的分解影响；pelvis是无旋转单shape，独立证明本项，质量1g与实际shape下限均正确。从实际clamped Collider重新计算的Ixx与native相等，误差5.55e-17。source六种scale共126组102PASS/24FAIL，常规尺寸与无下限触发的root/foot提供对照。未将source自身在4个极小body上的数值分解偏差“修正”为理想公式。

- [ ] BF144-D73 / P3 — 已创建rig的小尺寸缩放错误复用初始化capsule轴长下限。
  - Source：fresh `BodyInstance::UpdateBodyScale:2402–2422`的live分支将capsule半轴下限设为FCollisionShape::MinCapsuleAxisHalfHeight，即1e-4cm；与初始化GetFinalScaled的圆柱长度0.1cm下限不同。原生live probe确认pelvis在scale=0.01时按半轴0.35mm计算惯量。
  - Target：`scaled_collider:123–126`在创建与更新两条路径都将圆柱全长截到1mm；live pelvis实际半轴0.5mm，比源0.35mm多0.15mm。该case的目标质量/惯量反而与native live相符，证明是形状更新边界，独立于D72的初始化质量属性。
  - 反例：完整角色的player/mesh从unit scale改为1/0.75/1.5/0.01，4组3PASS/1FAIL；源与target半径均1.4mm，最后一组只有实际几何轴长不等。`bf144_pa_live_scale_matches_source_body_geometry_and_mass`保留native质量控制与当前Collider取样，没有改尺寸阈值。

- [ ] BF144-D74 / P3 — mesh缩放到零后Ragdoll仍保持刚体模拟。
  - Source：fresh `SkeletalMeshComponentPhysics.cpp:1946–1949,3533–3544`在OnUpdateTransform调用UpdateRigidBodyScaling；relative scale跨到零时立即SetAllBodiesSimulatePhysics(false)，独立于world tick。p.UpdateRigidBodyScaling默认开启，当前项目/源配置没有覆盖。
  - Target：`ragdoll_positive_scale:1199–1206`把零scale回退为ONE，`sync_ragdoll_body_modes:445–451`仍依Ragdolling保持20个Dynamic body，没有源的零scale停用生命周期。
  - 反例：活动Ragdoll在world pause后将已detach的player/mesh scale设为0，下一完整App帧仍有20个Dynamic，源应为0；scale=1对照保留20个。`bf144_pa_zero_mesh_scale_stops_source_body_simulation`2组1PASS/1FAIL。只登记停用分支，恢复非零的source重建路径未在本反例中宣称已验收。

- [ ] BF144-D75 / P2 — Ragdoll角驱动在启动限速之后采样pelvis速度，过早降低关节刚度。
  - Source：fresh `AlsCharacter_Actions.cpp:883–885,946–958`先保存pelvis当前linear velocity，以其完整长度/1000cm/s截到[0,1]后乘25000设motor刚度，再递减启动期计数并调用`ConstraintRagdollSpeed`。`PhysInterface_Chaos.cpp:485–489`再应用本机native角drive stiffness scale=1.5，阻尼/最大torque均为0；加速度drive不做厘米平方转换。
  - Target：`ragdoll_link.rs:207–214`把`limit_initial_ragdoll_speed`排在`ragdoll_joint_drive_adapter_system`前；后者`578–584`回读已被截短的pelvis速度，再由`ragdoll_angular_drive.rs:82–88`写入19个活动joint的57个motor通道。
  - 反例：实际加载角色由静止启动Ragdoll，下一帧限速仍为2m/s；输入(3,4,0)m/s时目标基础刚度5000，源应12500，实际各motor为7500而源应18750；输入竖直−12m/s时目标仍5000，源应25000。零速、1m/s及限速期结束后同四种输入均PASS，8组6PASS/2FAIL。只证明真实关节刚度输入差异，不把未运行的完整布娃娃轨迹纳入证据。
  - 漏检：`joint_drive_uses_source_constraint_drive_values`只对纯speed→stiffness函数传0/5/10，未把启动限速和实际ECS消费者组合。反例/对照见`bf144_pa_joint_drive_reads_velocity_before_initial_speed_limit`与`target/bf144/22/`；未修改移植实现或转换器。

- [ ] BF144-D76 / P2 — Ragdoll硬角锁使用非线性角校正，偏离源cached四元数约束。
  - Source：fresh `PBDJointCachedSolverGaussSeidel.cpp:1459–1483,1566–1597`按初始frame的四元数Jacobian和相对quat分量锁轴，锁定路径没有角容差；`PBDJointConstraintUtilities.cpp:346–371`给出Jacobian，最终由`ChaosCore/Private/Rotation.cpp:158–168`将累计DQ以一阶quat增量归一化。该源算法的有限角误差也须保留。
  - Target：`source_physics_joint.rs:365–373`对Locked Twist仍取完整twist angle和child X轴；`203–217,575–589`只有Limited走cached增量，Locked每轮重新算当前角并用指数旋转；`531–533`还给硬锁加了0.001rad死区。
  - 反例：native clavicle_l锁Twist、其余轴无误差；两Dynamic等惯量body相差1rad，源两body各修正约0.533293rad，target各0.5rad，完整角色实际物理帧每body差0.033293rad。0.0005rad时源仍校正，target完全跳过。静态parent且普通角度、零误差是对照；1/6次迭代共20组10PASS/10FAIL，完整角色两组都FAIL。夹具只隔离point/contact和相邻joint，不改生产角算法，不把有限角源特性改为理想完全锁定。
  - 漏检：旧cached drive测试不触发硬Twist；一般角测试只核对方向/减小误差，没有独立源Jacobian和有限迭代终值。证据`bf144_pa_angle_locked_twist_matches_source_cached_quaternion_constraint`及`23/`。

  - BF144.25补充：fresh同一GetLockedRotationAxes/InitLockedRotationConstraint也用于swing锁与混合硬锁。全部15个native Locked轴×parent dynamic/static×0/0.5rad共60组40PASS/20FAIL，source固定quat Jacobian与target当前角度求解的差异覆盖twist及swing。lowerarm的两组mixed旋转×dynamic/static加identity控制共6组2PASS/4FAIL，最大角校正差0.104148rad；仍是D76的同一硬锁非cached根因，不重复编号。证据`bf144_pa_angular_native_locked_axes_use_source_cached_jacobian`与`bf144_pa_angular_mixed_hard_locks_use_source_linearized_corrections`，源参考独立复算，无native Chaos仿真声明。

- [ ] BF144-D77 / P3 — Ragdoll角drive与soft limit容差没有按实际dt缩放。
  - Source：fresh cached solver `Init:190–192`用`min(1,3600×dt²)`缩放AngleTolerance；该值同时控制`InitSwingTwistDrives:2612–2614`和soft limit的scalar/SIMD判断。native配置为0.001rad，120Hz实际阈值应为0.00025rad。
  - Target：`source_physics_joint.rs:296–300`直接以固定配置决定drive是否启用，`232,531–533`也把同一固定容差用于soft修正；没有dt缩放。
  - 反例：两Dynamic等惯量body、native spine_01、motor目标误差0.0005rad，120Hz源每body应动约0.000180636rad，target保持不动；soft twist超限0.0005rad时源每body应动0.000222639rad，target仍保持。完整角色60Hz两对照PASS、120Hz两反例FAIL；直接drive/soft共21组17PASS/4FAIL，240Hz同样失败。该项影响微小角修正，不冒充大幅姿态异常。
  - 漏检：旧solver用固定60Hz及远大于容差的角度；没有改变dt同时保持误差落在新旧阈值之间的反例。证据`bf144_pa_angle_motor_tolerance_uses_actual_time_step`、`bf144_pa_angle_soft_tolerance_uses_actual_time_step`及完整角色对照。

- [ ] BF144-D78 / P2 — 全Limited软角约束逐轴立即写回，偏离源SIMD同时计算。
  - Source：fresh `PBDRigidsEvolutionGBF.cpp:1388`把当前UseSimd=True传给linear solver；native spine等三轴Limited且soft时，cached solver `1115–1130,1600–1678`用同一个迭代起点计算三轴DeltaLambda，再合计各body的DQ。不是普通逐轴Gauss-Seidel写回。
  - Target：`source_physics_joint.rs:199–238`按X/Y/Z循环；每个轴的`apply_angular_correction`立即更新body delta_rotation，后轴看到前轴的新校正，非对角惯量耦合时改变了输入。
  - 反例：native spine_01限位和soft参数，正定耦合惯量、三轴初始旋转(0.9,0.7,0.5)rad；独立源同时计算与target在1/6次迭代后均差0.072376rad，完整角色相同输入差0.0723763rad。8组6PASS/2FAIL，去掉多轴或去掉耦合即PASS；完整角色单轴对照PASS。mass conditioning保持启用，所选等质量/惯量比使源conditioning不修改输入；这里只验证受控角求解，不宣称全rig自然摔倒轨迹。
  - 漏检：旧cached参考按target同样的逐轴顺序复算，且实际PA drive测试每次只激活一个角轴；未覆盖源SIMD soft与惯量耦合的组合。

- [ ] BF144-D79 / P3 — 混合Locked/Limited角约束遗漏源接近180°的退化保护。
  - Source：fresh cached solver `1157–1168,1183–1188`在两frame的Twist轴dot<−0.998时跳过不可分解的Limited twist及单Limited swing，仍处理Locked轴。native neck_01为Twist/Swing1 Limited、Swing2 Locked，纯Swing1接近π时Locked误差为0，源这一轮不修正。
  - Target：`source_physics_joint.rs:365–406,2267–2294`只在quat twist长度几乎为零时放弃，没有源dot阈值；接近π但不等于π时继续施加大幅soft修正。
  - 反例：源阈值两侧、2.8rad、π−0.03与π共5组3PASS/2FAIL；完整角色neck在π−0.03时target每body额外修正1.125131rad，源应0。精确π反而因目标自身零长度guard而PASS。罕见极端角度边界，不按普通neck姿态缺陷描述。
  - 漏检：只验证普通角度或精确180°会错过这一窄区间；证据`bf144_pa_angle_mixed_limits_preserve_source_degenerate_swing_guard`及完整角色物理消费者。

- [ ] BF144-D80 / P2 — Ragdoll锁定锚点与投影复用了不同的球形约束算法。
  - Source：fresh `PBDJointCachedSolverGaussSeidel.cpp:379–433,576–630,824–903`在三轴Locked/SIMD分支缓存parent connector轴、双body力臂和各轴有效质量，以当次共同DP/DQ同时算三轴lambda，再累加线性DP与角向DQ；`241–274,2168–2238`的projection使用同样三轴公式，parent固定、child球形惯量。`ConstraintGroupSolver.cpp:518–559`在projection前提交并清空原DP/DQ。
  - Target：`src/avian_ext/source_physics_joint.rs:241–243,2102–2158`在位置及projection调用Avian `SphericalJoint→PointConstraintShared::solve`；该实现沿总separation方向求一个有效质量，每轮再旋转力臂并乘增量quat，与源cached三轴算法不同。
  - 反例：实际加载角色并启动Ragdoll，在spine_01保留native双frame/COM、限位与conditioning，隔离其他joint/contact，输入相同unit mass、球形inverse inertia=25与双锚点偏移。6次位置阶段偏移(0.08,0.09,−0.10)m时，源/目标位置校正差0.000632m、角校正差0.001324rad；单次projection偏移(0.02,0.03,−0.04)m时分别差0.000914m/0.000824rad。`bf144_pa_point_full_character_position_and_projection`8组4PASS/4FAIL；零误差、projection单轴和超过5cm后纯teleport对照通过。直接生产solver12组中有力臂且多轴的1/6次求解2FAIL，移除力臂则PASS。证据`target/bf144/24/initial/`、`24/full_initial/`。这是相同迭代数的受控算法反例，不宣称执行native Chaos仿真或完整自然落地轨迹。
  - 漏检：旧point测试只验锚点最终接近，projection测试只验误差减少/大误差teleport，未按当前cached/SIMD源公式比较有限次三轴响应。只登记，不修改移植solver。

- [ ] BF144-D81 / P2 — 世界Ragdoll错误使用PhysicsAsset的RBAN迭代预算。
  - Source：fresh `PhysicsAsset.h:223–228`说明SolverSettings只用于RigidBody Anim Node；实际世界经`ChaosScene.cpp:88–95→PBDRigidsSolver.cpp:3692–3697`应用PhysicsSettings。此次native PhysicsSettings为8次position/2次velocity/1次projection；三项solver override CVar均−1。`BodyInstance.h:1102–1108`在bOverrideIterationCounts=False时返回−1，native mesh及PA body不覆盖；`IslandGroup.cpp:98–144`才回落世界配置。引擎未应用项目配置时的默认同样为8/2/1。
  - Target：`tools/import/gen_ragdoll.py:993–995`把PA的6/1/1写入target_solver；实际`source_physics_joint.rs:684–693,795–800`按每joint该字段循环，没有世界/island预算来源。`bf144_pa_point_default_budget_matches_world_solver_instead_of_rban`检查完整角色19个活动joint，全部为6/1/1，19FAIL。
  - 反例：完整角色spine_01在相同零预测旋转、native力臂及相同速度阶段输入下，1次实际线性速度结果与源2次相差0.006645m/s；仅把夹具预算改为2，生产速度solver与独立源公式完全吻合。证据`bf144_pa_point_velocity_uses_source_predicted_geometry_and_world_budget`的两组零角速度对照；并非仅比较未消费的配置字符串。
  - 漏检：旧配置测试把PA导出的6次直接当世界source oracle，没有沿实际SkeletalMesh world body→scene/island路径确认所有权。此前BF144.22核对的配置字段和BF144.23同迭代数反例仍有效，但字段一致不代表使用范围正确；只登记，不修改转换器或重生成资产。

- [ ] BF144-D82 / P2 — Ragdoll线性速度约束沿用积分前的关节轴和力臂。
  - Source：fresh cached solver `17–45,576–630`由本帧预测body P/Q构造connector及双力臂；`984–1057`的速度约束复用这些缓存，三轴响应都按预测姿态计算。
  - Target：`source_physics_joint.rs:168–185`在积分前prepare保存frames/arms；`735–763`的速度阶段直接读原值，没有应用已经捕获的pre_solve_rotation。角位置/drive的缓存路径已使用该旋转，但线性速度路径遗漏。
  - 反例：完整角色spine_01两body在本帧共同绕Z预测旋转0/0.5/1rad，速度阶段统一输入parent v=0、child v=(1,2,3)m/s、双方w=0，保持native frame/COM与相同有效惯量，并把双方都控制为2次以隔离D81。零旋转PASS；0.5/1rad时角速度结果最大差1.845389/3.767715rad/s，2FAIL。独立参考直接使用捕获的预测姿态作为共同输入，不把UE与Avian的自由旋转积分差异算入本项。证据`target/bf144/24/velocity/`及后续最终记录。
  - 漏检：旧约束速度测试只验证误差降低或没有帧内旋转，未分辨本帧预测几何与prepare时几何。只登记，不改变solver执行顺序。

- [ ] BF144-D83 / P2 — 硬角约束速度阶段使用过大的未对标冲量阈值。
  - Source：fresh cached solver `1681–1702,1732–1754`仅对已激活的非soft约束做速度修正；以源单位的累计lambda绝对值>UE_SMALL_NUMBER(1e−8)为条件。角约束lambda含惯量单位，厘米到米会带来平方缩放，且cached quaternion Jacobian另有轴长度；不能直接复用任意无量纲epsilon。
  - Target：`source_physics_joint.rs:775–777`对所有角限位统一用lambda<=1e−6跳过。即使位置阶段已经成功校正Locked twist，缩放后的较小惯量仍使其lambda落入该区间，速度阶段不再消除锁定轴转速。
  - 反例：完整角色保留正常PA质量、形状与body/joint conditioning；scale=1/0.75/0.5，clavicle_l/hand_l，初始twist=0/0.0011/0.01rad，隔离point/contact和drive。位置阶段有校正后，速度阶段给双方同一参考输入(parent w=0、child w=X)，源应消除相对twist；0.75手腕、0.5锁骨/手腕的0.0011rad共3FAIL，仍保留1rad/s相对转速，18组其余15PASS。三个实际target lambda分别8.7300e−7、8.1427e−7、2.3375e−7；它们对应的源lambda远高于1e−8。恢复unit scale或增大到0.01rad时激活正确；0角度本来就不应激活。证据`bf144_pa_point_hard_angular_velocity_uses_source_impulse_activation`及`target/bf144/24/hard_scaled/`。
  - 漏检：旧硬角速度测试只用0.5rad和unit inertia，位置lambda远离阈值；没有覆盖较小角误差与合法角色缩放。本项只判定锁定轴速度约束的激活，不宣称次轴向量的所有数值都已等价；仅登记。

- [ ] BF144-D84 / P2 — Ragdoll自由角积分使用指数旋转，偏离源一阶quat积分。
  - Source：fresh `PBDRigidsEvolutionGBF.cpp:870–978`实际动态body路径先更新/阻尼/clamp W，再调用`Rotation.cpp:158–173`，公式为normalize(Q + 0.5×dt×PureQuat(W)×Q)。当前native未启用物理substepping，gyro=False、角阻尼0；最大角速度使用项目3600度/秒，native全局Hack上限1000rad/s不影响该值。
  - Target：Ragdoll body使用Avian `integrator/mod.rs:503–532`的from_scaled_axis(W×dt)，每步精确指数旋转；项目未给这些动态body替换为源积分。默认6个子步的恒定角速度乘积仍是整帧指数旋转，不能与源一次normalized一阶积分互换。
  - 反例：实际加载角色并启动Ragdoll，spine_01保留native惯量/角阻尼/最大速度，隔离关节和碰撞，初始姿态含三轴旋转，W方向为normalize(0.2,0.8,−0.3)。dt=1/60、1/30及100ms帧×速度0/0.1/30/60/80rad/s共15组6PASS/9FAIL；所有速度clamp和物理dt检查通过。60rad/s、实际dt≈1/30时单步姿态差0.429204rad；80rad/s正确clamp后仍差0.477497rad。0/0.1rad/s控制通过，100ms只模拟约33.33ms，未混淆world/physics时钟。证据`bf144_pa_angular_free_body_uses_source_quaternion_integration`、`target/bf144/25/free/`。
  - 漏检：旧PA Gate核对参数、限位或缓慢静止姿态，没有把实际自由旋转积分与当前UE函数逐步比较。仅登记，不为看起来更稳定或更精确而替代source语义。

- [ ] BF144-D85 / P3 — Soft角阻尼直接使用W×dt，遗漏源预测quat差分。
  - Source：fresh cached solver `17–35`使用初始/预测connector quat，通过`Rotation.cpp:87–104`的2×((Q−R)×inverse(R)).xyz计算ConnectorWDts；soft标量路径`1547–1558`及SIMD路径`1634–1654`在该差分上加入本轮累计DQ，再形成阻尼项。对normalized一阶积分，这个差分是W×dt/sqrt(1+|W×dt/2|²)。
  - Target：`source_physics_joint.rs:551–565`虽正确减去已捕获的预测旋转以求约束校正，却把初始角运动直接写为(child.angular_velocity−parent.angular_velocity)×dt，改变了soft damping输入。
  - 反例：完整角色spine_01保留native 15°twist限位和K=5,000,000/D=5000，在积分后、约束前通过公开SolverBody输入统一设置为源预测quat，确认PreSolveDeltaRotation一致，以隔离D84。预测twist固定0.6rad、unit有效惯量，60/30Hz×W=0/6/−30/30/60rad/s的10组2PASS/8FAIL；W=60、30Hz时单body角修正差0.008454rad，60Hz差0.002960rad。零W通过；同输入的raw-W公式吻合目标，排除了fixture未隔离。证据`bf144_pa_angular_soft_damping_reads_source_predicted_quaternion_delta`、`target/bf144/25/soft_initial/`。
  - 补充控制：同一夹具按native限位设置相同0.3382rad超限量，加入neck_01的非SIMD scalar分支及D=0消融；两条路径24组8PASS/16FAIL，其中零W与4个移除阻尼控制全部PASS。记录`25/soft_final/`及最终Gate。
  - 漏检：之前soft fixture的初始角速度为0，此时两公式相同；仅核对D系数无法检查其消费语义。属于随W×dt增大的数值阻尼差异，不把它重复记为自由旋转积分缺陷；仅登记。

- [ ] BF144-D86 / P2 — Ragdoll沿用默认六子步，改变源单步物理的位移与求解时钟。
  - Source：fresh `PhysLevel.cpp:194–212`读取实际PhysicsSettings并传给`ChaosScene.cpp:331–359`；native bSubstepping=False、bTickPhysicsAsync=False，设置MaxSubSteps=1、dt=min(world dt,1/30)。`PhysScene_Chaos.cpp:543–549`把AsyncDt设为−1，`PhysicsSolverBase.cpp:581–619`不走固定/子步分支；`PBDRigidsSolver.cpp:563–591`只调用一次AdvanceOneTimeStep。动态body `PBDRigidsEvolutionGBF.cpp:915–977`先得到本步速度，再用V×dt更新位置。
  - Target：`playable_character.rs:30`使用默认PhysicsPlugins，`source_physics_timing.rs:112–119`只clamp整帧delta；Avian `solver/schedule.rs:187–209`默认SubstepCount(6)并执行六次完整SubstepSchedule。Ragdoll body没有源单步积分适配，同一帧的重力被分成六个速度增量与六次位移。
  - 反例：完整加载角色进入Ragdoll并度过初始限速，spine_01保留native mass/COM，隔离joint/contact、W=0、双方阻尼=0，初速X=1m/s，统一Bevy重力。60/30Hz与100ms帧×default六步/单步×有无重力12组9PASS/3FAIL；无重力恒速全部PASS，单步全部PASS。六步60Hz位移差1.134872mm，30Hz及100ms物理cap位移差4.541397mm，末速度仍相同；实际Time<Physics>和每次SubstepSchedule delta均已记录核对。证据`bf144_pa_linear_substeps_preserve_source_single_step_displacement`、`target/bf144/26/initial/`。
  - 漏检：旧physics timing测试验证了整帧cap与末速度，sleep测试只要求不同子步数保持同一整帧计数，没有比较source单步位移；不能由“都采用约33.33ms上限”推导调度等价。关节在不同dt下求解的整体自然轨迹尚未实测，本项不据此扩大数字结论；仅登记。

- [ ] BF144-D87 / P3 — Ragdoll线性阻尼公式及施加顺序与源不同。
  - Source：fresh `PBDRigidsEvolutionGBF.cpp:915–931`先加Acceleration×dt和LinearImpulseVelocity，再乘max(0,1−LinearDamping×dt)。21个native PA body的LinearDamping=0.01；`PerParticleEtherDrag.cpp:9–10`的全局覆盖本次读取为−1，未改写该公式。
  - Target：`ragdoll_physics_profile.rs:1361`把源系数直接交给Avian LinearDamping；Avian `integrator/mod.rs:248–249,362–368`先对旧速度乘1/(1+damping×dt)，随后加本步加速度增量，既改变系数公式也漏掉本步重力/外力的阻尼。
  - 反例：完整角色spine_01保留native mass，双方统一单步以隔离D86；60/30Hz×原生/零阻尼×静止、5/60m/s恒速、静止/移动重力及50N外力共24组17PASS/7FAIL。零阻尼12组全部PASS；native阻尼时两种步长的重力/外力6组FAIL，30Hz重力末速度应−0.326891m/s、实际−0.327m/s，差0.000108987m/s；30Hz无加速度60m/s另显现纯公式差。原生六子步+阻尼的6组3PASS/3FAIL证明真实默认消费者也受影响。证据`bf144_pa_linear_damping_uses_source_formula_and_update_order`、`bf144_pa_linear_native_default_free_fall_consumes_both_differences`。
  - 漏检：旧profile测试只核对阻尼字段，动力学夹具常显式把LinearDamping设为0，没有验证其执行阶段。本项是小量数值差异，未据此声称大幅视觉异常；仅登记。

- [ ] BF144-D88 / P3 — Ragdoll显式低速唤醒保留了旧睡眠计数。
  - Source：fresh `ChaosEngineInterface.cpp:626–640`写入外部速度，`SingleParticlePhysicsProxy.cpp:127–132`把该更新送到`PBDRigidsEvolutionGBF.cpp:2031–2057`；只要非零速度就调用WakeParticle，即使刚体已经清醒。`PBDRigidsEvolution.cpp:1258–1272`和`IslandManager.cpp:860–902`重置计数，连接岛还禁止当前步睡眠；后续按native counter=4重新累计。
  - Target：`ragdoll_physics_profile.rs:757–838`的SourceBodySleep只有状态/采样/阈值累计，没有消费外部速度写入或显式wake来清掉已有计数。Avian `islands/sleeping.rs:569–619`对速度变更的唤醒查询只处理已Sleeping的body，也不维护该独立source计数。
  - 反例：实际角色启动Ragdoll，选hand_l两端body，native sleep阈值/质量保持原值，隔离其他joint/contact，统一单步；先静止3帧，第4帧把两body的LinearVelocity显式写为0.005m/s。60/30Hz与孤立/连接岛四组全部在第5帧休眠，source应分别到第8/9帧。共同sleep输入的受控版本与移除该覆盖后的真实body积分版本均复现；全部24组16PASS/8FAIL，静止和线/角超阈值控制通过。证据`bf144_pa_sleep_isolated_and_connected_counters_and_explicit_velocity_wake`与`target/bf144/27/`。
  - 漏检：旧wake测试先让body已经Sleeping再移除Sleeping，恰好命中此前休眠时清零的计数；没有覆盖“仍清醒且已有累计进度”的外部低速写入。本项只声明该边界提前3/4个物理帧休眠，不声称普通大冲量也会触发；仅登记。

- [ ] BF144-D89 / P3 — Ragdoll外力遗漏源睡眠平滑速度的预置。
  - Source：fresh `SingleParticlePhysicsProxy.cpp:163–168`将外部Dynamics更新交给`PBDRigidsEvolutionGBF.cpp:1885–1890`；其`ResetVSmoothFromForces`在同名header `281–301`先以固定1/30秒预测V+Acceleration/30+Impulse，再用0.3混合进VSmooth。连接岛随后还会按实际位移再平滑一次，避免刚受力的body立即重新休眠。
  - Target：`ragdoll_physics_profile.rs:785–796`仅在求解结束后用当前位移更新smooth_linear，没有消费外部力/冲量的预置；Avian的force变更唤醒只维护自身睡眠状态，不能补齐该source平滑变量。
  - 反例：完整角色hand_l两body保留native mass、COM与sleep阈值，先静止3帧，第4帧按各自质量施加一次产生1m/s²的等向推力，第5帧撤去；不覆盖solver位移或速度。60Hz连接岛在第5帧休眠；source第4帧VSmooth≈0.011999m/s，超过0.01阈值，按独立公式连续检查12帧都不应休眠。第4帧实际速度约0.016667m/s已核验。30Hz连接岛及两种dt的孤立body控制通过，共4组3PASS/1FAIL；移除推力的静止控制按第5帧休眠。证据`bf144_pa_sleep_external_force_seeds_source_smoothed_velocity`。
  - 漏检：旧sleep夹具固定body速度或直接给solver位移，没有通过真实外力预处理阶段；本项与D88的显式速度写入计数重置不同，缺失的是force输入对平滑状态的更新。只声明这类轻推力边界，未声称普通大力同样失败；仅登记。

- [ ] BF144-D90 / P3 — 新物理岛错误继承孤立body的睡眠计数。
  - Source：fresh `IslandManager.cpp:1855–1903`在两个无岛节点新建连接时调用CreateIsland；`IslandManager.h:399`初始化SleepCounter=0，`IslandManager.cpp:2418–2444`将新岛从初始sleeping切为awake时再次清零。反方向销毁无约束岛时，`1731–1739`明确把岛计数复制给孤立body；这两种拓扑转换不对称。
  - Target：`ragdoll_physics_profile.rs:803–805,835–837`把每body计数的最小值当作当前岛计数，再统一回写，没有辨别新岛创建。两个已有3帧静止进度的孤立body连成新岛后，仍从3继续计数。
  - 反例：完整角色hand_l两body，native质量/COM/阈值保持，关闭其他关节和接触，静止3帧后第4帧启用实际hand_l关节；不覆盖solver采样。60/30Hz两组都在第5帧休眠，source新岛应第8帧；每步实际joint_count已核验。反向在第4帧断开原有连接的两组都正确在第5帧休眠，共4组2PASS/2FAIL。证据`bf144_pa_sleep_new_island_resets_counter_while_split_preserves_it`、`target/bf144/27/topology/`。
  - 漏检：旧测试覆盖断开后的独立睡眠，没有覆盖孤立状态已有进度再新建连接；只登记已复现的岛创建边界，不把所有合并/拆分情况一概判错。

- [ ] BF144-D91 / P2 — Ragdoll的原生回弹系数未到达子碰撞体接触。
  - Source：fresh PA全部21个body使用DefaultPhysicalMaterial，Restitution=0.3、Average且无override；UE `BodyInstance.cpp:4580–4613`给每个simple shape设置该材质，`PBDCollisionConstraints.cpp:469–470`组合双方系数。
  - Target：`ragdoll_physics_profile.rs:1365–1366`只在body设置Friction/Restitution；实际Avian 0.7 `collision/narrow_phase/system_param.rs:619–628`只取collider的Restitution，未像friction一样回退到body。真实PA子shape没有该组件，使用默认0。
  - 反例：完整角色head保留native质量、惯量与材质，以中心球/平面接触隔离几何和转动，两边源系数均0.3、入射1.5m/s。60/30Hz×1/默认6子步，实际组合0.15且回弹0.225m/s，源应0.3与0.45m/s；仅把已有body系数放到该shape后4组全部恢复，共8组4PASS/4FAIL。证据`bf144_pa_contact_native_body_restitution_reaches_child_shape`、`target/bf144/28/`。
  - 漏检：旧PA测试只断言body组件值，自定义CCD也会回退body材质，未核对实际普通接触manifold中的组合值。只登记，未修改依赖或生产材质适配。
  - 原生形体补证：不改21个body的形体/质量/关节/自然运动，在实际floor仅提供同一0.3材质；60/30Hz分别出现2/3个原生接触，全部组合成0.15。仅在PA子shape添加body现有0.3后全部变回0.3，共4组2PASS/2FAIL。`bf144_pa_contact_native_geometry_material_and_ccd_consumers`同时核对全部21个body的原生CCD关闭消费和无graph error。

- [ ] BF144-D92 / P3 — Ragdoll普通接触的低速回弹阈值与源物理时钟不符。
  - Source：fresh `PBDRigidsEvolutionGBF.h:70`与cpp:1365给实际世界碰撞容器传入1000cm/s²默认阈值；全局`p.CollisionRestitutionThreshold=-1`。`PBDCollisionContainerSolver.cpp:88–89,160–166`以该加速度乘物理dt作为速度门槛，即60Hz约0.166667m/s、30Hz约0.333333m/s。项目BounceThresholdVelocity=200字段没有进入这条Chaos世界调用。
  - Target：实际Avian `SolverConfig`默认固定1m/s，`solver/plugin.rs:639–640`仅乘length unit；角色插件没有覆盖该资源。
  - 反例：完整角色head中心接触，先统一双方shape系数0.3以隔离D91；60/30Hz×入射0.05/0.25/0.5/1.5m/s×原阈值/源阈值共16组13PASS/3FAIL。60Hz的0.25/0.5及30Hz的0.5被目标压到零速度，源应分别0.075/0.15/0.15m/s；仅替换阈值后三组恢复，低于双方阈值和高于双方阈值的控制通过。证据`bf144_pa_contact_restitution_uses_source_acceleration_threshold`、`target/bf144/28/`。
  - 漏检：旧回弹验证集中于自定义CCD或较大速度，未覆盖原生普通接触阈值及其dt语义。属于低速回弹差异，未将截图或整段跌倒观感当作证据。

- [ ] BF144-D93 / P2 — Ragdoll硬材质接触被目标的柔性接触参数延迟恢复。
  - Source：fresh native PA材质SoftCollisionMode=None、body非one-way且无穿透速度override，world初始穿透恢复−1映射无限制，pushout上限10m/s。`PBDCollisionConstraint.cpp:518–524`实际stiffness=1；`PBDCollisionSolver.h:454–466,593–644,1114–1138`按当前法向误差乘有效质量作位置修正，中心单点无转矩时一轮可清零误差。
  - Target：Avian `solver/plugin.rs:292–299,326–351`默认contact damping ratio=10/frequency factor=1.5，`contact/normal_part.rs:132–149`把所有普通穿透接触按softness缩放后推动。PA soft=None未关闭这条柔性规则，最终速度relax到零也不恢复残余位置误差。
  - 反例：完整角色head保留native mass/inertia，以中心单点和零速度/重力隔离其它差异；60/30Hz×1/默认6子步×0/1mm/1cm重叠×原参数/完整法向修正，共24组16PASS/8FAIL。1cm重叠时单步只恢复约1.890mm、默认六子步约3.561mm，源应全部恢复；均未触及源pushout上限。将bias设为1/substep、mass scale=1、impulse scale=0后恢复，零重叠控制通过。证据`bf144_pa_contact_hard_surface_resolves_source_normal_separation`、`target/bf144/28/`。
  - 漏检：旧接触测试检查有限值、碰撞阻挡或inertia conditioning相对变化，没有以同一法向误差核对native硬材质的绝对恢复量。这里只证明单点法向规则不同，不宣称已对齐复杂多接触Ragdoll轨迹；仅登记。

- [ ] BF144-D94 / P3 — Ragdoll切向摩擦缺少源独立的位置修正阶段，改变首接触滑移距离。
  - Source：fresh `PBDCollisionContainerSolver.cpp:318–336,851–860,900–904`和实际world/CVar证明8个位置迭代中的末4轮执行friction、2个速度迭代中的末1轮执行friction；位置stiffness=0.5、速度stiffness=1。`PBDCollisionSolver.h:472–590,668–744`分别累加位置/速度冲量并共享Coulomb限制；`ConstraintGroupSolver.cpp:468–484`在两阶段之间把DP/DQ写入速度。
  - Target：Avian `contact/mod.rs:289–350`在每子步速度constraint中求摩擦，再由修正后的速度积分位置；`contact/tangent_part.rs:187–241`没有独立位置摩擦累计量和源末4轮的位置收敛过程。
  - 反例：完整角色head、相同有效惯量/接触点/系数0.7，消除D93柔性法向及D95旋转法向影响；60/30Hz×native/单位惯量×系数0/0.7×切速0/0.1/1，共24组18PASS/6FAIL。native惯量、切速1m/s时，60Hz目标滑移7.84165mm、源8.39321mm；30Hz为15.68330mm、16.78642mm。最终切向速度和角速度一致，位移仍不同；移除摩擦、零切速及单位惯量触及Coulomb上限的控制通过。源零力臂独立闭式参考8组PASS。证据`bf144_pa_friction_preserves_source_position_and_velocity_stages`、`target/bf144/29/`。
  - 漏检：旧PA摩擦测试以组件系数或最终速度为判据，没有同时核对源位置摩擦阶段和实际滑移；这里只登记毫米级单接触差异，不以完整Ragdoll轨迹比较作结论。
  - 保留接触补证：source `PBDCollisionConstraint.h:804–812`保存静摩擦锚点，cpp:1129–1149,1250–1292匹配quadratic本地点；先静止接触、再只改速度的稳定锚点分支12组8PASS/4FAIL，ContactId保持且无graph error，30Hz切速1时位移差1.09792mm。该结果仍属同一D94位置阶段根因，不重复编号。

- [ ] BF144-D95 / P3 — 法向速度阶段仍按正间距预测，未保留源独立速度解算。
  - Source：fresh sphere/box采用one-shot manifold，源`PBDCollisionSolver.h:868–945`用cached力臂的线性DP/DQ计算位置误差，随后velocity solve沿该法线解算；中心接触的r×normal=0，切向摩擦旋转不应制造新的法向间距。
  - Target：Avian `contact/mod.rs:287–293`在Relax时用delta_rotation旋转r再计算separation；`normal_part.rs:129–131`对新出现的正间距进入预测接触分支，允许法向速度变成−separation/dt。
  - 反例：完整角色head原生惯量、法向恢复已统一、切速0/0.1/1m/s；60/30Hz×目标quat/源Euler quat/仅法向使用线性控制共18组10PASS/8FAIL。切速1时默认目标重新产生−0.0185323/−0.0370038m/s法向速度，源应0；换源quat仍为−0.0185108/−0.0368518，故不是D84的自由积分差异。只在Relax期间令法向使用原固定力臂、随后恢复原quat，全部恢复；零切速控制通过。证据`bf144_pa_friction_rotation_preserves_source_cached_normal_separation`、`target/bf144/29/`。
  - 漏检：旧接触验证未在切向摩擦导致旋转后复核法向速度，最终非穿透位置不能证明速度阶段一致。只登记，不更改接触缓存或生产求解器。
  - BF144.55补充边界：fresh `PBDCollisionSolver.h:978–992,1178–1236`在速度阶段只用V/W和目标法向速度，不再读取当前位置残差；Avian `normal_part.rs:129–132`在`use_bias=false`时仍对正间距加入separation/dt。共同原生Cart head、零摩擦、源8次线性位置投影后，actual wZ=−0.204293、source=−0.207578；只去掉该预测项即恢复，跳过零pushout点没有影响。Stairs_01 foot先隔离D139后，actual wZ=0.462137、source=0.403496，同样需要去掉预测项。60/30Hz分离控制均复现；两侧间距已统一为源线性DP/DQ，证明该边界不需要非线性旋转重算。旧BF144.29的中心球控制保持有效，但不能将“改用线性间距”推广为一般速度阶段修复。与本项共用同一预测分支，合并登记而不新增编号；证据`55/common_geometry/`、`55/final/`、`55/analysis.json`。

- [ ] BF144-D96 / P3 — Ragdoll接触缓存丢失源静摩擦锚点历史。
  - Source：fresh `PBDCollisionConstraint.h:804–838`在static ratio≈1时保留旧ShapeAnchorPoints、0<ratio<1时插值旧锚点与当前点、ratio≈0时单接触改用当前点；cpp:1129–1185,1250–1292按quadratic本地接触点匹配后恢复锚点。native恢复开关True，exact=2mm、near=10mm。`PBDCollisionContainerSolver.cpp:142–158`随后从两侧保存锚点的世界差计算切向误差，即使当前切向速度为零也会恢复。
  - Target：Avian `contact_types/mod.rs:426–470`匹配旧接触时只复制warm-start normal/tangent冲量；`narrow_phase/system_param.rs:788–799`每帧生成当前几何点，`contact/mod.rs:184–200`没有历史切向位移输入。
  - 反例：完整角色head先建立静止接触，保持ContactId，向已有缓存注入±1mm/5mm的合法近距离历史偏移；下一帧只给法向入射速度，当前切向V/W=0，统一hard法向且将旧warm-start冲量归零以隔离历史信息。目标重新生成零X锚点、X位移始终0；独立源位置参考应恢复约0.494/2.470mm。ratio=0.5时源恢复约0.247mm；60/30Hz共16组8PASS/8FAIL，零偏移、ratio=0、零摩擦和20mm超距控制通过。证据`bf144_pa_friction_history_restores_saved_tangent_anchor`、`target/bf144/30/`。
  - 区分D94：本例当前切向速度与当前几何切向误差均为零，单独补当前帧的位置摩擦公式仍得不到旧锚点偏移；需要保留源跨帧输入。位置oracle复用已验证源公式，将历史切向误差等价代入位置阶段并扣除合成自由位移，未使用其速度输出，也未执行native Chaos仿真。旧测试只核对warm-start或稳定零偏移接触，因此漏掉该边界。
  - BF144.63补充复杂mesh：原生30组保存/匹配覆盖4/6/8/14点与非零锚点插值。严格少于8点时零ratio仍保存当前接触；8点及以上则省略，混合ratio的存活点仍按同一规则匹配。保留真实ContactId并核对当帧准备数据后，固定已知几何/位置算法差异的48组中14处缺锚点失败，全部恢复控制通过；另4个零摩擦控制通过。Cart/Stairs foot的零ratio因不保存而通过，Cart head和Barrel foot仍有跨帧切向误差。该边界改变本项输入选择，不另增编号；`63/final.native.json`、`63/history_initial/`。

- [ ] BF144-D97 / P3 — Ragdoll回弹目标读取了施加重力前的接触速度。
  - Source：fresh `PBDRigidsEvolutionGBF.cpp:581,638,650,916–978`先积分V和P，再生成接触；`PBDCollisionContainerSolver.cpp:118–129,168–173`用此时Body.V/W求ContactVelocityNormal，回弹目标为其负值×restitution。
  - Target：Avian `collision/narrow_phase/system_param.rs:739–752`在physics narrow-phase保存point.normal_speed；`dynamics/solver/schedule.rs:37–40,61–66`随后才在substeps施加重力；`contact/mod.rs:374–388`仍用旧normal_speed构造回弹目标。
  - 反例：完整角色head单个中心接触、初始下落2m/s、共同重力9.81、native restitution=0.3；镜像材质到shape并用hard-normal排除D91/D93，速度均高于两侧阈值以排除D92。60Hz目标回弹0.60000m/s、源0.64905；30Hz为0.60000/0.69810。16组12PASS/4FAIL；原生六子步和单子步均失败，零重力及仅替换积分后normal_speed的控制全部PASS。证据`bf144_pa_contact_phase31_restitution_samples_post_force_velocity`、`target/bf144/31/gravity/`。
  - 漏检：旧回弹测试使用零重力或把帧前速度直接作为共同冲击输入；未覆盖narrow-phase与force积分的先后关系。默认角色body/shape均无SpeculativeMargin覆盖、NarrowPhaseConfig为MAX，排除有限margin测速夹断；生产未改。

- [ ] BF144-D98 / P2 — 子碰撞体同步晚于预测包围盒，Ragdoll漏掉当前帧新接触。
  - Source：fresh `PBDRigidsEvolutionGBF.cpp:581,638,650,974–1002`先按本帧积分后P/Q更新bounds，再检测；`CollisionOneShotManifoldsMiscShapes.cpp:80–95`及`ContactPointsMiscShapes.cpp:322–343`用Box.PhiWithNormal−sphere.radius构造实际重叠点，随后位置/速度solver当帧处理。
  - Target：Avian `collider_tree/update.rs:67–71,753–778`在Finalize之后、Last之前，用child Position/Rotation构造velocity-swept AABB；`src/als/character/character.rs:230–245`却到Last才同步这些child pose。Avian原生child同步也只在下一帧First执行。因此恒速运动时bounds覆盖上一段位移，narrow-phase发生在本帧位置积分前又无法补救。
  - 反例：完整角色head保留native质量/惯量，受控中心sphere半径0.1m对静止box，无gravity/friction/restitution/其他关节。连续运动到x=−0.4后，下一帧自由终点−0.15，源应在box左面停到−0.2且V=0；target未生成接触，仍为−0.15及7.5m/s（30Hz）或15m/s（60Hz）。16组12PASS/4FAIL，原生六子步及单步均失败。采到的AABB.max.x仍为−0.295，而提前同一pose同步后为−0.045，接触生成且位置/速度恢复；未到达障碍的控制通过。证据`bf144_pa_contact_phase31_detection_samples_the_integrated_shape_position`、`target/bf144/31/`。
  - 漏检：旧接触测试从已经重叠的几何开始、只检查帧末child pose，或使用body与shape同entity；没有串联child更新→预测bounds→下一帧检测。消融去掉历史AABB缓存和手工平移，仅保留fixture中的提前pose同步仍得到相同恢复，生产实现未改。该缺陷与D93已生成接触后的柔性恢复独立。

- [ ] BF144-D99 / P3 — 多点碰撞回弹仍使用默认单次迭代，遗漏源世界velocity预算。
  - Source：fresh `PBDCollisionContainerSolver.cpp:900–904`每次世界velocity迭代都处理collision manifold；`PBDCollisionSolver.h:1158–1245`在native AveragePointEnabled=False时逐点处理，两次迭代来自当前世界PhysicsSettings。`CollisionResolution.cpp:652–665→CollisionOneShotManifolds.cpp:1133–1200`的水平capsule/box分支生成两个端点。
  - Target：Avian `dynamics/solver/plugin.rs:288–299,629–670,710–718`使用独立SolverConfig.restitution_iterations，默认1且项目未接入世界collision预算；此字段与D81中SourcePhysicsJoint的RBAN数据没有调用关系。
  - 反例：完整角色native head capsule、mass/inertia/conditioning与两个实际受力端点；在回弹阶段提供共同零V/W、已激活normal冲量、restitution=0.3和同一normal/有效惯量。目标vY=0.6575385m/s，源两次参考0.6316152；目标角速度(x,z)=(-0.733396,-1.047396)，源=(-0.402972,-0.575503)。60/30Hz均失败，仅把回弹迭代改2恢复；另有相同isotropic惯量矩阵控制及单点/零回弹对照，共20组16PASS/4FAIL。证据`bf144_pa_contact_phase32_multi_point_velocity_uses_source_budget`、`bf144_pa_contact_phase32_native_inertia_uses_source_multi_point_budget`、`target/bf144/32/`。
  - 漏检：旧回弹验收只含单点，重复迭代天然不改变结果；旧joint预算测试未覆盖独立collision solver。这里比较同一组阶段输入的有限次求解，不将target生成的微小法向分量或完整自然落地轨迹视为源已等价；生产实现未改。

- [ ] BF144-D100 / P3 — Ragdoll接触点与约束组顺序不同，有限次求解改变角速度。
  - Source：本次native UpdateConstraint给水平head capsule/box生成的端点顺序为正X端→负X端；`PBDCollisionContainerSolver.cpp:289–301`按未disabled的原始顺序追加solver点，`PBDCollisionSolver.h:1166–1245`逐点求解，没有对这条primitive分支另行排序。
  - Target：Parry capsule/box的feature接触经Avian `contact_query.rs:225–255`保留返回顺序，实际为负X端→正X端，再附加不参与当前回弹的中点。点集接近不能自证有限迭代结果相同。
  - 反例：完整角色native head、共同零阶段V/W、native质量与同一有效惯量，两侧均用源2次速度迭代以排除D99。60Hz时目标角速度(x,z)=(-0.402973,-0.575504)rad/s，按native点序和同一惯量计算的源为(+0.402973,+0.575504)；线速度相同。60/30Hz×native/isotropic惯量×保留/交换端点共8组4PASS/4FAIL，只交换两个受力点的顺序即可恢复。证据`bf144_pa_contact_phase33_native_order_reaches_angular_response`、`target/bf144/33/contacts.native.json`及`33/order_initial/`。
  - 漏检：旧审计按无序点集或相同人为点序比较；BF144.32仅证明共同有序输入下的迭代差异，不能代替native点序验证。本项未修改几何生成或solver。

  - BF144.58补充网格入口：fresh gather仍按原生点序，target `solver/plugin.rs:545–575`先overflow再遍历各颜色与manifold，组内按生成点序。已分类几何的18姿态在两种摩擦下共72组50PASS/22FAIL，失败均为同11姿态的自然序；固定位置投影/push、自由V/W、点集及全部已知阶段控制，仅恢复源序全部通过。Stairs02 foot实际遍历[5,4,0,3,1,2,6,7]，60Hz无摩擦vY=−0.640518、wZ=−0.815022，source为−0.677159、+0.410893。Fountain head再次出现端点反序导致角速度反号。source-order基线与.57逐字一致，见`58/final/normal/`、`58/final/friction/`；同根补充，不新增编号。

  - BF144.61补充compound入口：30姿态的源序/自然序×两种摩擦120组104PASS/16FAIL，同八姿态在自然序下失败；源序基线与.59逐字一致。Arch01 hull 2 foot的actual visit=[3,4,5,6,2,1,0]，共同位置输入后60Hz无摩擦wZ=−1.739168，source=+0.667201；只恢复点序即通过。原生32组container顺序已核对，24个多manifold保持各自身份；三姿态30Hz四组合保持相同分类。与组阶段组成完整240组，组合失败均由D100/D137解释，见`61/final/`，不新增编号。

- [ ] BF144-D101 / P3 — 近水平胶囊/箱体接触法向与支持点未保留源规则。
  - Source：fresh `CollisionOneShotManifolds.cpp:1133–1196`在abs(axis·normal)<0.01时使用垂直胶囊轴的CylinderPlaneNormal，并沿此法向生成/投影两个裁剪端点。native head倾斜0.5度时，转换后的normal为(0.00585553,0.99997443,−0.00410024)，支持点相较目标偏约0.64mm。
  - Target：Parry `contact_manifolds_cuboid_capsule.rs:65–181`按SAT选择box/segment特征，再沿选中法向加radius；当前同输入法向近似竖直，并在实际求解中留下第三个受力点。
  - 反例：完整角色head采用该native姿态、共同isotropic惯量和native mass；先匹配源端点顺序、2轮迭代以排除D100/D99，阶段输入V/W为零。源回弹应产生水平速度(x,z)=(0.00361210,−0.00252931)m/s，目标约为(−0.00000238,−0.00000243)。60/30Hz共4组2PASS/2FAIL；在constraint准备前仅替换为native点/法向/深度，实际速度与独立源参考恢复。证据`bf144_pa_contact_phase33_native_near_horizontal_geometry_reaches_response`和`target/bf144/33/`。
  - 漏检：旧用竖直normal的单点测试或直接把目标manifold当作共同source输入，没有执行native接触生成；本项只确认近水平分支的运动影响，其余倾角/形体的原生数据差异继续逐项追消费者。

- [ ] BF144-D102 / P3 — 接触力臂固定取中点，未保留源质量加权位置。
  - Source：fresh `PBDCollisionContainerSolver.cpp:95–103`以两侧inverse mass加权世界接触点，再减各自COM得到solver力臂；动态body对静态环境时直接使用动态shape表面点。`PBDCollisionSolver.h:668–731`由该力臂计算摩擦有效质量和角速度响应。
  - Target：Avian `collision/collider/parry/contact_query.rs:243–251`始终取两侧surface point的中点；`dynamics/solver/contact/mod.rs:312–351`和`tangent_part.rs:207–240`把该中点力臂用于摩擦。两侧点差平行normal时，中点位移不影响normal力矩，但会改变tangent力矩；参考面投影方向与normal不平行时，法向也必须使用源质量加权force point（BF144.50已补充此边界）。
  - 反例：完整角色head保留实际质量/惯量，受控中心球对静态box；从实际准备好的contact constraint拷贝进入公开速度solve，固定相同V=(1,−1,0)、W=0与normal累计量，隔离D94/D95的位置阶段。半径0.125m、1cm重叠、friction=0.7时，目标切向速度0.50834465m/s、角速度z=−4.2362094rad/s，source为0.52872527与−4.2298017。60/30Hz×零/1mm/1cm重叠×零/0.7摩擦×原点/源加权点共24组20PASS/4FAIL；只在constraint准备前平移两侧anchor到源加权位置后恢复。证据`bf144_pa_contact_phase34_friction_uses_mass_weighted_contact_position`、`target/bf144/34/weighted/`。
  - 漏检：旧测试把目标接触点直接作为共同oracle输入，未比较两引擎gather的质量语义；这里是实际consumer的受控单阶段证明，不声称完整自然Ragdoll轨迹已对齐。零重叠与零摩擦均通过；生产代码未改。

  - BF144.56补证：Fountain foot及Rock head/foot两种缩放的五个trimesh摩擦样本，仅把实际Parry表面点按源动态质量归属传给prepared约束即恢复；原始点/法向/深度集合和逐点资格保持不变，零摩擦对照通过。其余几何原因另列，不把完整native几何替换当作本项单因素证据；见`56/initial_normal/`、`56/initial_friction/`。
  - BF144.59补证：30个完整convex/compound姿态中，13处摩擦失败仅恢复动态表面力臂后通过；对应法向对照均通过，点数和接触几何输入保留。六矩阵180组在精简重编后逐行不变，其他16处表面摩擦失败另行分类；见`59/final/{raw_friction,surface_friction}/`。

- [ ] BF144-D103 / P2 — 碰撞与关节分阶段求解，遗漏源每轮交替约束。
  - Source：fresh `PBDRigidsEvolutionGBF.cpp:1411–1415`先注册collision再注册joint；`PBDRigidsEvolution.h:359–365`按添加次序分配container id，`ConstraintGroupSolver.cpp:55–70`对相同native priority=0稳定排序。位置和速度各自每轮执行collision→joint（cpp:421–435、501–516；并行路径通过prerequisite保持相同次序）。当前世界为8个位置、2个速度迭代。
  - Target：Avian `solver/schedule.rs:34–40,61–66`在Substep后单独执行Restitution；`source_physics_joint.rs:2160–2181`把关节位置与速度放在Substep，回弹后无joint velocity约束，位置阶段也没有逐轮交替contact和joint。各自增加D81/D99对应预算不能补齐这一关系。
  - 反例：完整角色的pelvis/spine_01保持实际质量与启用的linear lock，中心frame/单球接触隔离转矩，关节预算8/2且关闭projection，使用hard contact排除D93。共同回弹阶段V/W=0、输入normal_speed=−2、restitution=0.3时，目标父body速度0、子body0.6m/s；源两轮交替均应0.41612768m/s。60/30Hz×1/6子步及移除contact/joint、零回弹、显式交替控制共32组28PASS/4FAIL；交替同一实际collision消费者和中心点速度投影后恢复。证据`bf144_pa_contact_phase35_restitution_interleaves_source_joint_velocity`、`target/bf144/35/velocity/`。
  - 位置补证：同一受控两body、零初速/重力/回弹、单子步且不覆盖solver位置；目标两COM的y=9.997679m，源8轮位置参考为10.000947m，少恢复约3.268mm。60/30Hz及移除contact/joint共6组4PASS/2FAIL。证据`bf144_pa_contact_phase35_position_iterations_alternate_contact_and_joint`；旧Gate把joint和contact分开验证，未覆盖共同约束岛的交替。仅登记，未修改生产调度或声称执行native Chaos动态仿真。

- [ ] BF144-D104 / P3 — 箱体接触每帧重建，丢失源的manifold几何复用。
  - Source：fresh `CollisionUtil.h:71`允许box几何复用、排除sphere/capsule；native AllowManifoldUpdate=1。`ParticlePairMidPhase.cpp:500–551`对上一帧使用过的constraint先TryRestoreManifold；`PBDCollisionConstraint.cpp:807–922`在shape位移/旋转与lateral误差容差内保留body0原surface point，并将另一侧投影到当前接触面。该几何复用独立于saved friction anchor。
  - Target：Avian `collision/collider/parry/contact_query.rs:174–189`每次创建空manifold/workspace并清除旧点；`narrow_phase/system_param.rs:789–799`只能复制旧impulse。旧ContactId仍存在也不能保留源接触几何。
  - 反例：实际PA foot_l箱体45度姿态对10cm宽箱体支撑，连续两帧沿支撑边缘平移±2mm；native API确认复用并移动旧接触点，fresh几何点仍在静态支撑边缘。完整角色保留ContactId、native mass、共同isotropic惯量与零摩擦，统一已有点序/2轮回弹并给共同阶段V/W=0、normal_speed=−2；源角速度x≈0.338012rad/s，目标+2mm为0.352183、−2mm为0.325092。60/30Hz×窄/宽面×0/±2/6mm×原几何/源复用点共32组28PASS/4FAIL，仅替换native复用点即恢复。证据`bf144_pa_contact_phase36_restored_box_geometry_reaches_response`及`target/bf144/36/`。
  - 漏检：旧历史测试只验证friction anchor或warm-start，不对比同一ContactId下的几何点；本例摩擦0，因而不属于D96。零移动、10m宽面和6mm超出复用容差的控制通过；原始资产及生产实现未改，未声称native动态仿真已执行。

  - BF144.62补充convex/trimesh：54个convex leaf与18个trimesh姿态、静止/2mm/6mm/1度yaw共288组native确认36个box允许复用，head全部排除；小平移与旋转分别有27/28个leaf不同于重建几何。24个完整foot姿态保留实际ContactId，但目标点仍匹配冷查询；固定既有静态算法差异后的48组33PASS/15FAIL，只恢复原生保留点全部通过。另48组当前完整聚合确认无新增/消失hull，未漏掉相邻约束。
  - 直接边界与消融：Bench foot的原始target几何无需替换，2mm移动后wZ约−0.28437rad/s，源保留点为0；目标与原生重建响应在既有容差内一致，旧点控制恢复。两帧长×四运动×原始/重建/保留的24组16PASS/8FAIL，静止和6mm全PASS；Stairs02两帧长及两种摩擦的32组24PASS/8FAIL同根复现。每帧重配导致ContactId改变的消融被拒绝，按源码字节恢复并在同target重编通过；104组响应在删除冗余记录后逐字不变。证据`62/native_analysis.json`、`62/final/`、`62/lifecycle_ablation/`；仅补充D104，未修复生产，saved friction及冲量历史另验。

- [ ] BF144-D105 / P2 — 启动帧先按旧Kinematic图配置重算速度，覆盖应保留的刚体初速。
  - Source：fresh `AlsCharacter_Actions.cpp:792–807`在Start中立即SetSimulatePhysics/ResetAllBodies，再读取pelvis速度；`ChaosEngineInterface.cpp:494–542`转Dynamic保留V/W，`SingleParticlePhysicsProxy.h:1246–1269`亦只在离开Dynamic时清速度。`PhysAnim.cpp:653–655,697–701`普通骨骼更新仅向仍为kinematic的body写新动画目标。
  - Target：角色在`character_runtime.rs:104–128`标为PostPhysicsAnimation，`post_physics_animation.rs:54–59`到physics后才求新图；依赖的`physics_systems_avian.rs:134–169,34–58`却在PrePhysics使用旧ragdoll_config设Kinematic并重算V/W，`ragdoll_link.rs:207–214`之后的handoff才强制Dynamic。已写入的Ragdolling动作不能阻止这一次旧配置消费。
  - 反例：完整角色通过Ragdoll输入进入，pelvis既有V=(1,0,0)m/s、W=0；在实际kinematic目标producer处给下一目标0.5m/s位移和0.15rad旋转。60Hz进入handoff时V被改为0.5、Wy=9rad/s，30Hz为0.5、4.5，源均应保留1、0。60/30Hz×匹配/改变下一目标×原顺序/提前Dynamic共8组6PASS/2FAIL，仅提前模式切换即恢复。证据`bf144_ragdoll_phase37_entry_preserves_previous_body_velocity`、`37/final/`；这是受控动画目标的真实入口consumer，不宣称自然布娃娃轨迹已对齐。
  - 漏检：旧启动测试核对最终Dynamic数量，速度测试在PrePhysics图运行后重新注入V/W，掩盖更早的覆盖。D75是随后驱动力测速顺序，不能解释本项。未改图、调度或依赖源码。

- [ ] BF144-D106 / P3 — 动画刚体角速度用轴角计算，偏离源四元数差分。
  - Source：fresh `PBDRigidsEvolutionGBF.cpp:1201–1243`对kinematic目标调用CalculateAngularVelocity；`ChaosCore/Private/Rotation.cpp:107–126`先保持最短弧，再取`2*((q1-q0)/dt*conjugate(q0)).xyz`。同轴有限角应为`2*sin(angle/2)/dt`。
  - Target：`ragdoll_link.rs:529–536`的动画body同步及依赖`physics_systems_avian.rs:88–95`使用normalized差值quat的scaled-axis/dt。该差异发生在生成W时，独立于D84的动态旋转积分及D85的soft阻尼。
  - 反例：完整角色pelvis从共同旧pose到旋转0.6rad的新动画bone pose，经过实际sync与下一帧Dynamic handoff；夹具提前Dynamic排除D105。60Hz目标Wy=36rad/s、源35.46243；30Hz为18与17.731215。零旋转通过，仅改为源quat差分后恢复；长帧中的独立时钟差异归D107。联合24组15PASS/9FAIL，证据`bf144_ragdoll_phase37_animated_velocity_reaches_dynamic_handoff`、`37/final/`。

- [ ] BF144-D107 / P2 — 动画刚体速度用全帧时间，未保留源physics delta clamp。
  - Source：fresh `PhysAnim.cpp:697–701`把骨骼pose写为kinematic target；`ChaosScene.cpp:331–357`在当前同步、无substepping配置下使用min(frame delta, MaxPhysicsDeltaTime)，原生上限为1/30秒；`PBDRigidsEvolutionGBF.cpp:1225–1243`用该物理步长生成V/W。
  - Target：`ragdoll_link.rs:526–537`和依赖的kinematic速度producer均直接使用全帧`Time.delta_secs()`。物理积分本身的clamp不会回头修正已生成的速度。
  - 反例：共同pelvis位移1cm、帧长0.1s、零旋转时，源Vx=0.3m/s，实际sync及隔离D105后的handoff均为0.1；只改速度分母为物理delta后恢复。60/30Hz对照通过；0.6rad的长帧组同时需要D106公式与本项分母，两项单独替换各自仍保留另一失败。沿用同一24组证据，不另计样本。源/目标重力约定没有改变，也未把全局帧率改成固定30Hz。

- [ ] BF144-D108 / P3 — 本地输入启动Ragdoll后，初期限速多保持一帧。
  - Source：fresh `PlayerController.cpp:2326,5494`在controller tick处理输入，`Controller.cpp:494–515`与`MovementComponent.cpp:184–188`保证controller→movement→actor顺序。Ragdoll输入立即Start，`AlsCharacter_Actions.cpp:820–823`先设8次刷新预算并立即限速；同帧`AlsCharacter.cpp:294`调用Refresh，cpp:955–959消耗第一次。故启动帧与其后7帧受限，第9个显示帧不再限速。
  - Target：`AlsRagdollInitialSpeedLimit::new:78–79`把预算设为8+1，实际PostUpdate每帧只调用一次；将立即调用和当帧Refresh合并后仍保留9次，导致第9个显示帧继续clamp。
  - 反例：完整角色经Ragdoll输入启动，首帧V=1m/s，随后每帧在实际限速consumer前给V=3；entry speed=0时第9帧目标仍为2、源应为3。60/30Hz×entry speed=0/5×11帧共44组42PASS/2FAIL，仅低入口速度的第9帧失败；前8帧、之后帧以及较高入口speed limit控制均通过。证据`bf144_ragdoll_phase37_input_frame_consumes_first_refresh_budget`、`37/final/`。旧Gate只检查总调用预算和限速数值，没有把本地输入与actor当帧Refresh组合。

- [ ] BF144-D109 / P2 — Ragdoll actor跟随采样晚于源阶段，提前使用本帧physics结果。
  - Source：fresh B_Als_Character的PrimaryActorTick/EndTickGroup均TG_PrePhysics；`AlsCharacter.cpp:294`在该Tick调用RefreshRagdolling，`AlsCharacter_Actions.cpp:880–901`读取上次完成的pelvis位置并立即SetActorLocation。相应mesh tick依赖actor，不能将actor跟随改成physics后采样。
  - Target：`ragdoll_link.rs:219–224,1115–1151`在PhysicsSystems::Last之后读取当前pelvis，随后apply跟随写actor位置。目标因此少了一次源规定的采样延迟。
  - 反例：完整Ragdoll在空中、零重力/阻尼并排除joint/contact，pelvis从(0,10,0)以1m/s水平运动。60/30Hz首帧目标actor.x为0.0166667/0.0333333m，源应仍为0；只把实际follow consumer的输入恢复为physics前位置即通过，静止控制亦通过。8组6PASS/2FAIL，证据`bf144_ragdoll_phase38_follow_samples_pelvis_before_physics`、`38/clock_final/`。这是actor运动时点差异，未扩展相机表现。

- [ ] BF144-D110 / P3 — 世界暂停时Ragdoll仍随外部pelvis位置移动。
  - Source：fresh角色bTickEvenWhenPaused=False，UE暂停Tick路径不调用其RefreshRagdolling；mesh已detach，外部移动pelvis不会自行移动capsule actor。
  - Target：`ragdoll_ground_follow`与`apply_ragdoll_ground_follow`没有世界暂停门槛，Time.delta=0时仍刷新并写actor位置。把它移到physics前仍不能补齐这个执行条件。
  - 反例：活动Ragdoll和actor均置于(0,10,0)，world pause后外部把pelvis横移0.1m；目标actor也移动0.1m，源应保持原位。暂停/正常×移动/不移动共4组3PASS/1FAIL；实际delta=0已断言。证据`bf144_ragdoll_phase38_follow_respects_world_pause`、`38/clock_final/`。D30覆盖移动基座，本项是独立Ragdoll跟随consumer；既有驱动力暂停检查未覆盖它。

- [ ] BF144-D111 / P2 — 恢复朝向的低精度分解在边界选反起身方向与yaw。
  - Source：`AlsCharacter_Actions.cpp:1080–1104`使用FQuat4d::Rotator的Roll，经UnwindDegrees后<=0选朝上，yaw相应减180度。fresh `UnrealMath.cpp:690–722`的double singularity门槛为0.4999995，pole分支强制Roll=0。
  - Target：`ragdoll_recovery_rotation:922–938`以f32骨骼基向量重新计算roll和pole条件；`restore_ragdoll_player_parent:984–1046`据此写actor yaw及GetUp选择。近零roll的舍入与接近竖直时的分支可能不同于源，旧测试只有±0.7roll及精确pole。
  - 反例：同一世界姿态换回源坐标后，native原生输出分别为Roll=+0.000416479°、pole Pitch=−90°/Roll=0、Roll=+1.2259694e−7°；完整退出流程的facing在三组均相反，actor yaw相差约π，实际AlsGetUpState也选择相反方向。99组96PASS/3FAIL，原生99个Rotator逐项验证参考。证据`bf144_ragdoll_phase38_recovery_rotation_uses_source_rotator`、`38/recovery_final/`、`38/queries.native.json`。保持源的零点判据，未添加容差或平滑。

- [ ] BF144-D112 / P3 — 球体sweep的接触时间数值偏差使Ragdoll地面定位提前。
  - Source：fresh `SceneQuery.cpp:378–380`的Single Sweep开启MTD，经`SQVisitor.h:336`调用公开SweepQuery；读取器使用相同shape adapter、transform与MTD输入直接取原生结果。`CollisionConversions.cpp:369–376`将time换回sphere center，ALS再加H−R+MIN_FLOOR_DIST。
  - Target：RagdollSpatialQuery只增加过滤，数值由Avian/Parry提供；`shape_cast_support_map_support_map.rs:21–28`走directional_distance，`gjk.rs:674–722`可在距离容差或不再收紧界限时返回当前TOI。该核与当前源查询结果不等，未把具体某个提前退出分支冒称为已动态插桩确认。
  - 反例：半径0.3m球从y=1m向下扫10×1×10m的30度box斜面；原生distance=0.653589859m，源站姿actor.y约0.9654101m，实际目标为0.9655699m，提前约0.16mm。Standing/Crouching×unit/0.75/非均匀scale×平面/30°/60°/空中/零点回退/初始重叠/空碰撞共42组36PASS/6FAIL，仅30度六组失败；36个有几何的原生查询逐项复核，去掉斜面条件即失去信号。证据`bf144_ragdoll_phase38_ground_trace_geometry_controls`、`38/geometry_final/`及`38/queries.native.json`。只确认本批sphere/box，U01的capsule数值边界不据此直接归因或关闭。

- [ ] BF144-D113 / P2 — 布娃娃退出时mesh默认偏移错误地乘以actor缩放。
  - Source：fresh `AlsCharacter_Actions.cpp:1110–1113`使用ActorTransform.TransformPositionNoScale(BaseTranslationOffset)定位mesh，再KeepWorld挂回capsule；`Character.cpp:192–195,504–527`维护默认/蹲姿offset。native角色mesh默认Z=−92cm、yaw=−90°、scale=1。
  - Target：`ragdoll_link.rs:992–996,1037–1041`重组actor、父级和进入时保存的local transform，位置offset随actor scale相乘；单位scale下相同掩盖了差异。
  - 反例：完整角色scale=0.75时，站姿恢复mesh比source高0.23m、蹲姿高0.145m；直接actor和带旋转/均匀缩放父级均复现。scale=1四组对照通过，非均匀scale.y=0.8也产生0.184/0.116m偏差。证据`bf144_ragdoll_phase39_mesh_snapshot_transforms`、`target/bf144/39/final/`；旧parent recovery测试保持actor最终世界scale=1，只检查pelvis保持，没有核对源mesh偏移。

- [ ] BF144-D114 / P2 — 布娃娃期间改变actor缩放会在退出时错误改变mesh世界缩放。
  - Source：fresh Start在`AlsCharacter_Actions.cpp:784`按KeepWorld脱离；Stop调用的`SceneComponent.cpp:2065–2099`只写位置和旋转，不改scale，随后KeepWorld挂接继续保留脱离mesh的世界scale。
  - Target：`restore_ragdoll_player_parent:992–996,1037–1041`使用当前actor scale与旧local scale重建mesh，缺少源的世界scale保留语义。
  - 反例：进入时actor/mesh scale均1，活动Ragdoll期间只把actor scale改为1.25，退出mesh scale变为1.25，source应保持1；站/蹲、直接/父级四组均复现。原始0.75及非均匀scale也复现，共12个改变scale样本失败，12个不改变scale的控制通过。证据同D113；该判据独立于位置offset及pelvis跟随，旧退出测试未覆盖脱离期间改变actor scale。

- [ ] BF144-D115 / P2 — 非均匀mesh缩放在物理姿态回读中被根骨倒数scale抵消，并进入最终快照。
  - Source：fresh `PhysAnim.cpp:200–206,339–345,355–363`将component scale用于物理位置换算，却保留输入动画的骨骼scale；kinematic root在当前bUpdateMeshWhenKinematic=False时保留动画值。native SKM_Als根骨与Flail的0/0.1/0.5秒根轨道均scale=1。
  - Target：依赖`read_pose_avian.rs:43–66`从无scale的刚体Isometry回写局部Transform；`post_physics_animation.rs:142–145`只补偿uniform mesh，非均匀scale直接跳过。真实root被写入mesh scale倒数，`restore_ragdoll_player_parent`又将该错误值复制进final pose。
  - 反例：完整角色mesh scale=(1.4,0.8,0.6)，实际readback前root scale=(1,1,1)，之后为(0.7142857,1.25,1.6666666)，抵消mesh缩放。unit/0.75/1.5/非均匀×开启/关闭readback共8组7PASS/1FAIL；同一非均匀角色关闭readback即保持1，独立于D113/D114的退出变换。证据`bf144_ragdoll_phase39_nonuniform_readback_preserves_source_root_scale`、`39/readback_initial/`与`39/final/`；退出快照矩阵中8个非均匀样本也保留该root值。
  - 漏检：既有`scaled_readback_preserves_physics_world_transforms_across_animated_gaps`只枚举uniform 2/0.5/1，覆盖分支之外的非均匀scale未进入合同。仅登记，不改变回读插件或归一化实现。

- [ ] BF144-D116 / P3 — mesh从近零缩放恢复时未重建物理状态，保留旧刚体速度。
  - Source：fresh `SkeletalMeshComponentPhysics.cpp:3533–3550`以uniform relative scale的IsNearlyZero（每轴<=1e−4）识别恢复并RecreatePhysicsState；`ActorComponent.cpp:2594–2604`销毁/重建，`TermArticulated:1195–1242`删除body和constraint，`InitDynamicProperties_AssumesLocked:4690–4708`取actor速度，`ParticleHandle.h:99–103`初始化角速度0。ALS此前SetSimulatePhysics(True)的component级旗标保留，恢复为20个Dynamic+1个Kinematic。
  - Target：`ragdoll_physics_profile.rs:1199–1268`把零scale回退到ONE，只调整已有rig；恢复没有对应重建/速度初始化，`sync_ragdoll_body_modes`仅恢复原有body mode。
  - 反例：world pause，mesh经零点恢复1/0.75/1.5，零期间把pelvis速度设为(0.3,0.2,−0.1)m/s、角速度(0.2,−0.4,0.3)rad/s；actor速度0，source恢复后两者应0，目标完整保留。24组18PASS/6FAIL，先补上D74的Kinematic停用/清零后再写入仍失败；source setter允许Kinematic刚体写入，该对照不依赖“零点没停模拟”。0/5e−5/1e−4的恢复失败，2e−4与未过零控制通过。证据`bf144_ragdoll_phase40_zero_recovery_recreates_velocity_state`、`bf144_ragdoll_phase40_near_zero_threshold_controls`及`40/final/`。

- [ ] BF144-D117 / P2 — 活动rig缩放以世界原点缩放刚体位置，偏离mesh原点和当前骨骼姿态。
  - Source：fresh `UpdateRigidBodyScaling:3569–3573`在普通非零缩放后把body重新置于GetBoneTransform；从零恢复则经`InstantiatePhysicsAsset:919–928,1071–1084`从同一当前骨骼世界变换创建。`SkinnedMeshComponent.cpp:2401–2411,2467–2475`以component-space骨骼乘当前mesh世界变换，保留mesh origin。
  - Target：`apply_profile_to_spawned_ragdoll:1347–1351`直接令已有body的Transform.translation乘scale_factor，该量已经包含世界位移；随后的实际Position因此围绕世界原点缩放。
  - 反例：完整角色的detached mesh origin=(3,0.0015,−2)，从1缩到0.75时，pelvis相对source额外移动(−0.75,−0.000375,+0.5)m，误差约0.901388m；放大到1.5时误差约1.802776m。unit/0.75/1.5×经过零/直接更新×默认/平移/零origin共18组10PASS/8FAIL；unit和零origin均通过，所有质量/关节锚点及rotation控制通过。证据`bf144_ragdoll_phase40_recovery_mass_frames_and_pose_controls`、`40/final/`。旧缩放Gate只核对Collider尺寸、质量和关节局部frame，未核对实际body Position与mesh origin。

- [ ] BF144-D118 / P2 — GetUp与动态过渡反算播放时钟，改变原生混出临界帧。
  - Source：fresh `AnimInstance.cpp:562–578`先UpdateWeight再Advance；`AnimMontage.cpp:2234–2256,2328–2333,2626–2637`逐步累加clip位置，并按当前section剩余时间开始混出；`AlphaBlend.cpp:192–228`累加AlphaLerp和剩余时间。两套原生montage均为1.5秒、RateScale=1.2、0.3秒Cubic，通知会把Advance切到精确分支点。
  - Target：`get_up.rs:107–126`用elapsed×rate反算clip位置、用剩余时间比例反算alpha，临界浮点结果与原生积累状态不同。50ms帧的target在clip约1.1400002就开始混出，原生该帧尚未跨入窗口；不能把数学上等价的表达式视为生命周期等价。
  - 反例：实际两朝向×5帧步长×4倍率共40组36PASS/4FAIL。50ms、单位倍率在elapsed约1秒时target权重0.92592585、source为1；1.1秒时约0.5对0.7046613，target第24帧清理、source第25帧。16.7ms×半速另在source清理帧残留1.19e−7权重，晚一帧清理。49.9/50.1ms、其他倍率及动态倍率通过；暂停只保留已有边界，不引入独立时钟差异。原生FAnimMontageInstance逐帧实证见`41/clock_front_all.native.json`、`clock_back_all.native.json`；target见`bf144_get_up_phase41_clock_weight_notify_and_end`。旧Gate验证最终结束/近似权重，没有相同delta序列的原生生命周期判据。
  - BF144.82动态过渡补充：fresh同一Montage规则的原生10068帧与独立推进公式逐值一致；`stop_link.rs:260–272`再次用elapsed反算位置及`duration/rate−elapsed`判定剩余时间。先1秒、再0.0221221838秒时，源position=1.5331833363并已进入1e−4秒混出窗口，蹲姿目标尚未开始混出；实际角色播放状态也复现。该输入的蹲姿时长与源完全相同，排除D167。站立短时长会偶然抵消这一个时钟边界，不能把只改时长视为完整修复。半速普通采样末端前最多约2.15µs偏差，原生位置替换后实际槽时间恢复；本扩展未验证全部AlphaBlend或停止请求，未重开GetUp完成项。

- [ ] BF144-D119 / P2 — GetUp停止与共槽混合读取上一帧Roll状态和权重。
  - Source：fresh `AlsCharacter_Actions.cpp:90–100`成功Montage_Play后立即置Rolling；`AnimInstance.cpp:2767–2772`同步停止同组旧montage，随后同一次UpdateMontage更新所有权重再求槽姿态。Roll原生BlendIn为0.1秒，已有GetUp混出保留原Cubic曲线并缩短时长。
  - Target：`get_up.rs:255–258,301–322,352–354`在PrePhysics前检查roll.active并读取roll.pose/weight，却没有排在`advance_rolls`之后；`roll_drive.rs:1912–1916,2094–2111`到PrePhysics后的bridge才写当前roll槽数据。旧GetUp与新Roll因而使用不同帧的数据。
  - 反例：62.5ms帧、已解锁的起身在第16帧接受Roll，当帧GetUp权重0.8709766而原生0.31640625，第17帧旧GetUp仍残留0.27558246而source已结束；实际Roll权重1但组合槽仅0.9591762。两朝向均失败；夹具只补GetUp晚于roll准备的顺序后，停止时点和旧权重全部恢复原生值，但第16帧组合槽仍只含GetUp的0.31640625，证明晚于Graph的bridge仍是独立阶段边界，未把该控制称作完整修复。证据`bf144_get_up_phase41_roll_order_controls`；50ms中断矩阵也复现。旧测试只覆盖Ragdoll替换和最终标记清理，没有同时核对新旧montage当帧权重。

- [ ] BF144-D120 / P3 — GetUp与动态过渡在section末端采样完整长度，遗漏原生末端偏移。
  - Source：fresh `AnimMontage.cpp:2721,2744–2748`在无下一section时停止播放，并将位置保留为section结束−UE_KINDA_SMALL_NUMBER/2；本机原生结果为1.4999500513秒，混出未完成期间持续保留。
  - Target：`get_up.rs:115–116`持续以elapsed×rate并min(sequence_length)，因此尾段采样1.5秒。
  - 反例：50ms帧在第24帧用Ragdoll缩短/重启GetUp混出后，两朝向第25–27帧都采样1.5而source为1.49995；动作Ragdoll和输入标记保持正确。早期中断及未越过末端的控制通过，证据`bf144_get_up_phase41_interrupted_cleanup`。差值约50微秒，只登记低优先级姿态/曲线采样尾差，不扩大为可见表现结论。
  - BF144.82动态过渡补充：四个当前Sequence的原生动态Montage停在1.5332833529秒；`stop_link.rs:260`夹到duration，`standing_source.rs:65`又夹到六位小数配置时长，实际Transition槽尾段采样1.5333329439。站立/蹲姿左右脚及半速/倍速六组全部复现，60Hz单位倍率保持12个尾帧；只恢复原生位置的六组和零速控制全部通过。归并同一遗漏的section末端规则，没有新增重复编号；证据`82/*.native.json`和`bf144_animation_phase82_actual_sampling_clock`。

- [ ] BF144-D121 / P3 — 动作通知时间在生成记录中舍入，极窄边界内提前或延迟实际消费。
  - Source：fresh原生GetEndTriggerTime为1.1665666103363037，`AnimMontage.cpp:2988–3012`在End branching marker执行NotifyEnd，ALS仅在当前仍GettingUp时置空并解锁。
  - Target：`docs/bf103-notify-source-audit.json:266–267`保存1.166667的end/duration，`generate_als_notify_manifest.py:104,125`将该值及offset传入manifest，实际f32终点为1.1665669679641724；`animation_events.rs:679–693`直接使用此终点。
  - 反例：前15帧62.5ms，再前进约34.638941ms，源/目标clip都到1.1665667，原生通知已结束，目标仍GettingUp且输入阻塞。两朝向都复现；边界前/后控制通过，在夹具内仅把manifest终点恢复原生精度也通过。证据`41/notify_exact.native.json`、`notify_edges.native.json`与`bf144_get_up_phase41_notify_end_precision_controls`。常规帧步长无此差异；未改原始记录、manifest或转换规则，未据此复开其他已完成通知资产。
  - BF144.92 Roll点通知补充：fresh A_Als_Roll 的SetGroundedEntryMode索引3/GUID `DC67D23A-40C84F64-0545E484-C7B4E6E6`原生时间0.9333333373069763，六位小数记录进入manifest后为f32的0.9333329796791077，提前约0.358µs。共同区间/资格的实际派发100组中12FAIL，差异进入模式组件、provider和真实T2条件；只恢复时间的100组全过。实际Roll输入启动、满权重PostLocomotion槽和真实播放记录的五组边界中两组提前写入FromRoll，五个恢复全部通过；当前已等于舍入时刻时也会漏掉本应稍后触发的事件。60/20Hz、owner0.5/1/2六组自然播放共327帧均只派发一次且时点符合共同源时钟。归并同一记录精度根因；没有新增编号或可见姿态偏差结论。证据`92/native.json`、`92/analysis.json`、`92/final/`，生产与原始记录未改。
  - BF144.94 即时状态补充：fresh九个动作montage中，Roll起止与manifest的f32完全相同；GetUp两项及Mantle六项的end分别被延后约0.358/0.238/0.358µs。先激活状态，再将共同位置设到原生end的下一f32并以零步长更新，排除marker跨越算法影响：八项都晚清动作并迟发End，48组中16FAIL；只恢复各自原生端点的24组全过，动作组件与实际图的locomotion_action同时恢复。High的下一f32恰等于目标end，依然是有效区间校正反例；不假设该间隔内还有另一可表示时刻。证据`94/native.json`及`bf144_animation_phase94_precision`，归并D121。


- [ ] BF144-D122 / P2 — GetUp槽遗漏montage自身的11条曲线。
  - Source：fresh两套GetUp montage的原生RawCurveData包含FootLeft/RightIk、FootLeft/RightLock、LayerArmLeft/Right、LayerHead/Legs/Pelvis/Spine、ViewBlock；sequence另有PoseCrouching和PoseGrounded。`AnimInstanceProxy.cpp:1968–1978`先采样AnimTrack，再EvaluateCurveData并Combine montage曲线。
  - Target：`get_up.rs:130–146,270–280`仅加载、采样sequence的GraphClip和AlsCurves；目标glTF的ALS_float_curves只含上述两条sequence曲线，没有montage曲线数据或组合步骤。
  - 反例：实际角色两朝向在clip=0时，post_locomotion.pose的上下文仅含PoseCrouching=1与PoseGrounded≈0；源还应有ViewBlock=1、六条Layer曲线=−1及四条Foot曲线=0。两组均遗漏全部11条，实际SourceSlot输出的六条Layer及ViewBlock数值也不符；夹具内补齐曲线后两组producer/consumer控制通过，sequence控制通过，证据`bf144_get_up_phase42_montage_curves`。旧Gate核对播放/通知/输入恢复及动作pose，没有将montage和sequence曲线同时纳入合同；仅记录，未改生产数据或代码。


- [ ] BF144-D123 / P2 — GetUp/Roll及Turn组合槽过早归一化，改变与基础姿态的多方旋转混合。
  - Source：fresh `AnimInstanceProxy.cpp:2007–2070`把所有非加性montage及剩余基础姿态一起送入BlendPosesTogetherIndirect；`AnimationRuntime.cpp:458–478`逐项加权累加后只归一化一次。
  - Target：`get_up.rs:308–311`先经blend_slot_pose归一化GetUp/Roll，再由实际AB_Als的SourceSlotNode与基础姿态混合；中间归一化改变了两montage合向量的长度，三个输入均非零时不再等价。
  - 反例：完整角色两朝向，在50ms帧、GetUp接受Roll后的第23帧，已消费的GetUp/Roll/base权重约0.129629/0.5/0.370371；目标相对相同输入、相同权重的源一次累加结果，最大四元数距离约0.01280（约1.47度）。62.5ms另一组约0.00220；四组三方混合均失败，十组单montage/基础姿态控制通过。实际producer/consumer分别与原实现计算完全一致；只在夹具去掉中间归一化即恢复到约1.2e−7。证据`bf144_get_up_phase42_three_pose_slot_blend`。此判据固定已消费的时间和权重，独立于D118/D119，不重复计入旧时序差异；生产未改。
  - BF144.91 Turn补充：`turn_layer.rs:461–487`预混后再进入SourceSlot也存在同一中间归一化边界。先只把Turn的slerp改为nlerp以隔离D177，双/三实例加基础姿态的40组仍有28FAIL，最大四元数距离0.001550676；使用源一次累加后全部恢复。总权重1、两层各0.5及相同姿态控制通过；普通三层曲线的浮点重组残差也保留在原始报告中，不另计缺陷。证据`91/analysis.json`的nlerp控制及实际站蹲槽输出；未改生产。


- [ ] BF144-D124 / P3 — GetUp虚拟骨骼插值偏离当前源Sequencer模型的逐次生成。
  - Source：fresh两sequence的DataModelInterface实际为AnimationSequencerDataModel；本机未烘焙GetAnimationPose走该活动模型，`AnimSequencerDataModel.cpp:775–799,1466–1494`在采样当前骨骼后，以Target组件变换相对Source组件变换生成11根虚拟骨骼。保留的旧AnimDataModel不能替代当前接口作为oracle。
  - Target：两GetUp glTF把虚拟骨骼保存为30Hz、46个关键帧的独立轨道；`source_sequence_player.rs:90–110`对它们与普通骨骼一样插值，未按当帧Source/Target重新求相对变换。关键帧前后构造相对变换与帧间插值不交换。
  - 反例：两朝向在0.375/0.75秒的实际GetUp槽，原生完整79骨骼姿态对比均失败，最大位置差0.62–1.21mm、四元数距离约0.000679，集中在手部和足部虚拟骨骼；其余12个时点通过，16组68根原始骨骼读取及所有sequence曲线/根运动输出控制通过。在夹具用相同普通骨骼和fresh虚拟骨骼映射重算后，全部16组恢复既有3e−5容差，最大位置残差约6.3e−6m。证据`42/native_front_full_pose.json`、`native_back_full_pose.json`与`bf144_get_up_phase42_native_bones_and_root_output`；仅确认本机当前未烘焙采样路径，不冒称已覆盖cooked压缩路径。旧导入Gate只比较普通骨骼和关键帧，原始track读取也不会执行活动模型的虚拟骨骼生成。


- [ ] BF144-D125 / P2 — 大型平地箱体的胶囊sweep数值偏差，改变落地与地面预测。
  - Source：fresh `CapsuleComponent.cpp:240–244`与`ChaosEngineInterface.cpp:122–147`确认半高包含端球、segment沿Z且半长H−R；`WorldCollision.cpp:545–563`、`SceneQuery.cpp:365–382`、`SQVisitor.h:336`及`GeometryQueries.h:320–325`进入MTD开启、Thickness=0的原生SweepQuery/GJKRaycast2。本机同输入4m/100m箱体均返回20ms方向distance=0.055680113m、50ms方向=0.054472971m；源不是先前的解析平面假设。
  - Target：Avian/Parry的support-map shape cast进入directional_distance→minkowski_ray_cast，后者在距离阈值或不再收紧上界等条件下返回当前TOI；`motion.rs:2701–2733`直接把该距离经源式pullback转为applied及remaining_time，再进入落地walking积分。当前证据定位到实际查询核差异，未把某个提前退出分支冒称为已动态插桩确认。
  - 反例：完整角色相同H=0.9/R=0.3m、start.y=0.94m、初速(3,−3,0)m/s；100m箱体返回0.047215056/0.041710450m，分别提前8.465/12.763mm，命中witness仍分离6.082/9.372mm。4m的50ms方向接近原生，20ms方向另有0.069mm低量级差异。替换旧解析oracle为原生距离后，3帧长×有无保留输入×两箱体的12组实际落地消费者仍为6PASS/6FAIL，失败仅100m；20ms无输入时速度低0.105535m/s，100ms无输入位移少约10.03mm。证据`43/queries.native.json`、`bf144_capsule_phase43_native_queries`与`bf144_capsule_phase43_landing_remaining_with_native_time`。
  - 漏检/归并：BF144.07曾因缺少源查询实证保留U01；本批沿同一反例补齐原生结果，不重开已完成的落地回调/摩擦条目，不新增平滑、容差或查询补偿。100m条件去掉后六组消费控制通过；胶囊半高退化成半径使原生高度self-test失败，恢复后复验。
  - BF144.70补充查询边界：4个原生地面预测输入与解析平面距离一致；目标3个斜向扫描分别提前0.487/4.130/0.895mm，另一个方向对照通过。100m箱体起点穿透1/10cm时，源MTD分别为−1/−10cm，转换为Hit.Time=0；target直接support-map扫掠与实际SpatialQuery都返回None，4m控制均命中。关闭compute-impact-geometry仍无命中，排除碰撞树和后续接触重算；归于已登记查询核边界，未声称两种现象触发同一内部退出分支。证据`70/plane.native.json`、`70/penetration.native.json`、`70/penetration_isolation/`；实际预测输入恢复验证继续保留在本批记录。
  - 预测消费复验：20组原样/恢复中16PASS/4FAIL；三处斜向查询及初始穿透分别使Fall输入偏差，其中穿透样本target=0、source=1。只恢复源查询推导的当前预测字段，四处全部通过实际Fall混合输入合同；其余16组源公式、锁存、时间/缩放/屏蔽对照通过，见`70/final/`。Default Overlay的独立时序差由D153单列，不以恢复Fall声称整个姿态已等价。

- [ ] BF144-D126 / P2 — 凸包支持面提前截为四顶点，丢失有效支持点并改变布娃娃碰撞旋转。
  - Source：fresh `CollisionOneShotManifolds.cpp:422–506,979–1054,1078–1183`沿完整凸多边形的各条边裁剪box面或capsule轴线；box使用最多32点的中间buffer，裁剪结束后才将接触减到4点。实际Barrel原生head得到2点、foot得到4点。
  - Target：Parry `ConvexPolyhedron::local_support_feature:945–975`直接使用支持面前4个顶点；`contact_manifolds_pfm_pfm:111–132`裁剪该局部多边形并追加GJK点。当前Barrel顶面8顶点，前4个顶点构成的区域不覆盖源任何有效支持点，实际生产Collider接触仅留下一个GJK点。
  - 反例：实际PA head/foot与生产Barrel Collider，在scale=1/1.5、帧长1/60与1/30的八组真实contact constraint回弹消费中均失败。unit head的向上速度约0.599995而源两点应为0.616885m/s；unit foot角速度由源约(−0.442254,0,1.083443)变为(0.041313,0,0.028511)rad/s，差约1.16rad/s。保持质量/惯量输入、两轮速度预算、点序及其他消费者相同，仅换回native支持点的八组恢复既有速度/角速度容差；四边形支持面四组位置控制也通过。证据`46/initial/`、`bf144_convex_phase46_support_feature_retains_native_contacts`与`bf144_convex_phase46_support_points_reach_real_response`。
  - 漏检/归属：旧primitive-box Gate未覆盖大于4顶点的源convex面；.44只验证凸包点云/缩放，.45记录raw集合差异后本项才沿实际消费者确认。该截断同时影响box与capsule，与D101近水平capsule的normal/轴线判定边界分别登记；未修改依赖、生产几何或支持点算法。

- [ ] BF144-D127 / P3 — 回弹混用准备阶段有效质量与积分后的惯量，改变非均匀惯量刚体的角速度。
  - Source：原生SolverType=−1，fresh `PBDCollisionConstraints.cpp:351,373–377`使用Gauss–Seidel；`PBDCollisionSolver.h:772–859,1062–1065`同时缓存ContactMassNormal与InvI×RxN，`738–770`直接消费缓存角响应。`SolverBody.h:360–364`的位置修正仅累计DQ，shock propagation需要重算时也同时更新质量和角响应。
  - Target：Avian `contact/normal_part.rs:39–110`准备时保存effective_mass；`solver_body/plugin.rs:116–123,295–303`在每次位置积分后更新world inertia；`contact/mod.rs:358–411`回弹仍读缓存质量，但角冲量使用刷新后的矩阵，两个量不再来自同一姿态。
  - 反例：完整角色实际foot、同一native四点、源两轮回弹与相同阶段V/W=0，prepare逆惯量对角约(31.3792,31.3792,146.4362)，四处cached质量均约0.669501；60Hz单子步在回弹时按新矩阵重算已变成0.667765/0.671513。源角速度约(−0.1122104,0,0.0958985)，实际为(−0.1104980,0.0020097,0.0942158)rad/s；默认六子步也不一致。头/足×60/30Hz×1/6子步×原生/各向同性×恢复/保留共32组28PASS/4FAIL，失败均为保留实际foot惯量；只在夹具回弹前恢复prepare惯量即全部通过，实际未恢复输出另与新旧矩阵混用公式一致。证据`bf144_inertia_phase47_restitution_uses_prepared_tensor`、`47/initial/`、`47/final/`。
  - 漏检/消融：此前各向同性矩阵不会随姿态变动，head也未触发该轴向差异；.46按post矩阵同时重算质量与角响应的oracle不适用于本边界，已改用实际prepare快照并核对cached质量，不将旧oracle误差计入缺陷。接触点、点序、预算与其他已知差异固定；移除夹具无用几何分支后同target重编，原16组结果逐字相同。仅登记，生产及依赖未改。

- [ ] BF144-D128 / P3 — 凸包接触未保留源参考面投影，改变支持深度和旋转修正。
  - Source：fresh `CollisionOneShotManifolds.cpp:870–905,973–1071`以保存的面平面、0.002参考面偏置及三维裁剪生成接触，并将所有裁剪点投影到选定参考面。原生Arch_01的hull 0/1顶面各有4顶点，源自身也保留约1.8mm的非共面残差；不能将非共面本身判作缺陷。原生平面投影公式与全部相应接触深度误差小于1e−10m。
  - Target：`PolygonalFeature::contacts_face_face`对面内点用前三顶点构造平面，对边交点直接插值原始边；未保留源的统一参考面投影。两处实际边插值逐点预测目标深度至约2.2e−8m，最大源差分别0.535916/0.534020mm；只使用原生参考面的对照缩小到约0.61µm，共面hull 3控制通过。两侧均为4点的hull 0排除D126截断因素。
  - 反例：完整角色创建的实际foot constraint，经真实窄相位过滤后拷贝进入公开位置solve；共同零V/W、硬接触、同一惯量、源点序，比较源首次位置迭代与该消费者的V×dt/W×dt。hull 0目标向上修正约1.000285mm、角修正近0；按BF144.50校正的源force point，应为1.129120mm和x约−0.00257236rad。hull 1同样失败，仅在prepare前替换native接触数据均恢复。其余相邻raw样本一并核对，23组native几何控制全部通过。证据`48/topology.native.json`、`48/topology_analysis.json`、`bf144_convex_phase48_remaining_position_consumers`。
  - 漏检/范围：旧顶点云相同与primitive-box Gate未检查cooked面平面及边插值的实际消费；初次仅检查前三点平面不足以解释另一侧拱面，补齐边交点分支后两侧均闭合。这里证明首次位置消费者的独立差异，未声称全帧源动力仿真或多hull聚合已通过；生产未改。
  - BF144.59补充：Arch01 hull 2的参考面只有三个顶点，native phi仍为`dot(P−O,R)×dot(R,N)`，逐点误差<0.14nm；target沿N投影，不能把本项根因限定为非共面。与D132一起隔离后，保持实际N及其他hull，仅按同一Parry裁剪API使用R并恢复源深度，完整compound摩擦由wX=0.899109恢复至0.902285，source为0.902309；仅深度控制仍FAIL。见`59/remaining_analysis.json`、`59/final/fields/`，同根扩展。

- [ ] BF144-D129 / P2 — 已有支持点后仍追加GJK点，改变接触集合与位置求解。
  - Source：fresh `ConstructCapsuleConvexOneShotManifold`沿完整面裁剪胶囊轴线，原生Arch_01 hull 3与head平面接触为两个端点；`PBDCollisionSolver.h`按该manifold逐点投影。
  - Target：Parry `contact_manifolds_pfm_pfm.rs:111–132`在3D中即使面裁剪已经得到支持点仍追加GJK点；Avian只对超过4点的manifold裁点。实际head的第三个中间点因此保留，foot的五点被减到四点则是此前通过的控制。
  - 反例：源点序、相同质量/惯量和首次硬位置投影下，实际head三个点给出向上1.193881mm、角修正(x,z)约(0.00246798,0.00352465)rad；原生两点应为1.124689mm及(0.00158931,0.00226977)。60/30Hz×保留/删除GJK的四组2PASS/2FAIL；只在prepare前去掉唯一两侧feature id均为UNKNOWN的点即恢复容差。证据`bf144_convex_phase48_gjk_position_control`与`48/final/`。
  - 漏检/消融：BF144.46仅排除了实际foot中已被裁掉的GJK点，没有证明三点head路径等价。本项从实际ContactGraph识别并删除该点，保留其余点、法向和顺序，区别于D100点序及D126缺失支持点；生产未改。
  - BF144.51同根扩展：box并非一律通过。Arch01 hull 4脚部对hull 3的四个裁剪角点加GJK后触发Avian五点裁减；深度加权选择的前两点在相邻边上，后续仅一侧有极值，最终丢失一个角点而剩三点。源恰有四点，不进入reduction；目标z角修正−0.431838rad，删除额外GJK后+0.059941rad，剩余参考深度和裁剪方向差异恢复后为源+0.061701rad。保持实际raw collider顺序、COM与全部其他输入，删除前后的点数/角点及消费者均直接验证；证据`51/face_initial/`与`51/clip_initial/`。此为同一pfm-pfm追加规则的新消费者边界，不新增编号。
  - BF144.52三角网格扩展：Fountain及三种Stairs的head沿同一pfm-pfm追加规则保留第三点，首次位置角修正偏离原生两个受力端点；60/30Hz只删除实际ContactGraph中唯一双UNKNOWN点的8组均恢复既有容差，原8组均失败。Cart的相同控制仍失败，保留独立核验。证据`bf144_trimesh_phase52_capsule_gjk_controls`、`52/capsule_initial/`。

  - BF144.56补充消费：Crate head的同面GJK点几乎与已有支持点重合，首次位置差异不显著，但速度摩擦仍重复消费；先剔除D140背面后，仍需删除该额外GJK点才能恢复。60/30Hz两因素控制确认两部分各自必要，沿用本项原因，不另计缺陷；见`56/backface_controls/`。

- [ ] BF144-D130 / P2 — 胶囊斜碰凸面时生成位于胶囊内部的辅助接触点。
  - Source：fresh `CollisionResolution.cpp:690–708`调用专用CapsuleConvex manifold；`CollisionOneShotManifolds.cpp:1200–1267`在端帽对凸面且abs(axis·faceNormal)<0.707时，从轴线减去垂直轴的圆柱法向×radius得到表面线段，裁剪后投影到凸面，再追加原GJK端帽点。native六个斜面样本的圆柱辅助点均处于9cm半径的真实表面；其中一个被裁空，仅保留GJK点。
  - Target：Parry `shape.rs:840–841`将Capsule暴露为Segment加border radius，`polygonal_feature_map.rs:20–24`直接返回轴线特征；`contact_manifolds_pfm_pfm.rs:94–105,143–149`先裁轴线再沿统一GJK法向偏移radius。轴线内部的结果到轴距离为r×sqrt(1−(axis·normal)²)，因此不是圆柱表面；实际内陷0.56–19.09mm。六个源支持面均仅3顶点，独立于D126四点截断和D101近水平专用分支。
  - 反例：实际PA head对Arch01邻接hull，abs(axis·normal)=0.584，目标辅助点内陷约16.94mm。共同零V/W、硬接触、同一惯量和源点序下，首次位置投影的z修正为−0.112751m，源为−0.127132m；y角修正为−0.070712rad，源为+0.322935rad。六姿态×60/30Hz×原始/只恢复圆柱辅助点共24组12PASS/12FAIL；控制保留实际GJK点、法向及其余消费者，全部恢复既有容差。证据`50/capsule_analysis.json`、`50/capsule_control/`、`bf144_side_phase50_capsule_surface_controls`。
  - 漏检/消融：此前水平顶面使axis·normal≈0，沿接触法向偏移恰好落在圆柱表面；仅核对点数或完整聚合会掩盖形体错误。本次逐叶隔离，并验证实际中间点的径向距离符合目标公式、源点处于真实表面；只改变辅助支撑即可改变失败信号。仅登记，生产及依赖未改。

- [ ] BF144-D131 / P2 — box与凸包的棱边接触遗漏原生专用投影分支。
  - Source：fresh `CollisionOneShotManifolds.cpp:914–969`在两侧支持面均不接近接触法向时标为EdgeEdge；正polygonal margin路径寻找两侧最近棱边，沿接触法向投影到静态棱边，生成单点并提前返回。实际Arch_02样本contact type=2、有效margin=(0.6cm,0)，两个最大面法向对齐值约0.9023/0.9231；源静态碰撞force point使用body侧投影点。
  - Target：Parry通用pfm-pfm路径仍做面特征裁剪并追加GJK点，实际得到四点；Avian四点不裁减，GJK的切平面位置也与源棱边投影相差约2.83mm。
  - 反例：完整角色实际foot对Arch_02 hull 2，固定源点序、零V/W、硬接触和共同惯量，首次位置投影的z角修正为0.05130rad，源为0.10393rad；仅删辅助点后为0.11355rad，仍不相符。60/30Hz×保留/删除辅助点×保留/恢复源force point共8组2PASS/6FAIL，只有两项同时恢复通过，最大角残差约1.34e−6rad；normal及phi保持实际值。证据`51/classification.native.json`、`51/edge_initial/`、`bf144_box_phase51_edge_controls`。
  - 漏检/消融：此前默认box正面对平面，没有进入源EdgeEdge分支；该样本独立于D129的平面多余GJK以及D128参考平面深度。四组合直接证明两部分均不可省略；只登记，未改生产碰撞或依赖。

- [ ] BF144-D132 / P3 — box与凸面按GJK法向裁剪，遗漏源参考面法向。
  - Source：fresh `CollisionOneShotManifolds.cpp:422–466,973–1071`使用选定参考面R，构造cross(R,edge)裁剪平面，保留另一形体表面点；接触法向N与R可以不同。五个原生box面样本contact type均为VertexPlane，参考面属于静态凸包。
  - Target：Parry `polygonal_feature3d.rs:207–242`将两侧支持面投影到以分离轴N为法向的二维平面，再求顶点包含和边交点；N不等于R时，裁剪边界和支持点位置随之改变。该差异与D128的深度计算分别可控。
  - 反例：实际Arch01脚部对hull 4，恢复源深度并去掉额外GJK后，x角修正仍为0.180056rad，源为0.189291rad。固定实际支持多边形、N、源深度公式和首次位置消费者，在相同公开PolygonalFeature::contacts中仅替换裁剪轴；五姿态的N控制全FAIL、R控制全PASS，最大角残差约2.1e−6rad。其他两Arch01、Arch02及Bench也复现，证据`bf144_box_phase51_clip_controls`与`51/clip_initial/`。
  - 漏检/消融：此前水平面使N与R平行，或仅比较点数/法向，未约束切平面内位置。保留N的单变量消融重新出现全部五处失败；不用新裁剪器、不放宽容差、未改生产代码或依赖。
  - BF144.59补充：Arch01 hull 2脚部原先首次位置测试未显著暴露约48.3µm的裁剪点偏差；在同时触及hull 3的有限次速度摩擦中，恢复后者的已知输入仍FAIL。只恢复hull 2参考深度仍FAIL，使用同一实际三角支持面、GJK法向N和公开裁剪函数，把裁剪轴改为R后PASS。该控制保留其他hull，未把整组native替换当作本项证据；见`59/final/fields/`。

- [ ] BF144-D133 / P2 — 三角网格共享边接触未归属到一个三角面，重复点改变响应。
  - Source：fresh `MeshContactGenerator.cpp:103–158`对cooked共享边建立EdgeContactDataMap；已有face contact时保留先前三角面的接触，跳过后续邻面上的同边接触。Stairs02实际face252/270共享顶点75/76；native踏面保留边上两点，立面仅保留上部两点。
  - Target：Parry `contact_manifolds_trimesh_shape.rs:160–215`逐三角形输出独立manifold，Avian逐manifold过滤/裁点；缺少源跨三角面的共享边选择。实际同一条边的踏面法向+Y和立面法向+Z均保留，立面多出底部两点。
  - 反例：完整角色实际foot对Stairs02，首次硬位置投影z修正0.020814m，源0.012406m；x角修正0.035726rad，源0.105745rad。保持实际点、法向、源点序及其余消费者，仅按fresh共享边和源保留面删除重复立面点，Stairs02/03×60/30Hz的4组全部恢复；原4组全部失败。Stairs01相同控制消除大部分差异但仍有独立背面接触，未冒称完整恢复。证据`52/edge_facts.json`、`52/edge_initial/`、`bf144_trimesh_phase52_shared_edge_controls`。
  - 漏检/消融：此前raw点集审计不识别cooked共享边，单凸包/primitive也没有跨三角面选择。删除控制保留同法向重复点，仅改变源已明确抑制的立面点，排除D129额外GJK和点序因素；未改生产网格或依赖。

  - BF144.56补充边界：同一`MeshContactGenerator.cpp:103–158`对同法向face contact也跳过另一三角面；Stairs踏面内部边的两处交点在target各出现两次。源first-face所有权保留8点，旧恢复仍留10点；位置/无摩擦响应近似相同，摩擦最后一步却再次处理重复支持。Stairs02在60Hz的actual vY=−0.510929、wY=−0.050122，source为−0.530816、−0.209820；只按已有cooked边和0.998面判据保留一个三角面的点即恢复。三种台阶×60/30Hz×摩擦0/0.7×保留/单面所有权共24组18PASS/6FAIL，全部恢复通过；不修改冲量资格阈值。旧位置反例不变，同根归并而不新增编号；证据`56/coplanar_controls/`。

- [ ] BF144-D134 / P3 — box对三角面遗漏SAT参考面偏置，选择不同法向与裁剪轴。
  - Source：fresh `ConvexTriangleContactPoint.cpp:297–308`给三角面分离值加0.01cm，再与box面和有效edge轴比较。Cart脚部cooked face988的三角面值−0.014653768m，box面为−0.014620466m；无资格过滤的最大edge轴为−0.014585355m，但它未通过Minkowski容差。源偏置使triangle胜过box，native type=VertexPlane、法向含z=−0.000740249；去掉偏置即变为PlaneVertex，.57补齐了此前全轴上界没有区分有效候选的边界。
  - Target：Parry `contact_manifolds_cuboid_triangle.rs:115–126`严格比较SAT分离值，没有该偏置；实际选择edge法向约(−0.00005136,1,0)，与独立edge轴复算一致。该面与生产triangle149的顶点偏差小于3.4e−8m，法向偏差小于3.8e−7，排除几何替换解释。
  - 反例：完整角色实际foot首次位置投影z修正约0，源为−1.10594e−5m。将原力臂等价平移到实际动态表面，保留全部点及实际深度，只改用同一目标三角面法向后z修正−1.10654e−5m，位置/角度均恢复既有容差；删GJK对foot没有效果。60/30Hz最小偏置控制均通过；证据`52/cart_triangle_map.json`、`52/cart_sat_analysis.json`、`52/cart_initial/`、`52/final/cart/`与`bf144_trimesh_phase52_cart_normal_controls`。
  - 漏检/范围：以往共面box/triangle使SAT轴相同，raw点数差又遮蔽了这条微小法向偏置。本项只确认foot的该面，不把仍失败的Cart head归入；未改生产碰撞规则。

  - BF144.57补充消费：源`ConvexTriangleContactPoint.cpp:64–109,325–341`由同一选定triangle面决定裁剪轴；旧恢复只改normal，四个支持点仍沿目标edge轴裁剪，切向位置约差10.8µm。在同一公开`PolygonalFeature::contacts`中仅把裁剪轴换成实际triangle面法向，保留实际深度、其余面、动态表面点及16点集合，摩擦60/30Hz都恢复；不改法向时60Hz无摩擦仍由D141失败。原normal-only位置Gate没有覆盖这个后续摩擦消费者，同根补充而不另增编号；见`57/clip_controls/`、`57/clipping_analysis.json`与`bf144_trimesh_phase57_cart_controls`。

- [ ] BF144-D135 / P2 — 共面三角形内部边生成切向接触，遗漏源无效法向剔除。
  - Source：fresh `MeshContactGenerator.cpp:294–316`按共享边两邻面的法向范围核验；当前阈值0.7，超出范围过多即禁用该点。Stairs01 cooked face270/271共面且法向均+Z，内部边76/78的额外接触法向与面法向点积0，邻面点积1，因此原生不保留该点。
  - Target：生产Collider::try_trimesh使用空flags，Parry triangle_normal_constraints返回None；逐三角形SAT可在该内部对角线生成约(0.026977,−0.999636,0)的面内法向，实际约束保留一处0.619mm穿透。它并非背面接触；仅按背面条件过滤无效果。
  - 反例：完整角色foot对Stairs01，先固定D133共享边去重，实际y位置修正0.579422mm、z角修正0.002690rad，源为1.052046mm及−0.001232rad。再只按fresh共面共享边和源0.7规则剔除该点，60/30Hz均通过；内部法向剔除/共享边去重的8组2PASS/6FAIL，两个恢复控制缺一仍不等价。证据`52/internal_edge_facts.json`、`52/internal_initial/`和`bf144_trimesh_phase52_internal_edge_controls`。
  - 漏检/消融：平面primitive没有内部三角边；此前未将三角形邻接法向范围接入raw→实际消费者核验。删除控制只移除已由源规则证实无效的一点，保留其他实际点、深度与法向；生产和依赖未改。

- [ ] BF144-D136 / P2 — 胶囊对薄三角面的端帽、边侧剔除及面接触支持集合遗漏。
  - Source：fresh `CapsuleTriangleContactPoint.cpp:221–240,268–271,352–358,440–451`使用端帽资格、最近边的有符号面侧判定，以及abs(axis·faceNormal)<0.34时的面法向接触。Cart head的cooked face988只生成一个端帽点，989只生成一个穿越边点；逐面原生集合与聚合后相同。988/989另外两个可检查边候选的DotFace约−7.54cm，源直接拒绝；端帽表面沿本三角面法向偏移9cm。
  - Target：Parry `contact_manifolds_pfm_pfm.rs:94–105,143–149`对Segment/triangle统一投影裁剪，再沿GJK法向偏移radius，没有上述边侧/面接触规则；实际对应triangle149/150在ContactGraph各留下4点，法向近竖直，端帽表面沿切向偏约57µm。仅删除GJK点仍失败，区别于D129；也不是box的D134 SAT偏置或跨面D133共享边归并。
  - 反例：完整角色实际head的首次硬位置投影y为0.015918558m、x角修正−0.033211265rad，源为0.015739888m和−0.030906435rad。只在这两个薄面保留与原生支持点对应的目标点、恢复源法线及表面位置，保留实际深度和其余两个面的目标集合后，两帧长均通过；单独或缺任一控制仍FAIL。最终16组thin为2PASS/14FAIL，另12组Fountain/Rock控制10PASS/2个既有D129 FAIL；证据`53/all.native.json`、`53/capsule_analysis.json`、`53/initial/`、`53/final/`与`bf144_trimesh_phase53_thin_controls`。
  - 漏检/消融：此前普通宽面或完整native点替换掩盖逐三角面生成差别；本次补读聚合前集合，并用相同源几何独立复算端帽/边条件，表面点与原生残差小于1.5e−14cm。额外深度替换和未使用的全量native注入已移除，同target重编后28组响应逐字不变；读取工具的字段缺失、坐标空间、索引及深度消融均拒绝，恢复后通过。仅登记，生产和转换器未改。

- [ ] BF144-D137 / P2 — 速度阶段未按源逐点联合采样并施加法向与切向冲量。
  - Source：fresh `PBDCollisionSolver.h:668–744,1201–1222`先从同一个body V/W取法向与两条切向误差，计算全部冲量，再合并施加；gather保留固定力臂和切向基。原生Velocity.FrictionEnabled=True、Use1DFriction=False、stiffness=1，当前世界末一次velocity迭代执行摩擦。
  - Target：Avian `contact/mod.rs:286–318,323–351`先施加法向冲量，再重新计算接触点速度求摩擦。偏心接触的法向角冲量会改变切向速度，因而输入已与源不同；本项不涉及位置阶段D94或旋转后法向间距D95。
  - 反例：完整角色head对Fountain的同一原生表面点、质量、共同惯量、同一正法向位置冲量、摩擦0.7、零回弹目标及阶段输入V=(1,−1,0.4)、W=0，仅执行最后速度步。法向冲量后接触切速从(1,0,0.4)变为约(1.43,0,0.0989)；实际vX=0.473034、wZ=−3.556902，源为0.625838、−1.140109。只令现有target切向solver读取法向求解前的速度、保留实际法向冲量和Coulomb上限，即恢复；head/foot×60/30Hz×摩擦0/0.7×原始/恢复共16组12PASS/4FAIL，所有恢复和零摩擦对照通过。
  - 漏检/消融：以前单点夹具令力臂沿法线，法向冲量不改变切速；多点全轨迹又混入点序、位置阶段和几何差异。先做32组顺序/分轴质量四组合，证实该单步只需顺序控制，再删多余组合，同target重编保持16组响应逐字不变。闭式复算重现两侧冲量与锥截断分支；证据`54/single_order/`、`54/final/single_order/`、`54/single_analysis.json`及`bf144_trimesh_phase54_single_order`。仅登记，未改生产。

  - BF144.58补充分组边界：同一源`SolveVelocity:1166–1245`在每点施加N/T后立即进入下一点；target两段循环把本点摩擦延至后续点normal之后。固定源点序、共同位置输入和已恢复的各点pre-normal采样，Fountain head两点actual vX=0.304290、wZ=−10.522009，source为0.535978、−5.964850；改回每点N/T合并即恢复。Crate foot、Fountain两body与Rock foot两种缩放在60/30Hz均复现，八姿态×零摩擦/两帧长摩擦×块/逐点共48组38PASS/10FAIL，所有逐点及零摩擦控制通过；只有一个活动点的三个head姿态也通过。normal块结果与同一实际多点`ContactConstraint::solve`逐位相同，排除另造法向算法；见`58/final/group/`。单点联合采样只是同一阶段语义的一部分，不能单独替代逐点施加顺序；同根补充。

  - BF144.61补充convex分组：30姿态中13个在固定源点序后仍受组内N/T分段影响；零摩擦响应逐位相同，全部逐点恢复PASS。Arch01 hull 2 foot同时接触两个hull，60Hz摩擦wX由source 0.902309变为组处理12.485702；Bench及Barrel也复现。每个法向块都与同一实际多点ContactConstraint::solve逐位一致，未把多个hull拼成一个法向；错误合组消融稳定失败并经同target重编恢复。完整四组合及30Hz反例见`61/final/`、`61/group_ablation/`，同根补充。

- [ ] BF144-D138 / P3 — 切向速度摩擦使用沿当前滑动方向的单一有效质量，遗漏源分轴解算。
  - Source：fresh `PBDCollisionSolver.h:787–831,691–713`分别保存U/V两个固定切向基的有效质量，逐分量算冲量后统一裁到摩擦锥；当前Use1DFriction=False。head固定输入的两条切向有效质量约1.315069/1.389128。
  - Target：Avian `contact/tangent_part.rs:207–241`用当前两条切向速度分量计算单一方向有效质量，再让增量冲量平行当前切速；同一输入的有效质量约1.370716。两侧均发生Coulomb截断，但分轴缩放后的冲量方向不同；不是锥边界epsilon导致的差异。
  - 反例：沿D137同一实际head点，先执行一次无摩擦法向步，使最后摩擦步的法向误差为0，隔离顺序因素。实际vZ=0.363549、wX=3.921909，源为0.372164、3.785663；仅恢复分轴质量后通过，单独改采样顺序没有作用。head/foot×60/30Hz×摩擦0/0.7×原始/恢复共16组14PASS/2FAIL；foot及零摩擦对照均通过。
  - 漏检/消融：旧中心接触与轴向切速使两种质量公式退化为同一结果，未覆盖法向角冲量改变滑动方向后的摩擦。32组分离控制缩至16组后输出逐字不变；独立闭式复算同时重现两侧冲量，截断前模长距锥上限至少0.038，排除阈值猜测。证据`54/single_mass/`、`54/final/single_mass/`、`54/single_analysis.json`及`bf144_trimesh_phase54_single_mass`。10个多点共同几何样本同时恢复D137/D138后全部通过；仅登记，未改生产或依赖。

- [ ] BF144-D139 / P2 — 法向速度阶段重新激活零位置冲量点，缺少源资格条件。
  - Source：fresh `PBDCollisionSolver.h:1115–1142`的8次无摩擦位置投影保存每个点的NetPushOutNormal；`1178–1181,1225–1236`在默认position启用、split=0时，仅对其大于0的点执行法向速度求解。`Evolution/SolverBody.h:142–149`先用DP/DQ除以dt加入V/W；源速度累计量从0开始，负下限为−NetPushOutNormal/dt。
  - Target：Avian `contact/mod.rs:287–321`在速度松弛阶段遍历所有prepared点，`normal_part.rs:129–164`只约束累计冲量非负。位置冲量为0的点仍可因当前向内速度而产生新冲量；没有保留源的逐点资格。
  - 反例：完整角色head对Crate_Broken的同一两个原生点、实际质量、共同惯量/点序/预算、摩擦和回弹目标均为0，输入自由V=(1,−1,0.4)。源8次位置后，第二点pushout=0、线性间距=0.477mm；目标首个速度步仍给它约1.3113冲量。60Hz两轮后actual vY=0.029936、wZ=0.711528，source为−0.248935、4.865362。只跳过源零pushout点即恢复，单独去掉D95预测偏置无效；30Hz同样失败并恢复。Rock缩放1.75控制确认同根，Stairs多点需要与D95共同恢复。
  - 漏检/消融：旧速度夹具给全部穿透点统一正位置冲量，未覆盖源位置迭代淘汰点。18姿态两因素四组合矩阵72组51PASS/21FAIL，完整恢复全通过；最终去掉无用softness覆盖并精简一般矩阵至36组，保留输出逐字一致；3个分离姿态×60/30Hz×四控制共24组10PASS/14预期FAIL。全正间距及不受影响姿态提供对照。证据`bf144_trimesh_phase55_activation`、`bf144_trimesh_phase55_minimal_controls`、`55/final/`；共同阶段消费证明，未声称原生完整布娃娃轨迹已一致，未修复生产或转换器。

- [ ] BF144-D140 / P2 — 三角网格背面仍生成接触，遗漏原生单面条件。
  - Source：fresh `CapsuleTriangleContactPoint.cpp:85–104`在轴心位于三角面背面时拒绝整个胶囊接触；`ConvexTriangleContactPoint.cpp:137–151`和`MeshContactGenerator.cpp:253–270`分别处理box背面分离与背面接触法向。Crate_Broken正反两面cooked175/176位置重合，原生逐面胶囊分别生成2/0点；轴心有符号距离±8.898455cm，拒绝容差约0.00009cm，aggregate只留正面。其PA、材质和旧aggregate payload完全相同。
  - Target：生产trimesh的normal constraints为空，Parry `contact_manifolds_trimesh_shape.rs:193–216`对两面均调用双向convex/triangle，Avian不按源单面规则过滤。实际triangle138/150顶点距native面仅2.28e−8m、绕序相反，接触法向与两面法向点积约+1/−1，两组均进入prepared约束。
  - 反例：完整角色实际foot对Crate_Broken、已对齐受力点和源速度公式、摩擦0.7，两面共8点，60Hz actual vY=−0.270138、wZ=1.981274，source为−0.326681、1.716444。只剔除背面后保留4点并恢复；去GJK无效果。head需同时消除该背面和既有D129的额外GJK，二者单独恢复均不通过；60/30Hz及零摩擦对照覆盖。
  - 漏检/范围：旧首次位置/零摩擦测试的重合面法向相同，重复点没有显著位置差；不能由此认定原始几何数量差无影响。32组背面/GJK组合22PASS/10预期FAIL，按源规则联合恢复全通过。证据`56/backface.native.json`、`56/native_check.json`、`56/face_analysis.json`、`56/backface_controls/`及`bf144_trimesh_phase56_backface_controls`；这是当前原生几何和共同阶段输入的实际consumer证明，生产和原始资产未改。

  - 夹具消融：移除prepared manifold到raw triangle的一一匹配后，两组被错误地映射为同一背面，恢复控制全部失败；按源码字节恢复并在同target重编后，恢复控制重新通过。删除foot无信号GJK组合，最终24组16PASS/8预期FAIL，与原32组保留行逐字一致；同面点不能只靠位置近似值复用三角面身份。

- [ ] BF144-D141 / P3 — box对三角面的棱边候选缺少源Minkowski容差，误选近共面分离轴。
  - Source：fresh `ConvexTriangleContactPoint.cpp:247–256,297–315`先筛选有效棱边，`ConvexContactPointUtilities.h:249–256`要求ADC×BDC<−0.01且CBA×BDC>0.01。Cart foot face989的近竖直目标轴虽然分离值更大，CBA×BDC只有0.00106483cm²而被排除；独立复算有效候选后box面胜出，native type=PlaneVertex、normal=+Y。去掉该容差才选出目标edge轴；triangle偏置在这一面不足以改变结果。
  - Target：Parry `sat_cuboid_triangle.rs:94–139`仅排除近零叉积，遍历九个edge方向；`contact_manifolds_cuboid_triangle.rs:115–126`按严格最大分离值选轴，实际triangle150法向为(−0.0000768247,1,0)，与不带源资格容差的复算一致。
  - 反例：完整角色实际foot的prepared约束，使用共同8次源位置投影及两次速度预算、既有摩擦/资格控制。60Hz无摩擦actual vY=0.00261470、wZ=−0.0437744，source为0.00257435、−0.0443716；保留全部16个实际点及深度，只把triangle150法向恢复为box面后通过原2e−5/2e−4门槛。删共享边两点无效；30Hz对照本已通过。与D134裁剪因素组成16组10PASS/6预期FAIL，联合恢复全部通过。
  - 漏检/范围：旧单次位置响应未放大这条微小法向差异；此前全SAT轴复算没有执行源有效候选筛选。fresh完整native、两面独立裁剪和原点消费者闭环，证据`57/sat_analysis.json`、`57/clipping_analysis.json`、`57/restored.native.json`及`57/final/`；只登记当前实际姿态，不泛化为所有box或修补生产依赖。

- [ ] BF144-D142 / P2 — 胶囊轴线被当作闭合双边特征，重复交点进入速度求解。
  - Source：fresh `CollisionOneShotManifolds.cpp:465–517,1154–1196`将胶囊轴线作为单一线段裁剪，只保留两个端点；当前原生Arch02 head对hull 0为一个负phi交点和一个正phi端点。
  - Target：Parry `polygonal_feature3d.rs:55–61`把Segment设为两个顶点且两个edge id均为0；`333–369`随后按闭合多边形正反各遍历一次，生成两处坐标、深度及feature id都相同的内部交点。加上GJK后共四点，Avian不裁减。删除GJK仍留下该独立重复，不能归为D129。
  - 反例：完整角色实际head与生产Arch02 compound，固定源阶段、顺序、其他已证实支持几何和原始N/r/phi；只删除完全相同的非GJK记录，60/30Hz×摩擦0/0.7共8组4PASS/4FAIL，四个删除控制全部恢复。60Hz无摩擦actual vY=0.047037、wZ=4.052744，source为−0.061554、4.722065；摩擦同样失败。两侧DP/DQ逐位一致，但重复点残留约8.18e−10位置冲量，按源严格>0资格触发后续速度步，不能靠放宽阈值消掉。
  - 漏检/消融：旧首次位置响应及点数合同未检查后续资格，重复点的位置作用被有限精度抵消；保留重复即稳定复现，去重保持其他点及全部字段不变。证据`59/diagnostic/`、`59/final/duplicate_controls/`及`bf144_convex_phase59_duplicate_controls`；只登记，不修改依赖或生产实现。

- [ ] BF144-D143 / P3 — box对凸包直接使用完整形状求法线，遗漏源核心边距。
  - Source：fresh `CollisionOneShotManifolds.cpp:840–853,525–570`取constraint有效margin，通过`TGJKCoreShape`调用GJK；`ChaosCore/Public/Chaos/AABB.h:322–350`把box各面向内移动该边距。Bench foot的实际margin为0.6cm、静态凸包为0；UseGJK2=false。原生三个核心支持点的重心分离为0.5000003cm，法线为+Z，加入边距后接触phi约−0.1cm；由actual simplex独立复算，未进入重叠EPA。
  - Target：Parry `shape.rs:779–781`给Cuboid提供完整多边形且border radius=0，`contact_manifolds_pfm_pfm.rs:64–105`直接查询。相同Bench实际局部输入形成重叠simplex并进入EPA，返回runtime N=(-9.8717e−5,1,0)；用同一公开GJK仅把box半长减0.006m，即得到竖直法线，不需要更换求解器或精度。
  - 反例：完整角色实际foot及生产compound，保留原八点、r/phi、其他hull和已证实的共同阶段。60Hz无摩擦actual wY=0.7256534，source=0.72523594；只应用核心查询的N后为0.7252306。两帧长×摩擦0/0.7×原始/恢复共8组4PASS/4FAIL，四个恢复全PASS。字段不变性与公开API原始结果核验均通过；证据`60/gjk.native.json`、`60/native_check.json`、`60/diagnostic_exact/`、`60/final/`。
  - 漏检/消融：旧box正面样本的外形/核心方向相同，首次位置消费也未放大该微小差异；triangle SAT的D141不是此路径。移除边距即恢复原失败；只增加通用reader取证字段及自测，生产未修复。

- [ ] BF144-D144 / P3 — 胶囊线段裁剪的二维交点数值偏差进入速度摩擦。
  - Source：fresh `CollisionOneShotManifolds.cpp:465–517,1154–1196`按边平面有符号距离逐步裁剪一条线段，再偏移到圆柱表面。原生Arch01 hull 4两个支持点重合；按源三维公式独立复算误差约13nm。
  - Target：Parry `polygonal_feature3d.rs:333–369,382–428`通过二维Gram行列式求边交点，当前输入的近似项相减使参数为0.499925822；同一投影坐标用叉积复算约0.50000119。实际两个圆柱支持点相距6.793µm，公开裁剪API在实际compound相对变换下逐位重现；不能归为D142重复记录或D130圆柱表面偏移。
  - 反例：保留实际N、phi、GJK点、数量和其他hull，在同一f32几何上仅换用源三维裁线，局部端点间距降至0.103µm；经过原世界变换后只有第二个受力点改变。60Hz摩擦actual vY=−2.3972e−5、wX=0.542248，source约0、0.542461；恢复后vY=5.28e−7、wX=0.542470。两帧长×摩擦0/0.7×原始/恢复共8组6PASS/2FAIL，全部恢复及零摩擦控制PASS；证据`60/numerical_analysis.json`、`60/cylinder_analysis.json`、`60/diagnostic_exact/`、`60/final/`。
  - 漏检/消融：旧位置Gate及无摩擦样本未暴露后续摩擦误差，笼统归因f32也不能解释同精度源算法恢复。移除源裁线即复现两处失败；删除多余native替换、字段模式及诊断分支，同target重编后16组总响应逐字不变。未改依赖、生产或容差。

- [ ] BF144-D145 / P3 — 接触速度求解携带上一帧冲量，偏离源每帧从零初始化。
  - Source：fresh `PBDCollisionSolver.h:1002–1054`每次初始化manifold将NetPushOut/NetImpulse各轴归零；`PBDCollisionContainerSolver.cpp:373–415`的上一帧输出保存摩擦锚点和报告结果，下一帧gather不将旧冲量施加到body。当前世界使用已核验的Gauss–Seidel分支。
  - Target：Avian `contact_types/mod.rs:426–470`复制旧warm-start冲量；`contact/normal_part.rs:104–109`把它作为新约束累计量，`contact/mod.rs:223–262`在求解前施加到速度。有限轮数下，多点求解不会总是消除初值影响。
  - 反例：真实角色Cart head/foot与Stairs02 foot保留ContactId，采用同一当帧实际几何、点序、共同位置结果和两轮法向速度预算。60/30Hz、零/0.03Ns/清零控制共24组18PASS/6FAIL；上一帧实际产生的缓存再验证16组10PASS/6FAIL，所有清零恢复及单点Barrel控制通过。Cart head 60Hz实际缓存下wZ为1.077489，源零初值为−0.325842；只清零旧缓存即回到−0.325842。完整目标物理帧的输出也随该单一缓存控制改变。
  - 边界与漏检：共同阶段诊断仅对有源位置冲量的点应用实际warm_start/solve，以隔离D139；原生几何差异、点序和迭代差异不作为本项原因，未宣称完整Chaos动态仿真。旧首次接触测试均从零缓存开始，D96又刻意清零冲量以隔离锚点，因此未覆盖本项。证据`63/warm_initial/`、`63/generated_initial/`，生产实现未改。

- [ ] BF144-D146 / P3 — 隐藏期间更新montage后，恢复图更新没有重新触发OnBecomeRelevant。
  - Source：fresh `SkeletalMeshComponent.cpp:1883–1918`在隐藏时仅因活动montage等条件允许TickPose；`AnimInstance.cpp:709–724`的montage-only路径重置proxy更新计数。`AnimNode_LinkedAnimGraph.cpp:96`同步子实例计数，`AnimNode_CallFunction.cpp:29–40`据计数不连续触发OnBecomeRelevant。原生AB_Als_Head将InitializeHead放在该调用点，父实例随后直接把pitch/yaw设为当前目标。
  - Target：`post_physics_animation.rs:188–190`跳过求值时保留原arena计数；`source_instance.rs:212–217`下次只看到连续的一次递增，因此SourcePropertyWriteNode不再执行初始化。
  - 反例：完整角色隐藏超过1秒后，播放实际transition montage并确认隐藏期间clip推进到0.2秒、pose_ready、根图计数未推进；恢复可见时目标pitch第三人称4.921°、第一人称30.767°，源应为45°。60/30Hz共16组12PASS/4FAIL，只补回InitializeHead标志的控制全部PASS；持续可见及无montage的纯空闲隐藏控制也通过。实际Head回调写入当前帧，并进入生成的BS_Als_Look输入链，最终head姿态已采样。
  - 漏检：旧可见性测试核对gate/pending及帧计数，没有区分ShouldTickPose外层资格和实际CallFunction计数。首版把纯空闲隐藏也判为重置的oracle已撤销；结论只使用带实际montage资格的`65/checked_initial/`，未修改生产实现。

- [ ] BF144-D147 / P3 — 图停止更新时仍推进动画实例的历史全局时间倍率。
  - Source：fresh `UAlsAnimationInstance::NativeUpdateAnimation:111–116`用上一次NativeUpdate保存的global dilation计算DeltaTimeWithoutTimeDilation；隐藏且无montage时，UE的ShouldTickPose不调用NativeUpdate，历史值保持。`RefreshHead:637–648`在第一人称平滑中消费此dt。
  - Target：`head_layer.rs:194–204`每次PostUpdate都调用unscaled_delta_seconds并写回历史倍率，即使AnimationPoseUpdateGate已经停止图更新。
  - 反例：无montage隐藏期间把全局倍率从1变为0.25，恢复帧scaled dt=0.004166667秒。源保留旧倍率1，未缩放dt仍为0.004166667；目标却使用隐藏期间写入的0.25，得到0.016666668。实际Head pitch为30.767°、源应为11.286°；只恢复历史倍率即通过。6组5PASS/1FAIL，持续可见与第三人称控制通过；和D146的montage重激活条件分离，见`65/checked_initial/`。

- [ ] BF144-D148 / P3 — 替换动画实例后保留旧曲线投影，改变新实例的首帧姿态。
  - Source：fresh `AnimInstance.cpp:363–369,446–455`在Initialize前执行Uninitialize；`AnimInstanceProxy.cpp:455–466`清空全部AnimationCurves。`AlsAnimationInstance.cpp:400–405`以新实例的空曲线计算HeadBlendAmount=1；原生Head Additive节点绑定GetParent.ViewState.HeadBlendAmount。
  - Target：`source_gameplay_tag_input.rs:89–95`在Discard清理输入，`animation_visibility.rs:25–27`清理gate，但`animation_context_projection.rs:44–98`仅在求值后覆盖AlsRootAnimationContext；`head_layer.rs:61–69,206–218`在新实例首次求值前消费旧曲线。
  - 反例：实际Rifle瞄准后，PoseAiming=0.994912；覆写播放器或移除后重插时，新实例frame=1、elapsed=0，Head alpha仍为0.005088而源应为1。两路径×普通/瞄准旧姿态×保留/全部清空/仅清PoseAiming的12组10PASS/2FAIL，两个恢复控制均通过。仅清该字段使最终head四元数从约(0.025741,0.113965,−0.004760,0.993140)变为(0.026208,0.219716,−0.001992,0.975210)，其余旧曲线保持不变。
  - 漏检：既有Discard测试只覆盖标签、slot、linked graph与rig query输入，没有覆盖独立的根曲线投影。实例ID已变化、旧标签和gate已清理，排除旧实例继续运行。证据`66/initial/`、`66/field_initial/`与`bf144_animation_phase66_replacement_curve_ownership`；仅登记，未修复。

- [ ] BF144-D149 / P3 — 启动零delta求值后提前清除pending，改变首个普通更新的动画选择。
  - Source：fresh `Actor.cpp:6262,6297`先逐个RegisterComponent，再标记全部注册完成；`SkeletalMeshComponent.cpp:1036,1215–1216`在mesh注册过程中执行初始化零delta求值。ALS `NativePostUpdateAnimation:227`因此保留pending；之后`InitializeComponent:1455`以force=False调用InitAnim，不重新初始化已有主实例。首个普通tick仍按pending执行`RefreshInAirOnGameThread:1103–1104`，抑制并清除Jump请求，随后才清pending。
  - Target：`post_physics_animation.rs:188–205`用零delta完成首个pose后立即令gate.pending_update=False；`ab_als_provider.rs:310–313`在第二次图更新发布False，`locomotion_layer.rs:90–101,190`因而接受同帧Jump请求。
  - 反例：通过公开spawn入口生成新角色，确认首个图frame=1、elapsed=0、已在地面；下一帧实际Tnua Jump启动，两侧共同竖直速度4.0365m/s，目标FSM进入Jump（权重1），源应Fall。仅在夹具保留pending到这次更新，FSM即进入Fall（权重1）；延后一帧才按Jump的控制仍进入Jump。3组2PASS/1FAIL，证据`66/startup_initial/`及`bf144_animation_phase66_startup_pending`。
  - 漏检：旧启动检查验证零delta和非空pose，却未区分注册过程初始化与第一个普通更新的pending寿命。fresh原生T19条件确为InAir且bJumped，Jump/Fall状态身份取自原生节点；不是只核对请求或标签。仅登记，未修复。

- [ ] BF144-D150 / P3 — 动画输入在Custom模式仍使用Walking制动参数，改变Mantle混入时的地面姿态。
  - Source：fresh `CharacterMovementComponent.cpp:4351–4368`对MOVE_Custom返回最大制动0；`AlsCharacter_Actions.cpp:496–497`在Mantle播放前切到并锁定Custom。`AlsAnimationInstance.cpp:708–710,829–847,903–918`读取该模式值，dot(Acceleration,Velocity)<0时以其计算lean，最大值为0时目标lean为0。
  - Target：`ab_als_provider.rs:330–342`无论运动模式均发布GroundedDynamics；实际Custom时仍为12.5m/s²。`source_native_refresh.rs:185–203`把它传给Grounded lean，Standing的BS_Als_Lean读取当前结果。
  - 反例：完整角色进入实际High Mantle，第2帧dot(V,A)=−48.811157、PostLocomotion权重0.074074，Grounded回调仍写入当帧。目标lean forward=0.053087，源同输入应0.023253；只将该字段改为0，值即对齐。两帧×恢复/原样共4组3PASS/1FAIL；正点积帧数值和Lean姿态完全相同，负点积帧实际Blendspace输入及输出变化，最大骨骼角差0.013047rad。
  - 漏检：旧测试只使用Grounded模式，并用provider自己发布的最大值计算期望；没有经过Custom的源getter和仍有权重的地面consumer。fresh原生High Montage混入0.2秒、slot不强制更新source，但本帧source权重非零；Air的惯性切换不复用这个证据。证据`68/checked_initial/`、`68/final/`及`bf144_animation_phase68_custom_braking_uses_source_mode`；最终原生等价姿态尚未验收，未修复生产。

- [ ] BF144-D151 / P3 — 把局部投影长度当作世界水平速度，改变移动标志与Standing状态选择。
  - Source：fresh `AlsCharacter.cpp:1375–1389`从GetVelocity的世界XY分量计算Speed，再以1cm/s判断HasVelocity；`1435–1436`与动画`712–716`按实际50/150cm/s阈值计算moving/moving smooth。该speed不依赖actor旋转；当前原生Standing Idle→Move直接绑定bMovingSmooth。
  - Target：`tnua_ext/motion.rs:41–45`先向角色forward/right投影，再取二维长度；`actual_motion.rs:40–44`把此长度作为horizontal_speed，provider再发布speed和移动标志。倾斜时会丢失分量，纯yaw时的舍入也可能跨越严格阈值。
  - 反例：完整角色先等到实际Standing Idle，在相同世界velocity=(0,0,−1.6)m/s、无输入下控制actor俯仰60度，目标发布0.8m/s并保持Idle；源应1.6m/s并开始Idle→Move。仅恢复horizontal_speed即进入原生对应转换，当前actor旋转与速度均已核实。直立/倾斜×原样/恢复四组3PASS/1FAIL；另11组输入边界中0.5及1.5m/s各出现一处误判，同根归并。
  - 漏检：旧旋转测试只用yaw且仅比较速度误差容差，未检查严格布尔阈值或倾斜输入；初版诊断误把Gameplay Grounded当作Standing Idle，已补实际状态等待并排除该前置失败。证据`68/checked_initial/`、`68/final/`及`bf144_animation_phase68_speed_uses_world_plane`、`bf144_animation_phase68_motion_inputs`；未改生产或放宽阈值。

- [ ] BF144-D152 / P2 — 地面预测复用物理形体，遗漏源胶囊组件的查询尺寸。
  - Source：fresh `AlsAnimationInstance.cpp:785–789,1166–1188`从CapsuleComponent读取scaled radius/half-height，再用Identity旋转构建独立查询胶囊；`CapsuleComponent.h:201–211`半径取水平两轴较小值、半高取竖直尺度。这不同于D11所述物理形体的水平大轴规则。
  - Target：`locomotion_layer.rs:159–163,196–202`直接把角色Collider传给地面预测；Avian查询读取其shape_scaled。横向scale=(2,1,1)时，查询半extent为(0.6,0.9,0.285317)，源应为半径0.3m、半高0.9m的圆胶囊。
  - 反例：完整角色实际Fall、中心高3m、vy=−10m/s，平台边缘距中心0.4m。原生源胶囊不命中，目标命中并发布GroundPredictionAmount=0.345407，活动Fall混合节点读到该值；仅把当次查询形体替换成源尺寸即恢复0。unit/nonuniform×平台居中/边缘×原样/恢复共8组7PASS/1FAIL，全部恢复与unit控制通过。
  - 漏检：旧查询测试覆盖uniform缩放和当前位置，并以同一物理Collider复算期望，未独立读取组件getter。额外原生半径0.6m控制仍命中该平台，证明单独修复D11的物理圆截面不能修复此输入边界。证据`70/queries.native.json`、`70/geometry_initial/`与`bf144_animation_phase70_prediction_capsule_input`；仅登记，未改生产。

- [ ] BF144-D153 / P3 — 缓存Pose求值顺序丢失源队列，Default Overlay的预测输入滞后一帧。
  - Source：fresh AB_Als_Layering原生cached-pose队列共11项，Locomotion位于Overlay之前；两个输入分别只进入自己的SaveCachedPose。`AnimNode_SaveCachedPose.cpp:111–125,178–201`先登记请求，再在PostGraphUpdate更新源；`AnimInstanceProxy.cpp:150–158`按原生队列执行，因此Locomotion内RefreshInAir先刷新父状态，Default Overlay的TwoWayBlend再读取当前GroundPredictionAmount。
  - Target：`source_cached_pose.rs:86–105`按收集顺序立即resolve请求，未消费源编译队列。完整角色中Fall节点已读到当帧预测，而Default Overlay节点`adf49347-4a01-b4bd-6434-518242a029cb`仍读上一帧的父字段。
  - 反例：固定源已核验的世界速度(−6,−40,−6)、mesh scale=2和原生平面查询结果，仅切换GroundPredictionBlock。解除屏蔽时source=0.8923675、Overlay actual=0；重新屏蔽时source=0、actual=0.8923675。仅在求值前发布同一个当前父字段，两次切换均恢复；连续不变的两帧本已通过，共8组6PASS/2FAIL，Fall当前值全程正确。
  - 漏检：旧字段与图绑定检查只核对帧末父状态，恒定输入又掩盖跨图的旧值；把全部同名读者与帧末值直接比较也不能定位执行顺序。证据`70/layering_schedule.native.json`、`70/overlay_initial/`及`bf144_animation_phase70_cached_overlay_order`。本项只确认当前Default Overlay预测链，其他缓存副作用消费者留核心图审计；未修改转换器或运行时。

- [ ] BF144-D154 / P3 — Feet恢复有效时清空锁定权重，丢失源保留的淡出。
  - Source：fresh `AlsAnimationInstance.cpp:1251–1275,1364–1392`只改变有效性并将已有锁定位置重新基于当前目标；`1398–1523`保留原LockAmount，当前曲线更低时继续淡出。`GetControlRigInput:252`消费最终组件空间位置。
  - Target：`foot_ik_apply.rs:640–642`在became_valid时无条件调用`reset_foot_lock_to_target`，该函数同时将LockAmount归零；后续部分权重不允许重新淡入，因而整个剩余锁定丢失。
  - 反例：完整角色恢复前LockAmount=0.8、当前曲线0.5，实际权重变0而source应为0.5；下一帧足目标沿X移动8cm，实际Rig的RigInput位置比源多移动4cm。只在恢复帧补回源权重，下一帧消费恢复。原权重0与当前曲线1的控制通过，12组10PASS/2FAIL。
  - 漏检：旧`reset_foot_lock_to_target`测试直接期待恢复时权重0；已有pending测试只核对有效性与Rig开关，没有检查恢复后的历史权重和位置。证据`71/initial/`、`bf144_animation_phase71_revalid_lock_weight`；仅登记，未改实现。

- [ ] BF144-D155 / P2 — Feet使用矩阵逆变换，非均匀缩放加旋转时改变源组件空间输入。
  - Source：fresh `RefreshFeet:1308–1313`先建立`ComponentTransform.Inverse()`，`RefreshFootLock:1430`等再调用该逆Transform的TransformPosition；UE `TransformVectorized.h:1423–1452,1675–1693`对应`inverseRotation × ((point−translation) / scale)`。源算法自身的QST顺序必须保留。
  - Target：`foot_ik_apply.rs:678–681`经`ik_space.rs:37–46`使用仿射矩阵逆变换，先逆旋转再除scale；非均匀scale与旋转不能交换。
  - 反例：完整角色mesh scale=(1.4,0.8,0.6)、actor yaw=0.7，Foot Lock关闭以隔离其他算法，实际Rig输入为(0.196194,0.134653,0.073582)，source为(0.284042,0.134653,−0.072761)，相差0.170685m。仅恢复该输入字段即通过；unit、uniform1.5与零yaw控制均通过，共8组7PASS/1FAIL。
  - 漏检：旧空间测试验证数学往返或uniform scale，没有以源Inverse QST顺序复算实际Feet读者。证据`71/initial/`、`bf144_animation_phase71_component_inverse`；本项只登记Feet直接消费者，不修改通用空间helper或其他调用者。

- [ ] BF144-D156 / P3 — 初始瞬移时间用None代替源零值，丢失世界启动时的Foot Lock重定位窗口。
  - Source：fresh AB_Als CDO与`AlsAnimationInstance.h:58`均为TeleportedTime=0；`ProcessFootLockTeleport:1340–1347`在Feet已有有效历史、IK×Lock相关且World.TimeSince(0)≤0.2时重建世界锁定位置。UE `World.h:4714–4716`以游戏时钟直接相减，没有“尚未瞬移”例外。
  - Target：`FootLockLifecycleState:30–34`默认teleported_at=None，`foot_ik_apply.rs:613–620`只在Some时启用窗口，直到收到事件才存在源对应时间。
  - 反例：完整角色保留有效锁定，受控世界时钟0.1s、当帧移动10cm且未超过自动瞬移阈值，source应跟随组件重定位，实际Rig输入多出10cm偏移、rebase次数0。通过公开事件在游戏时间0建立源时间后，同输入恢复；0.3s/1s及恢复控制全部通过，共6组5PASS/1FAIL。
  - 漏检：旧时钟测试先显式发送事件，再验证超时与暂停，未覆盖默认时间0的有效分支。证据`bf144_animation_phase71_initial_teleport_window`及本批最终记录；初始夹具混用当前组件与上帧组件的恢复失败已排除，最终判据用真实上帧组件历史。仅登记，未改生产。

- [ ] BF144-D157 / P3 — Feet无效时provider覆盖源应保留的曲线历史，改变起跳脚选择。
  - Source：fresh `RefreshFeet:1299–1305`在bValid=false时立即返回，保留FootPlantedAmount/FeetCrossingAmount；原生Locomotion的Jump Entry Conduit以FootPlantedAmount≤0进入Left Foot，>0进入Right Foot。
  - Target：`ab_als_provider.rs:139–149,424`直接按最新RootAnimationContext生成这两个字段，绕过`AlsFootLockState`已正确保留的历史；实际SourceScalarExpression的global回退读到该新值。
  - 反例：完整角色在source明确判为无效的identity pelvis条件下起跳，FeetState原值−0.8、最新曲线+0.8时，实际Jump选择Right Foot而源应Left Foot；反号同样选错。仅发布保留字段即恢复两种选择，有效Feet四个控制通过，共8组6PASS/2FAIL，均在第一帧实际Jump FSM记录权重。
  - 漏检：旧曲线provider测试只验证钳制值，未加入Feet有效性和实际Entry选择；原pending首帧不执行Jump脚选择的尝试属于前置不满足，未计缺陷。证据`71/guard_checked/`、`bf144_animation_phase71_invalid_feet_jump_consumer`；仅登记，未改图或provider。

- [ ] BF144-D158 / P3 — pending帧跳过足目标采集，动态过渡消费旧锁定距离。
  - Source：fresh `RefreshFeetOnGameThread:1251–1294`在pending时只使Feet无效，随后仍读取当前两个足目标；`RefreshFeet`跳过锁定计算，已有世界锁定点保留。`RefreshDynamicTransitions:1694–1736`不要求Feet有效，继续以当前TargetWorld与旧LockWorld的距离判定；原生Standing Idle的On Update调用该方法。
  - Target：`foot_ik_apply.rs:559–567`在pending时直接continue，足目标保持旧帧；`ab_als_provider.rs:375–378`据此生成锁定距离，`source_native_refresh.rs:361–389`实际消费并排队过渡。
  - 反例：完整角色实际Idle，左脚LockAmount=1、右脚0，pending帧组件移动20cm且未超过瞬移阈值；source距离平方0.04m²超过0.0064m²，target发布0、当帧不排队。只恢复该距离字段，真实回调写入StandingLeft序列；零位移与非pending控制通过，共6组5PASS/1FAIL。
  - 漏检：旧pending测试只检查Rig关闭，未核对GameThread继续采集的目标及直接动态过渡消费者。证据`71/pending_checked/`、`bf144_animation_phase71_pending_target_distance`；回调执行及当帧队列write-frame均已断言，未把旧队列值计作触发。仅登记，未改生产。

- [ ] BF144-D159 / P3 — Rig射线查询丢弃恰好位于距离上限的有效命中。
  - Source：fresh `Raycasts.cpp:75–79`只在LatestStartTime>RayLength时拒绝，SQVisitor直接接收该有效ray命中；原生box ray在长度100cm、命中距离100cm时返回true，前后0.01cm控制分别命中/不命中。
  - Target：`control_rig_query.rs:50–53`在Collider.cast_ray已经返回命中后，再要求length<distance；等于当前最大长度且尚无命中时也被丢弃。
  - 反例：实际查询使用Rig scale=1.25、默认向上0.5/向下0.8m，底面恰在世界−1m；Parry返回distance=1.625m，查询wrapper返回None。完整角色中实际ApplyFootOffsetLocation读到0而source为−0.766336m；夹具仅将原生合同偏移量写入现有变量Copy，实际消费者恢复。稍近底面控制通过。
  - 漏检：旧query测试只覆盖区间内部与完全未命中，没有比较primitive已命中却被wrapper丢掉的端点。证据`72/ray.native.json`、`72/query_initial/`、`72/full_initial/`；本项仅验证trace与偏移输入，完整弹簧/IK姿态另行验收。

  - BF144.74补充：实际Stairs复杂网格的未缩放终点同样失败，Parry BVH自身已用严格best_cost排除命中，不能只改外层query比较。默认FootTrace输入进入该mesh时source偏移−0.766336m、target为0。源缩放包装另有严格NewTime<Length，相关缩放端点本身应拒绝；12组端界不能用统一放宽距离处理。证据`74/endpoint.typed.native.json`、`bf144_animation_phase74_mesh_endpoint_and_initial_hit`。

- [ ] BF144-D160 / P2 — Rig初始重叠射线未转换零法线，被误判为不可行走。
  - Source：fresh `SQVisitor.h:34–38`对零距离ray撤掉位置/法线标志；`CollisionConversions.cpp:403–432`使用起点和反向射线的安全归一化法线，FootOffsetTrace继续按bBlockingHit消费。原生box内部射线Time=0、raw normal=0，经过源转换应为向上法线。
  - Target：`control_rig_query.rs:53`直接转发Parry的零法线；`control_rig_units.rs:113–118`得到slope_cos=0并清空偏移，漏掉源命中转换。
  - 反例：完整角色的脚侧查询盒覆盖trace起点0.5m，query确实返回起点与零法线，实际偏移消费者为0而source应为+0.5m。仅恢复偏移变量即通过；把同一盒顶面降到起点表面的源等价控制也通过。查询单元与完整角色均复现，未用放宽坡度阈值掩盖结果。
  - 漏检：旧Rig trace单元测试传入预制非零法线，query集成测试从形体外开始，未串起inside-ray→命中转换→walkability。证据同D159；未修改生产query或Rig实现。

- [ ] BF144-D161 / P2 — Foot Trace复杂网格射线遗漏实际单面剔除，背面命中进入脚底偏移。
  - Source：fresh `ChaosInterfaceUtils.cpp:325`从BodySetup双面字段设置culls_back_face，12种实际Playground复杂mesh均为False/True；`TriangleMeshImplicitObject.cpp:169–181`以Dir·TriangleNormal>0拒绝背面。源FootOffsetTrace的Visibility复杂查询直接消费该结果。
  - Target：`playground_collision.rs:229–252`建立普通Parry trimesh；`control_rig_apply.rs:437–442`正确选用复杂几何及缩放，但`control_rig_query.rs:42–50`调用默认双面cast_ray，未传入源剔除规则。此处是ray独立算法，不归并接触生成D140。
  - 反例：完整角色实际CR_Als，桶底置于−0.3m、脚底射线起于+0.5m且位于桶内。原生无命中，目标命中底部背面并将−0.261963m送入ApplyFootOffsetLocation；scale=1.5时同根偏移−0.542933m。只在夹具按当前ray方向剔背面，两个实际消费者均恢复0；正面和非均匀缩放控制通过，8组6PASS/2FAIL。
  - 漏检：旧复杂几何测试以同一个Parry trimesh作为期望，只证明没有误选简单几何，未独立读取源单面属性或源ray核。972条实际mesh查询有165组由culling单因素恢复，另8组法线余差仍单独核验；证据`73/matrix.native.json`、`73/matrix_initial/`、`73/full.native.json`及`bf144_animation_phase73_mesh_rays_reach_foot_offset`。只登记，生产与原始资产未改。

- [ ] BF144-D162 / P3 — 复杂网格射线在共享边/顶点邻域选择不同面，法线差异进入足部旋转与偏移输入。
  - Source：fresh `Triangle.h:375–423`使用double计算及1e−8重心容差；`TriangleMeshImplicitObject.cpp:181–188`接受当前float长度内的候选并覆盖结果。实际Fountain顶点依次接受外部面281、285、286，281/286含很小的负重心分量，最后286虽比285远约3.17e−10cm，仍在float长度内。native逐面记录与原查询逐值相同。
  - Target：Parry `ray_triangle.rs:85–159`使用float与严格非负重心条件，`bvh_traverse.rs:362–440`按严格更短距离保留候选；生产查询返回另一面法线。背面剔除单独恢复不能消除该边界，不能用点位置接近代替法线合同。
  - 反例：完整角色实际CR_Als，FootTrace精确源/目标同输入，Fountain顶点的单位/1.5/非均匀缩放三组全部把不同法线送入ApplyFootOffsetRotation。非均匀组source normal=(−0.166315,0.972883,−0.160743)，actual=(−0.105964,0.940714,−0.322224)，脚底偏移由source 0.203753m变为0.208486m。三组偏离顶点1cm的控制通过；仅恢复当次trace两个输出，三个实际消费者均通过。
  - 棱边补充：两个Arch02诊断射线仅改用同一Parry核的源厘米/Core坐标即选回native面56/231，确认米制float变换输入会改变近边资格；默认50/80cm完整Rig在其中一处实际选面50而源为231，法线进入足旋转输入，源输出恢复及1cm内点控制通过。完整Rig最终16组12PASS/4FAIL，四个失败均恢复；旋转/弹簧算法及最终姿态验收另项继续。
  - 漏检/范围：旧测试核对命中位置或大平面的法线，未覆盖共享边附近的候选接受与距离收紧。证据`74/visits.typed.native.json`、`74/expanded_full.native.json`及`74/final/`。原五条长射线映射实际Rig距离后会被坡度门槛拒绝，不把这些raw法线差异重复计作运动缺陷；早期远处缩放夹具的11µm位置干扰已用逐顶点完全相同的输入排除，未作生产缺陷或阈值调整。

- [ ] BF144-D163 / P3 — 足旋转在Euler半转边界未保持源计算分支，角度表示差异进入限位与最终脚骨。
  - Source：fresh `AlsRigUnit_ApplyFootOffsetRotation.cpp:49–97` 将当前/目标转到小腿空间，降为float Euler后用包含当前动画角度的区间约束；`UnrealMath.cpp:691–718` 的double Rotator保留±180°分支。900个精确输入的原生转换和实际参考骨架连乘均已核验；source不是把两个半转表示统一规范后再约束。
  - Target：`control_rig_math.rs:33–52` 的等价代数在有符号零处可选相反Roll端点；`control_rig_units.rs:268–278` 的目标坐标四元数组合又可因double舍入跨过Roll/Yaw半转分支。`constrained_angle:38–51` 随后使用不同的当前端点扩展区间，使微小表示/舍入差异成为大角度输出差异。两条数值路径不能简化成单改一个Euler符号即可全部修复。
  - 反例：采用真实ALS初始calf/foot参考旋转，受控post-IK半转姿态的源当前Euler为约(0,180,180)；左脚目标Roll177.4759°在源区间保留，目标区间却裁为0。完整角色使用实际30°坡面查询及float FootTarget输入，最终左/右脚分别偏离177.4759°/130.7921°；仅以原生合同结果替换本次旋转输出，两个实际SetRotation消费者恢复。八个单元反例失败，16个±0.0001°邻域控制及5720次一般单元步骤通过。
  - 边界与漏检：反例控制了IK后的有效旋转输入，不是默认动作自然触发证据；TwoBoneIK数学已在.78–79核对，.80的启动/站立方向/转向瞄准/蹲姿矩阵未触发，其他动作与历史仍未证明。旧足旋转测试使用相同target Euler/Quaternion helper构造期望，覆盖的是普通角度、参考缓存和法线状态，没有原生半转分支或组合后的数值边界。证据`76/math.native.json`、`76/products1.native.json`至`products6.native.json`、`bf144_animation_phase76_half_turn_reference_candidates`、`bf144_animation_phase76_full_half_turn_weighted_consumers`。未修改生产实现或转换规则。

- [ ] BF144-D164 / P3 — TwoBoneIK在距离退化边界跨过源判断分支，改变膝／肘的求解方向。
  - Source：fresh `TwoBoneIK.cpp:84–94`在double厘米距离小于1e−4时才回退到X轴，否则按目标方向归一化；后续弯曲平面和余弦定理直接消费该方向。288组边界输入由原生`SolveBasicTwoBoneIK`逐值确认，包含相等／不等骨长、原点偏移及共线pole。
  - Target：`control_rig_ik.rs:88–110`在米制反射坐标中计算距离，阈值换算为1.0000000000000002e−6。代表输入的源距离恰为0.0001cm、目标为0.000001m，source保留实际方向而target回退X；其他原点/方向组合也会因差值和求和顺序跨过该不连续边界。不能用一般坐标基公式正确来证明边界分支一致。
  - 反例：完整角色现有左右腿IK，采用真实参考骨长、1.25缩放和受控近根effector，最终膝位置分别偏离0.6442049m/0.6442081m。只写回本次原生求解的joint/end位置，两个实际消费者恢复；距离上下0.1%控制通过。全部16组完整角色输入（含另一项的伸直边界）已再次原生核验，独立公式逐值一致。
  - 漏检/边界：旧测试覆盖普通弯曲、远目标和零权重，未比较精确距离门槛两侧。此为受控IK输入反例，尚无默认动作自然触发证据；上游FootOffsetLocation的距离限制与历史仍影响可达性，不能据此泛化为常见站姿故障。证据`78/positions.native.json`、`78/full.native.json`、`bf144_animation_phase78_position_kernel_boundaries`与`bf144_animation_phase78_full_boundary_consumers`；仅登记，未改生产阈值或求解器。

- [ ] BF144-D165 / P3 — TwoBoneIK自动骨长在米制float提前舍入，近伸直时放大关节位置差。
  - Source：fresh `RigUnit_TwoBoneIKSimple.cpp:161–189`以double厘米参考骨段乘当前/初始缩放，再把长度写入float；位置核随后按double消费该float厘米长度。四个实际调用的显式长度均为0，都会进入自动计算。
  - Target：`control_rig_units.rs:377–394`直接把米制长度转为float，`control_rig_ik.rs:167–172`再升为double求解。转换单位和float舍入不交换，细小长度差在完全伸直附近变成不同的残余弯曲量；源与目标均让B继承A缩放，该共同规则不是根因。
  - 反例：480组实际调用数据中5组超出10µm位置合同；完整角色的左腿非均匀缩放与右腿1.25缩放两组，膝位置分别偏离0.1035288mm/0.0702093mm。只恢复本次原生joint/end位置即通过，目标距离上下0.1%控制通过。160组有效单元输入及16组完整角色输入的源公式全部与原生逐值相同；将同一个目标位置核放回源坐标/厘米float输入后，5组位置偏差全部消除。
  - 漏检/边界：旧测试用整数骨长或普通弯曲，未覆盖从实际参考骨架重算长度后紧贴总骨长的输入。差异为数值边界，当前反例控制了缩放和IK目标；.80公共矩阵的5782次有效足IK与原生核验未触发D164/D165，post-spring限位在这些样本中保持伸直余量。该结论不覆盖其他动作、缩放或历史，双臂权重为0也不构成有效手臂求解证据。证据`78/units.native.json`、`78/full.native.json`、`80/summary.json`与`bf144_animation_phase78_unit_lengths_spaces_and_guards`；仅登记，未加精度补偿。

- [ ] BF144-D166 / P3 — 动态过渡的锁定距离精度与单位舍入改变资格或左右脚选择。
  - Source：fresh `AlsAnimationInstance.cpp:1716–1756`以double厘米世界坐标计算两脚锁定距离平方，和float厘米阈值平方比较；同时合格时取距离较大脚，严格相等才取左。实际配置为8cm，Standing/Crouching的Idle均调用此函数。
  - Target：`ab_als_provider.rs:375–378`把米制`Vec3::distance_squared`作为F32发布，`source_native_refresh.rs:361–378`比较float米制阈值并选择左右。距离平方的低位被舍入后，不等距离可变成相等；单位换算与float平方也使8cm附近的资格不同。
  - 反例：共同有效输入左距平方100.00000298023227cm²、右距平方100.00000299023226cm²，源选择右，目标两值均变为0.010000001m²而选择左。另一个30°方向输入源距平方63.99999962647951cm²未超过64，目标却触发。两种站姿实际Idle图和队列消费者均复现，84组控制/恢复中6处原始失败；只恢复距离资格和排序的标量输入后42组恢复全部通过，四资源、blend 0.2、rate 1.5、start 0字段一致。
  - 漏检/边界：旧测试使用明显超限、整数比例或严格相同距离，没有在源double与target float之间比较微小差值。本项控制了消费时的锁定点，尚未证明默认动作自然触发；已登记D158的pending输入历史未复开。证据`81/static.json`、`bf144_animation_phase81_distance_boundaries`和`bf144_animation_phase81_actual_queue_consumer`；仅登记，没有补偿距离、放宽阈值或修改生产代码。

- [ ] BF144-D167 / P3 — 动态过渡的目标时长丢失float精度，使临界帧提前混出。
  - Source：四个动态过渡Sequence经fresh `GetPlayLength()`和原生动态Montage构造均为1.5333333015441895秒；`AnimMontage.cpp:2626–2637`用实际剩余时间与1e−4秒窗口判断自动混出，当前源BlendOutTriggerTime为0。
  - Target：`als_standing_montages.ron`保存1.533333，解析为float的1.5333329439163208；`stop_link.rs:214–223`用此值初始化站立动态过渡，缩短约0.36µs。蹲姿播放状态使用1.5333333常量，故该处未缩短，但所有四个槽采样仍读取截断后的配置时长。
  - 反例：先推进1秒，再推进0.0221220143秒，源位置1.5331829786尚未到混出窗口，站立目标已开始混出。左右站立单元及实际角色均复现；随后10/50ms两帧的最大权重偏差约0.05975，仅将本次播放时长恢复原生float后，触发帧与权重偏差均恢复。蹲姿同输入和窗口之后控制通过。另一时钟反算分支已归D118，不把数值抵消误认为源等价。
  - 漏检/边界：旧时钟测试直接引用相同六位小数时长，普通帧长也不会落进这个窄区间。本项为受控有效动画时钟输入，未宣称默认动作自然触发；证据`82/boundary.native.json`、`bf144_animation_phase82_trigger_boundaries`和`bf144_animation_phase82_actual_trigger_boundaries`。仅记录，未改原始导出、目标配置或转换器。

- [ ] BF144-D168 / P3 — Transition/Turn槽遗漏源实例求值资格，小权重仍改变运动曲线。
  - Source：fresh `AnimInstance.cpp:4648–4669`先仅向MontageEvaluationData提交weight>1e−5的实例，`AnimInstanceProxy.cpp:2105–2148`再合计这些实例的槽权重；`SlotEvaluatePose:1897–1904`在总权重≤1e−5时直接保留基础Pose/Curve。四个当前动态过渡的源`AllowTransitions`初值均为−1，普通相关权重才按加性规则进入基础曲线。
  - Target：`stop_link.rs:625–649`从>1e−6的单实例开始预混Pose与曲线，`711–718`随后向生成槽发布weight/layer_weight=1；`source_slot.rs:112–149`因此继续消费预混曲线，源总槽相关性判断已无法从该输入恢复。
  - 反例：真实角色的Standing/Crouching均保持稳定Idle，以共同有效基础曲线0.999992及单实例权重3e−6测试，源槽输出仍0.999992，目标为0.99998903；实际下一帧`transitions_state.b_transitions_allowed`由应为true变false。权重1e−5也复现；基础曲线1的控制只出现曲线偏差。64组中8FAIL，32组仅恢复源门槛的对照全部PASS；零权重、0.5e−6、1.1e−5及普通权重控制通过。
  - 漏检/边界：旧测试只比较普通混合量，未覆盖总权重不相关分支。此处控制了槽输入曲线和权重，尚未声明默认动作自然触发许可翻转；原生0.2ms淡入确可产生约3e−6有效权重。BF144.91复核补齐上游资格：先剔除不相关实例，再按剩余实例计算总权重及归一化；旧“必须保留小权重参与总和”的判断作废。证据`83/*.native.json`、`bf144_animation_phase83_actual_slot_weight_threshold`；仅登记，未修改生产混合或阈值。
  - BF144.84多实例补充：fresh `AnimationRuntime.cpp:1285,1332`与`AnimCurveTypes.h:821`还对各层Pose/Curve执行>1e−5的相关性判断。BF144.91重新核对上游后确认，双9e−6层均不进入源MontageEvaluationData，源槽总权重为0；目标两层都进入预混。四资源实际配对的8个输入中，曲线最大差约1.8e−5、位置约2.47µm、四元数距离约2.90e−6；未额外修改基础曲线，下一帧的过渡许可也全部由应为true变false，仅恢复源合成通过。原176组实际槽及60组局部比较在补齐上游资格后判定均不变。
  - BF144.91 Turn补充：`turn_layer.rs:473,593–609`仅按f32::EPSILON过滤Pose，而曲线上下文仍使用全部已采样实例。实际站蹲槽的单9e−6、单1e−5及双9e−6共52组均失败，最大曲线差0.0020031242；全部源过滤恢复通过，1.000001e−5和零权重控制通过。此处是原始实例不具备求值资格，与D179归一化后的曲线累加门槛不同。

- [ ] BF144-D169 / P3 — Transition/Turn槽在总权重略高于1时提前归一化，丢失源容差区间。
  - Source：fresh `AnimInstanceProxy.cpp:2016–2031`只在`InTotalNodeWeight > 1 + ZERO_ANIMWEIGHT_THRESH`时归一化各Montage，当前阈值为1e−5。`GetSlotWeight`把节点显示权重夹到1，不会改写保存的总权重或各层权重。
  - Target：`stop_link.rs:640–648`使用`total.max(1.0)`作为分母，任意正超额都会缩小姿态和曲线；不是源的带容差条件。
  - 反例：两层权重0.6/0.400005，源保留原权重，目标先除以约1.000005。实际Standing/Crouching四资源配对与0.25/0.4秒采样的8个输入，槽曲线最大差约5.04e−6；只恢复源合成全部通过。0.6/0.40002及0.9/0.8等确应归一化的控制通过。
  - 漏检/边界：旧组合测试使用总值≤1或明显超额，没有覆盖(1,1+1e−5]。本项为共同有效权重输入的边界证据，不宣称默认动态过渡自然产生该精确总值；证据`84/extended.native.json`与`bf144_animation_phase84_actual_multi_slot`，未改生产算法。
  - BF144.91 Turn补充：预混先除总权重，随后`turn_layer.rs:487`将发布权重夹到1，同样提前消去微小超额。0.6/0.400005在站蹲、左右/90/180及相同姿态配对的20组全部失败，最大曲线差0.0005493164、scale向量差8.672015e−6；仅把旋转改为nlerp仍全部失败，源统一加权恢复全部通过。0.6/0.40002的权重、位置、scale及曲线控制通过，旋转差由D177单独覆盖。

- [ ] BF144-D170 / P3 — 加性旋转在源近全权重区间仍插值，未保留full-weight分支。
  - Source：fresh `AnimationRuntime.cpp:1288–1312`在权重≥1−1e−5时走AccumulateWithAdditiveScale；Quaternion整体乘权后归一化，因此旋转方向等于完整加性旋转，平移/缩放仍按权重。实际C++/ISPC实现与50个公开原生累积样本已核对。
  - Target：`mesh_space_additive.rs:30–40`在所有权重下都与identity插值，包括0.99999/0.999995；已接近全权重仍略微缩小旋转。
  - 反例：真实动态过渡0.25/0.4秒姿态、权重0.99999/0.999995，实际根槽四元数距离约0.7–1.62e−6；源分支恢复降到约0.18e−6，权重1及0.99998控制通过。一般2e−6姿态合同不够敏感，专项合同依据源近全权重旋转不再插值的规则区分，未调整生产阈值。
  - 漏检/边界：旧“组合等于顺序应用”测试两边使用同一target helper，共同遗漏该源分支；大角度单元反例也由原生调用确认。本项为细小旋转差，不扩大为可见姿态故障；证据`84/accumulate.native.json`、`84/extended.native.json`及`bf144_animation_phase84_composition_boundaries`，未修复。

- [ ] BF144-D171 / P2 — Transition再次替换同组Turn时重启旧淡出，延长旧姿态的消费。
  - Source：fresh `AnimInstance.cpp:3671–3695`对同组实例调用Stop；`AnimMontage.cpp:1632–1642`在已混出状态仅接受更短时长，相等或更长请求不重置进度。Skeleton原生组映射确认Transition、TurnInPlaceStanding和TurnInPlaceCrouching均为Grounded；当前替换混入时长为0.2秒。
  - Target：`turn_layer.rs:264–270`的`stop_for_group_replacement`无条件把当前权重作为新起点、elapsed归零；`stop_link.rs:324–332`每次播放Transition都会调用它，因此同样的0.2秒请求也延长旧Turn。
  - 反例：实际Standing/Crouching各启动一个Turn，再通过原图Dynamic回调/dispatcher播放两次Transition，第二次发生在旧淡出0.15秒时。source下一帧结束，目标随后三帧仍有0.13183594、0.078125、0.024414063权重，生成Turn槽直接消费这些值。只保留第二次请求前的旧淡出进度，两侧都恢复；单次替换控制通过。60个测量帧中6个原始失败。
  - 漏检/边界：旧测试只核对第一次同组替换或最终清理，没有在已混出时重复同组播放。本项控制了请求资格/时点，使用当前0.2秒设置，未宣称默认动画曲线会自然允许该精确组合。证据`85/turn_standing.native.json`、`85/turn_crouching.native.json`及`bf144_animation_phase85_repeated_group_replacement`；仅登记，未修改生产逻辑。

- [ ] BF144-D172 / P2 — Turn Slot未更新时，停止请求仍对当前移动姿态启动惯性混合。
  - Source：fresh `AnimNode_Slot.cpp:60–71`只在Slot的Update中把组请求交给当前惯性化消息；`AnimInstance.cpp:4671–4675`每帧复制并清空请求。两种原生Turn Slot均位于Idle，父Inertialization位于整个站蹲状态机之后；移动分支不更新Idle Slot时，不应由这次停止产生该父节点的惯性化请求。
  - Target：`ue_animgraph_converter.py:1357–1390`按状态机内部存在Slot就把epoch绑定到父惯性化节点；`turn_layer.rs:490–501`向两种stance持续发布全局epoch，`inertialization_passthrough.rs:507–512`直接消费其增加，未保留Slot当帧更新资格。
  - 反例：完整角色由实际输入进入移动，核实Turn Slot未更新且FSM无惯性化请求；使用已由真实回调创建的Turn实例作为共同受控输入，再显式停止。原样站立/蹲姿各有3帧错误混合，首帧骨骼局部位置最大偏移约6.48/9.12cm、旋转约25.47°/18.98°；只清除该epoch绑定的14帧控制全部恢复当前输入姿态。正常Idle显式/自然停止及移除请求的56帧对照通过，当前RotationYawSpeed过滤断言也通过。
  - 漏检/边界：旧测试只检查生成图绑定键及Idle停止，未覆盖状态机仍更新而内部Slot不更新的情况。证据`86/standing.native.json`、`86/crouching.native.json`、`86/analysis.json`及`bf144_animation_phase86_irrelevant_slot`；请求存在与停止时点受控，不宣称默认动作自然触发，亦未以请求链验证代替完整惯性求解器等价。仅登记，生产、转换器和资产未改。

- [ ] BF144-D173 / P2 — LandToGrounded误用其他过渡的参数，并固定选择站立动画。
  - Source：fresh AB_Als_Locomotion的`AnimNotify_LandToGrounded`直接调用`PlayTransitionRightAnimation`，原生节点`183f8499-4c3b-6ddd-3000-fe957622821c`参数为混入0.1秒、混出0.2秒、速率1.4、起点0、StandingIdleOnly=false。`AlsAnimationInstance.cpp:1650–1661`按当前Stance选择站立或蹲姿Right Sequence。
  - Target：`transition_notify.rs:352–357`调用固定`standing_transition_right()`；`stop_link.rs:151–161`给出混入0.2秒、混出0.2秒、速率1.5、起点0.2，且不读取当前stance。这既改变新动画的采样与混入，也以错误0.2秒延长同组旧动画的混出。
  - 反例：实际角色跳跃、经历Air并落地后，无通知注入的真实回调得到上述错误参数。另在站蹲各6帧的受控单事件消费中，首个采样帧source应为0.07秒/权重0.5，actual为0.275秒/0.15625；旧动画应为0.5，actual为0.84375。蹲姿还采样了Standing Right。仅把事件改接原生参数和当前stance选择，12帧恢复全部通过，根Transition Slot实际消费的20个样本也核对通过。
  - 漏检/边界：旧通知测试核对事件计数与现有构造器，未逐个读取当前蓝图调用实参。85组比较72PASS/13FAIL包含上述12个受控失败和1个实际跳跃落地失败；60帧队列保留/覆盖/停止阻挡控制通过。证据`87/locomotion.native.json`、`87/native_check.json`、`87/analysis.json`及`bf144_animation_phase87_natural_landing`。最初多事件“最后调用覆盖”猜测被参数错误混淆，不另计缺陷；只登记，未修生产。

- [ ] BF144-D174 / P3 — Quick Stop的yaw重新计算在0°/175°边界改变左右动画选择。
  - Source：fresh `AlsAnimationInstance.cpp:1590–1609`先以float输入/目标yaw减去double `RotationWorldSpace.Yaw`，将差降为float后Unwind，再以`>175°`逆时针重映射和`<=0°`选择左侧。RotationWorldSpace来自ActorTransform的double Rotator；45个共同旋转的UE原生输出已核验，直接选择规则405组通过。
  - Target：`stop_link.rs:399–414`从float Quaternion的forward重新取yaw，并把输入/目标经弧度往返再转回float角度；`transitions.rs:135–145`在这些舍入后的值上做相减/分支。源和目标阈值本身相同，差异发生在阈值前的表示和运算路径。
  - 反例：源body yaw=−175°、所选yaw=0°时，源剩余角为175°，应选Right、速率约2.9652777；目标舍入后跨过175°，改选Left、速率约3.0347221。相同body与所选yaw=−175°时，源应选Left，目标却选Right。实际站蹲×输入/目标两条来源的12组均在根Slot消费错误Sequence；仅恢复源index/rate的28组全部通过，正负邻域及非VelocityDirection控制通过。
  - 漏检/边界：旧选择测试直接输入角度，只覆盖整角度普通分支，没有经过真实Quaternion→heading、弧度往返及double→float差分链。当前有效输入矩阵70次分叉均为左右选择变化；证据`88/yaw.native.json`、`88/analysis.json`及`bf144_animation_phase88_quick_stop_consumers`。只确认受控数值边界和实际消费，未证明默认动作自然触发；未改阈值、加补偿或修生产。

- [ ] BF144-D175 / P2 — Turn混入首帧为零delta时，仍在播放的实例被零权重清理。
  - Source：fresh `AnimMontage.cpp:2785–2790`只在已停止且混合完成后销毁实例；动态Montage刚开始可以有效且权重为0。原生八资源的零delta首帧均保持valid/playing，后续时间恢复后正常混入；`AnimInstance.cpp:1043`的零delta分支选择同步更新，并非跳过该生命周期。
  - Target：`turn_layer.rs:229–248`推进后仅保留尚未准备采样的实例或权重大于epsilon的实例；sampling_ready=true且首次delta=0时，播放中的实例被删除，且没有自然结束/惯性请求。
  - 反例：实际图回调创建Turn并确认采样已就绪，将owner倍率设为0一帧后恢复；八个资源均消失，后续没有对应Slot样本和RotationYawSpeed。仅保留被误删的同一实例，64帧恢复全部通过；48组姿态/曲线对照全部拒绝原样，最大骨骼局部位置差约10.4cm、旋转约11.86°、转向曲线差约94.16°/秒。
  - 漏检/边界：旧测试覆盖“资源未准备好时不走时钟”，没有覆盖“已准备好但首次时间为零”。证据`90/StandingTurn90Left.native.json`等8资源、`90/analysis.json`及`bf144_animation_phase90_zero_delta_turn_lifetime`；这是正在播放实例的错误清理，不与.83已证明无姿态影响的“停止后的零权重晚清理”混同。仅登记，未修生产。

- [ ] BF144-D176 / P3 — Turn自然结束遗漏原生停止窗口，惯性化晚一帧开始。
  - Source：fresh `AnimMontage.cpp:2625–2637`在剩余播放时间≤max(触发值,1e−4秒)时停止；当前Turn触发值为0、混出模式为Inertialization，因此当帧权重归零并发出0.2秒请求。
  - Target：`turn_layer.rs:239–243`只在累计位置达到duration时结束。当前累加位置在常规步长下可略低于终点，源已进入停止窗口，目标仍保留全权重直到下一帧；这不是D118所指的反算时钟，未直接归并。
  - 反例：六个2秒资源在原生60/30/120Hz轨迹中各晚一帧；实际角色使用1/60秒世界步长、owner倍率1，第100帧source已停止而target仍有Slot样本和全权重。只恢复源停止时点，惯性请求及姿态轨迹恢复；826组停止前姿态历史一致，停止后最大局部位置差约0.987mm、旋转约0.0251°。两个Crouch180资源在当前60Hz矩阵中本已通过，作为控制保留。
  - 漏检/边界：旧自然结束测试只确认最终清理/epoch增加，未与原生停止帧和实际惯性姿态对齐。证据`90/native_analysis.json`、`90/analysis.json`及`bf144_animation_phase90_natural_stop_consumers`；只确认停止时机，未以此宣称完整惯性求解器或通知事件等价。仅登记，未加阈值补丁。

- [ ] BF144-D177 / P2 — Turn预混使用球面插值，偏离源加权四元数累加。
  - Source：fresh `AnimInstanceProxy.cpp:2056–2085`把所有非加性Montage和相关的基础姿态按顺序交给`BlendPosesTogetherIndirect`；`AnimationRuntime.cpp:458–484`先逐项加权累加、按最短符号对齐，最后仅归一化一次。64个公开UE原生混合样本已与独立公式核对。
  - Target：`turn_layer.rs:461–487`逐层调用LinearInterpolator；依赖的`pose.rs:156–159`实际使用Quat::slerp。总权重恰好为1、没有基础姿态参与时，两个不等权重的Turn也已不等价，不能只修D123的中间归一化。
  - 反例：实际Standing/Crouching八资源在0.25/0.4秒采样，左右90/180及相同姿态控制共20组；权重0.3/0.7时14组失败，最大局部旋转差约0.34113°，位置、scale和曲线均在合同内。仅在夹具将预混改为nlerp后20组全部通过；0.5/0.5、全单层和相同姿态控制通过。原producer与实际发布Pose逐值核验，使用共同采样时间/权重隔离D175/D176。
  - 漏检/边界：旧`turn_layer.rs:1020–1021`测试把同一个LinearInterpolator当作source oracle，不能发现插值算法不一致。证据`91/settings.native.json`、`91/analysis.json`与`bf144_animation_phase91_actual_slots`；为实际当前角色/图的受控重叠输入，不宣称默认输入自然产生全部配对。仅登记，未改生产或依赖。

- [ ] BF144-D178 / P3 — Turn基础姿态不再相关时，目标把未满权重的Montage升为全权重。
  - Source：fresh `SlotEvaluatePose:2058–2085`在剩余基础权重≤1e−5时不加入基础姿态，但保留Montage实际权重；`AnimationRuntime.cpp:469–480`仅在输入多于一个时归一化旋转，单层位置、scale和曲线仍按实际权重输出。
  - Target：`source_slot.rs:116–132`在source_weight=0时直接返回归一化的overlay，并用1.0生成槽曲线上下文；丢掉了尚未等于1的Turn权重。
  - 反例：八个当前Turn资源×0.25/0.4秒，权重0.999995的16组实际站蹲槽全部失败，最大局部位置差5.202275µm、scale向量差8.672015e−6、曲线差0.00088500977。单层四元数长度也不同，但这里不将长度差误报为角度差。源权重恢复16组全过，0.99998、0.99999和1.0的48组对照通过。
  - 漏检/边界：旧SourceSlot测试使用0.6/1.0，未覆盖“不求值基础姿态但Montage尚未满权重”的区间；与D170加性旋转full-weight分支不同。证据`91/analysis.json`的nearfull_isolation及实际Slot输出；仅登记，不改变生产门槛或声称可见姿态故障。
  - 后续消费：在共同正倍率1.2下，16个失败继续进入实际ModifyCurve的RotationYawSpeed，最大差0.0010681152；16个源恢复全部通过。倍率作为同输入控制，不以此替代.89的实际选择/倍率推导。

- [ ] BF144-D179 / P3 — Turn曲线预混遗漏归一化后后续层的相关性判断。
  - Source：fresh `AnimationRuntime.cpp:230–239`对首层Curve执行Override，对后续层执行Accumulate；`AnimCurveTypes.h:817–829,919–933`只有后续累加要求weight>1e−5。先通过实例资格的层仍可能在总权重归一化后降至该门槛以下，首层与后续层不能统一过滤。
  - Target：`turn_layer.rs:601–609`把每层按归一化权重交给`source_pose_nodes.rs:967–989`，所有曲线都直接value×weight累加；没有保留源首项/后续项的区别。
  - 反例：原始权重1/1.1e−5/1的三个当前Turn均通过源实例资格，中项归一化后约5.5e−6；实际站蹲槽20组曲线全部失败，最大差0.0009765625，位置/旋转/scale均通过。把小权重改放首项，20组原始目标全部通过；源曲线运算恢复两组共40个控制全部通过。D168的上游原始权重剔除无法修复此反例。
  - 漏检/边界：旧曲线混合只核对单层、普通权重或原始权重已不相关的实例，未将实例资格与归一化后的曲线资格分开。证据`91/rows.json`中两个三实例权重顺序及64组原生混合对照；受控有效槽输入，未宣称默认Turn调用自然形成该超额权重。仅登记，未修生产。
  - 后续消费：共同正倍率1.2下，中项小权重的20个失败继续进入实际ModifyCurve输出，RotationYawSpeed最大差0.0011749268；首项20控制及两组40个源恢复全部通过。

- [ ] BF144-D180 / P3 — Roll排队状态使用的GraphClip轨道遗漏有效触发偏移。
  - Source：fresh `AnimTypes.cpp:81–98`按GetTime+TriggerTimeOffset及有效起点+Duration+EndOffset计算；A_Als_Roll的RootMotionScale原生有效区间为0.0000999999975→1.5001000166，TranslationScale=1.3。`AnimSequenceBase.cpp:478–505`按本帧区间重叠采集，越过结束点的这一帧仍会排入状态。
  - Target：`animation_events.rs:730–735,950–965`对排队状态采样实际GraphClip，`A_Als_Roll.anim.ron:27–29`却保留0→1.5；manifest已有正确有效区间，但此路径没有用它判断普通状态的当前资格。当前`compile_als_event_tracks.py:66–83`会输出0.0001→1.5，表明起点产物已过期、终点仍被裁到clip长度；修正manifest精度不能消除这条独立差异。
  - 反例：共同满权重播放区间在源起点前一个f32时，实际派发器提前Begin并写1.3；从0.1推进到1.5时提前End并写1。六帧轨迹中四帧回调/倍率不符，仅在内存恢复实际事件轨道的两个有效端点后全部通过。566个以正确有效区间构造的局部资格/生命周期对照通过，说明问题进入实际采样轨道。
  - 漏检/边界：旧根运动通知测试同时自建manifest与EventTrack，未核对安装产物；旧通知端点单测也未经过该轨道。证据`93/native.json`、`93/static.json`及`bf144_animation_phase93_actual_states`。限定共同合法区间的受控边界，普通帧率自然Roll没有复现此项；未更改转换器或产物。

- [ ] BF144-D181 / P3 — Roll中断后排队RootScale状态留到播放实例删除才结束。
  - Source：fresh `AnimMontage.cpp:2818–2870`对已中断实例不再排入通知；在正常派发帧，`AnimInstance.cpp:1949–1980`对未再入队的活动状态调用End。ALS `SetRootMotionScale.cpp:43–61`在当前倍率仍等于通知值时恢复1；已被其他调用改写的倍率保留。
  - Target：`animation_events.rs:875–887,950–956`仍把中断播放计入active_sources，随后直接continue，既没有更新为空队列，也没有退出该状态；只在实例从播放列表消失后执行inactive清理。
  - 反例：实际共同派发的中断/零权重/外部改写三个轨迹共七帧回调或倍率不符；只移除中断记录后通过。真实Roll后进入Ragdoll，owner0.5/1/2共66帧中42帧不符，倍率分别多保留23/11/5个世界帧，最后才补End；对应恢复全部通过。正常自然Roll的244帧和非匹配倍率保留控制通过。
  - 漏检/边界：旧Ragdoll中断测试只核对动作停止与混出，没有核对活动排队通知。Ragdoll期间实际Roll驱动已停止，本项不把仍存在的root采样量算作位移；仅确认状态/回调延迟，跨阶段跳过派发仍由后续批次覆盖。证据`93/analysis.json`、`bf144_animation_phase93_natural_roll_interrupt`，未修生产。

- [ ] BF144-D182 / P2 — 同一Roll状态通知按播放实例拆分，重入时覆盖源应保留的根运动倍率。
  - Source：fresh原生RootScale的NotifyStateBehaviorFlags=0；`AnimNotifyQueue.cpp:70–76`合并同一通知，`AnimInstance.cpp:1893–1938`仅在NoMergeOnConcurrentPlay置位时要求实例ID相同。因此旧Roll与新Roll连续提供同一通知时保留活动身份，不重复Begin。ALS重入许可允许action已经清空而旧montage尚在播放；Begin才设置1.3，Tick不重写，End也保护其他调用改过的倍率。
  - Target：`animation_events.rs:239–241,950–955`以(source_asset, instance_id)分别保存生命周期。新Roll实例重新Begin；旧尾部还会另发End。superseded_roll只避免部分清理写入，不能阻止新Begin覆盖倍率。
  - 反例：真实Roll到action结束且旧实例仍active时再次按Roll；默认1.3的六帧中两帧多发Begin/End但位移一致。先通过现有倍率组件设置2再重入时，目标改回1.3，随后五帧实际根运动驱动少移动约0.72–2.40cm；源此时应保留2。仅在夹具按源入队规则保留同一通知身份，12个真实重入恢复帧全部通过；四个普通接替失败帧也恢复。
  - 漏检/边界：旧多实例通知测试核对的是本就独立的branching动作；旧Roll重入倍率测试只核对默认1.3，未把排队状态的NoMergeOnConcurrentPlay与外部倍率改写组合。真实驱动的位移与缓存倍率逐帧吻合，误差来自重复Begin，源/目标都在通知前缓存当帧倍率，因此第一重入帧的位移保持一致。证据`93/native.json`、`93/analysis.json`及`bf144_animation_phase93_reentry_root_consumer`；倍率2为外部改写控制，未宣称默认1.3出现位移差异。

- [ ] BF144-D183 / P3 — 即时动作状态只按帧末区间求值，遗漏branching marker回调。
  - Source：fresh `AnimMontage.cpp:741–777,2281–2289`在(previous,current]中寻找首个标记并拆步；`2877–3005`先按start<position≤end校正活动状态，再执行Begin/End marker，最后仅Tick仍活跃状态。区间校正规则不能替代标记回调；零步长没有跨标记，但仍执行区间校正。
  - Target：`event_lifecycle.rs:171–194,263–275`直接用最终current_time求native状态，仅当同帧跨越start和end才补Begin/End。因此精确到达起点时不Begin，精确到达终点时不End；GetUp/Mantle从默认0开始、单帧越过结束点时，负起点不在跨越区间，又漏掉源会执行的Begin/End。
  - 反例：九个当前montage先统一原生精确端点以隔离D121。2250组局部比较有345FAIL；424个实际派发帧有28FAIL，其中10帧动作组件和实际图的locomotion_action不符。只在夹具把本帧标记语义映射回现有生命周期输入后，1125个局部及212个实际恢复全部通过；权重0、LOD过滤、chance0、普通零步长、中断直到终止及外部动作保护控制通过。
  - 漏检/边界：旧动作端点测试使用边界前后较大间隔，未覆盖exact marker；普通状态单测只验证区间规则，不能验证Montage拆步事件。六个真实Roll正常/中断控制共310帧全部通过，本项不宣称普通帧率自然触发。参考由fresh UE执行顺序和原生资产有效时间构成，未把读取工具的GetAnimNotifies采样称为UE角色回调执行。证据`94/static.json`、`94/analysis.json`与`bf144_animation_phase94_actual_lifecycle`；仅登记。

- [ ] BF144-D184 / P3 — Roll旧branching实例终止时额外保护同名新动作。
  - Source：fresh `AnimMontage.cpp:1760–1778`在Terminate结束每个仍活跃的branching状态；ALS `SetLocomotionAction.cpp:37–48`只比较当前动作tag，没有新实例保护。旧实例中断后状态仍活跃；新实例已Begin后，旧实例稍后Terminate会清除仍为Rolling的动作，新实例的Tick不会再写Rolling。Stop从ActiveMontagesMap移除旧实例，ALS的Rolling且Montage_IsPlaying为false分支允许再次启动。
  - Target：`animation_events.rs:895–907`仅对Roll资产检测superseded_roll，并跳过旧实例End的动作清除；其他动作采用源的tag保护。这一分支令同样的旧中断→新Begin→旧终止轨迹保留Rolling，源应为空。
  - 反例：九资产×有无外部Ragdolling覆盖×原始/恢复的144个实际帧中，只有原始Roll且无外部覆盖的旧终止帧失败；动作组件和当前图输入都错误保留Rolling。夹具让同一旧状态经过正常End路径后72个恢复帧全部通过，外部不同动作保留控制也通过。
  - 漏检/边界：旧`stopped_roll_tail_accepts_a_new_loaded_montage`把“旧结束不清新Roll”直接写成期望；BF144.14四个重入控制只覆盖正常动作结束前后的接收与第一重入帧，未覆盖旧中断状态的稍后Terminate。此处是源允许的停止后重入合同与实际派发器控制，未宣称默认输入自然触发，也不与排队RootScale合并身份的D182混算。证据`94/static.json`、`bf144_animation_phase94_instance_termination`；未改生产保护分支。

- [ ] BF144-D185 / P3 — Mantle提前混出直接使用未消费的移动命令。
  - Source：fresh `AlsAnimNotifyState_EarlyBlendOut.cpp:44–66`只在Tick读取Character的bHasInput并停止当前实例；`AlsCharacter.cpp:1109–1122`由CMC当前加速度归一化后生成该值。`AlsCharacterMovementComponent.cpp:202–210`在InputBlocked时先把输入清零，因此仍按着移动键也不应触发提前混出。
  - Target：`mantle_drive.rs:377–385`从AlsInputCommand或AlsInputOverride直接取move_dir并比较长度，没有使用已消费的输入。四个条件的OR、默认开关/Tag、仅Active阶段和实例定位本身正确，但该输入来源会产生额外或缺失的停止。
  - 反例：六资产的96个共同状态/override控制中24组失败，输入恢复的48组全过。真实low/high攀爬尾段通过现有组件阻止移动后再按前进，controller basis已为0，目标仍停止；0.1秒后实例及当前根Slot发布权重分别为0.8017496/0.8437502，源应保持1。8个实际路径控制中仅这两组失败，4个输入恢复全部通过，未阻止输入时正常停止。
  - 漏检/边界：旧`mantle_early_blend_out_uses_source_conditions_and_custom_movement_mode`直接传入bool，只验证条件组合，没有覆盖命令到已消费输入的链路；.17只测未阻止的移动命令。366个实际条件/阶段/实例控制及90条1260帧原生停止对照通过，排除曲线、时长和重复停止算法。证据`95/native.json`、`95/analysis.json`与`bf144_animation_phase95_blocked_input`；输入阻止是受控的现有API状态，未宣称默认攀爬尾段会自行开启。未修生产，也未把原生Stop权重采样称为UE角色通知回调执行。

### 核验边界记录（不计为无差异）

- [x] BF144-U01 — 大型平地箱体的胶囊sweep数值边界已于BF144.43完成同输入原生核验，归并为D125。原先仅有解析对照的`07/remaining_corrected/`已补上`43/queries.native.json`与实际落地消费复验；该关闭表示差异已解释并登记，未修复，也不表示无差异。

- [x] BF144-U02 — 实际convex/trimesh的manifold差异已于BF144.46–.64完成当前输入闭包的动力消费者分类；确认问题均已登记，关闭不代表修复或总体等价。BF144.45的98组实际PA capsule/box与生产Playground Collider对照中，接触/分离状态全部一致；几何合同54PASS/44FAIL，其中15组只差重复点数量。Barrel平面接触的head由源2点变为目标1点、foot由源4点变为1点；三角网格另有额外点、不同法向及支持点位置，原生单约束最多14点。证据`45/contacts.native.json`、`45/clearance.native.json`、`45/final/`；fresh Parry凸多面体支持面仅取前4个顶点，pfm-pfm另加GJK点，已定位候选机制，尚未把raw集合差异直接计成运动缺陷。下一步分别沿convex支持面和trimesh特征/聚合进入实际normal、friction及历史消费者；已有D100/D101/D102覆盖的原因须归并，不重复编号。 BF144.46/48将桶类八边形截断归D126、非共面参考投影归D128、head额外GJK消费归D129，46组剩余逐shape首次位置合同及删除控制完成。BF144.49经真实原生聚合入口及32组生产compound控制关闭多hull集合/裁点生命周期；60组消费中30组native几何控制全PASS，原18处失败的5处归D126/D129，BF144.50按正确的源force point复核后分类不变，并将6个胶囊斜面根因闭合为D130、两处顶部foot归D128；BF144.51将剩余六box分为D131棱边分支、D132裁剪方向及既有D128参考深度/D129额外GJK裁点影响，48组分离控制闭合；BF144.52补齐32组完整trimesh与18组首次位置消费，九处失败中八处归D129/D133/D134/D135；BF144.53以逐面原生端帽/边侧规则及28组最小消费控制，将Cart head闭合为D136；BF144.54在10个全负phi姿态的共同几何/位置冲量输入下分离速度摩擦，确认D137/D138；BF144.55以源位置投影和18姿态共同几何控制闭合零pushout资格及混合phi，新增D139并补充D95；BF144.56将五个表面受力点余差归D102、重合GJK归D129、同法向共享边摩擦归D133，并确认单面剔除D140；本批17姿态恢复通过，只余Cart foot face989的PlaneVertex/edge选择与裁剪，以及分组和历史继续保留U02。 BF144.57以有效棱边资格和参考面裁剪将Cart foot余差归D141及D134，18姿态两种摩擦的36组恢复全通过；该姿态共享边删点不改变响应，不另计差异。U02仅余实际接触分组与历史消费者。 BF144.58将trimesh自然点序与组内N/T阶段分别归D100/D137，120组的全部恢复通过。覆盖回查补正：上述收口仅涵盖trimesh，convex/compound的完整速度摩擦矩阵尚未执行，仍与跨帧历史共同保留U02。 BF144.59完成30个完整compound的180组速度摩擦矩阵及最终257组控制，新增D142线段交点重复，13组归D102，并补充D128/D132的共面投影消费；60组字段恢复全部通过。两处字段根因（Arch01 hull 4圆柱端点、Bench hull 2的GJK法向）以及convex自然序/分组、跨帧历史仍保留，不能计作无差异。 BF144.60由实际核心GJK和同输入3D裁线把两处字段分别闭合为D143/D144，16组中的8个恢复控制全部通过；U02继续保留convex自然点序/分组与跨帧历史。 BF144.61完成30姿态自然序/分组的240组及三个30Hz边界，分别归并D100/D137；恢复输入全部通过、合并组消融被拒绝，U02仅保留convex/trimesh跨帧历史。 BF144.62完成288组原生两帧几何及105组消费者/生命周期控制，convex/trimesh几何复用归并D104；U02继续保留saved friction/impulse、重建状态与initial penetration。 BF144.63以30组原生保存/匹配和105组消费者/边界核验归并D96，并登记旧冲量初始化D145；U02仅保留GJK重建状态与initial penetration。 BF144.64补齐108个有缓存重建、3个无缓存对照和60个完整当前compound，暖/冷字段全部逐值一致；58个EPA分支也一致。实际世界初始穿透参数解析为1e10cm/s，本批接触及帧长的7722个源公式分支归零，故未发现额外历史差异。剩余边界已闭合。

- [x] BF144-U03 — 已于BF144.47补齐prepare/post矩阵、源生命周期及实际回弹对照，归并D127。`46/inertia_snapshot/`的旧post oracle保留为诊断记录，结论使用`47/final/`的prepare快照与32组控制；关闭表示差异已解释并登记，未修复。D126的常值惯量及实际head控制不受该边界影响。
