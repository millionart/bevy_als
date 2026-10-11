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

## BF139 全范围移植差异审计（只记录）

用户当前目标：找出所有 P2 及以上的移植差异，确认后实时记录进本清单。审计基线为 `bb3dccf3066a217b750670918e30f248ef2c5b98`；本清单登记待修差异，[审计记录](docs/porting-audit-2026-10-11.md) 保存详细证据与覆盖边界。本轮授权 source/target 审阅、最小反例和文档记录，不修复生产实现、不重开历史完成项、不把 TODO 的待验收状态直接视为缺陷。

**本轮审计完成（2026-10-11 15:00 +08:00）：8批覆盖闭环，确认18项P2，未确认P0/P1，均已登记且未修复。最终29个反例/对照为3PASS、26预期语义FAIL；这些失败用于证明待修差异，不代表修复验收通过。P2-01源身份误判已撤销。**

- [x] BF139.01 Character / Movement / Input：开始：2026-10-11 11:27 +08:00；预估结束：2026-10-11 12:12 +08:00；实际结束：2026-10-11 11:43:32 +08:00，偏差−28分28秒，审计Gate通过、6项差异未修复。依据本地 Character、CMC、Extras Input 的函数闭包及 BF138 已归档物理审计边界，逐项检查状态、输入、速度、时钟、碰撞、基座和旋转的真实消费者。Gate：源文件当次身份、可定位的算法/输入/生命周期差异、最小反例与漏检原因。后续动作/物理、动画/图/资产、相机/场景/UI 分批独立登记，各批不超过 60 分钟。
  - 初始范围：已确认本地源可用、工作区干净、182 个 Rust 源文件；ALS 插件含 54 个 C++ 与 96 个头文件（含 Editor、网络、调试和本地取证辅助，后者需按职责排除）。已阅读刻意差异与最近 BF135/BF138 结果；历史修复不作为当前缺陷，固定长帧腿平面症状尚未获得 source 根因，不预先定级。

  - source/static 闭环（2026-10-11 11:42 +08:00）：fresh 本地 Character/CMC/Input、UE Actor component tick/PhysFalling/StepUp、AnimationInstance world-space 输入已核对；原生 reader 导出 IA_Als_Sprint 与 CS_Als_Default，watchdog 自测全部通过。直接链接当前生产库的 9 个最小测试为 2 对照 PASS、7 语义断言 FAIL，归并为以下 6 项 P2；没有运行时实现修改。普通 actual rotation request 被覆盖、未接入当前生成图的旧 stop 输入、以及 default bIgnoreBaseRotation=true 下的速度旋转均不单独算缺陷。

- [x] BF139.02 Roll / Mantle / GetUp 与动作 RootMotion：开始：2026-10-11 11:43 +08:00；预估结束：2026-10-11 12:28 +08:00；实际结束：2026-10-11 11:56:37 +08:00，偏差−31分23秒，审计Gate通过、新增3项P2未修复。依据当次 `AlsCharacter_Actions`、Mantle RootMotionSource、蒙太奇消费与 BF137 已修 owner-clock/parent-space 边界，检查启动/抢占/退出、设置、目标基座、slot/notify/root 的实际消费者。Ragdoll solver/PA 独立下一批。
  - source/static 闭环：fresh native 导出 Roll、GetUp Front、Mantle High 的 RateScale/Blend/Trigger/Slot；UE `UpdateMontage→UpdateWeight→Advance→Stop` 与 `AlphaBlend::Update` 时序对照完毕。完整加载角色后的5个动作反例全部语义 FAIL；累计14测试为2对照PASS/12预期FAIL，归并9项P2。Mantle不扫掠与源 `PhysCustom(...,false)` 一致，不登记；GetUp BlendIn native字段为0，初始weight1不是缺陷。当前目标丢失、抢占、montage tail、基座相对空间和已修动作时钟未发现另一项已证实P2。
- [x] BF139.03 Ragdoll / PA / 物理适配：开始：2026-10-11 11:57 +08:00；预估结束：2026-10-11 12:47 +08:00；实际结束：2026-10-11 12:05:21 +08:00，偏差−41分39秒，审计Gate通过、新增3项P2未修复。依据本地 Ragdoll Start/Refresh/Stop、PA的body/constraint闭包、现有source joint/CCD/sleep实现及 BF135 已归档反例，检查源参数、碰撞/约束/驱动、重建、坐标和最终姿态消费者。Gate：fresh native PA与源算法对照，候选问题以最小反例确认并登记；不修改solver或重跑无关全构建。

  - source/static 闭环（2026-10-11 12:05 +08:00）：fresh原生PA及21body/22shape/20constraint/49pair完成；现转换函数重建profile与安装数据全部字段一致。初始非等比尺寸、正数睡眠配置、支撑移除同帧ragdoll退出三个实际消费者反例FAIL，对照通过，新增P2-10/11/12。查询mode过滤、初始限速、驱动输入和已修父级写回不重复登记；不修改生产代码或转换器。
  - 验证收口：当前生产库17个反例/对照10.05秒完成，为3PASS/14预期语义FAIL，归并11项有效P2；Sprint按原始源更正后的对照PASS。原始阶段报告与更正记录分别保留，watchdog无超时，生产源/资产零改动。

- [x] BF139.04 AnimationInstance / 状态输入 / Feet：开始：2026-10-11 12:05 +08:00；预估结束：2026-10-11 12:55 +08:00；实际结束：2026-10-11 12:10:51 +08:00，偏差−44分09秒，审计Gate通过，补充2个同根因消费者、有效总数仍11项P2。依据原始AnimationInstance的逐帧/相关性回调、View/Spine/Head/Locomotion/Grounded/Standing/Crouching/Air/Feet/Turn函数闭包及现provider/native-refresh/动画时钟，核对算法、生命周期、设置和坐标消费者；图/FSM与离线生成依赖另批。Gate仍为fresh source→消费者→最小反例，只记录源已证实P2。
  - 实时记录（2026-10-11 12:10 +08:00）：fresh AIS_Als_Default原生导出与原始资产身份通过，Head/Spine/Grounded/Standing/Air/Feet/Turn设置和逐帧合同已对照。新增两个实际消费者反例FAIL并归并已有根因：P2-05的Head body world yaw在90°父级下得到0°；P2-09的Transition在结束前20 ms推进100 ms，首个Stop帧weight=0.64799964而源仍1。旧view_pitch_norm与诊断DynamicTransition缩放分支没有真实pose消费者，不计P2。
  - 收口：19个最小测试11.82秒完成，3PASS/16预期语义FAIL；Head/Transition证据及完整输出存本地phase4。源Turn的惯性混出、raw view yaw speed、不相关图的诊断状态与已实现脚锁base/teleport/时钟均未证实另一项P2，不改生产代码。

- [x] BF139.05 生成图 / FSM / 曲线 / 同步 / Rig与资产转换：开始：2026-10-11 12:11 +08:00；预估结束：2026-10-11 13:06 +08:00；实际结束：2026-10-11 12:43:18 +08:00，偏差−22分42秒，审计Gate通过、新增2项P2未修复，有效累计13项。依据15个安装root、23个递归AnimBP、374个已有生成产物与39个上轮native依赖的规模，重新核对源身份、完整安装闭包和当前共享转换/运行时节点语义；同步对本轮发现的源差异做focused反例。原生读取与完整重转实际28分40秒，所有新P2确认即写本清单，无转换器或生产图修改。
  - source/static进展（2026-10-11 12:18 +08:00）：127个动画glTF逐资产/源track比较完成，严格raw合同104通过、23提示差异，原阈值与失败报告保留。逐项复核：12个转身/原地旋转root受源ForceRootLock=RefPose约束，不能把raw root的90/180°变化作为输出pose差异；3个Rifle仅FName大小写需语义匹配；其余8个pose/overlay量化差异上界0.08835 mm、0.18716°，未达到P2。当前Sequence Player/Evaluator、缓存pose、Slot、同步角色、Rig初始化/当帧pose输入与坐标算法继续核对；15root原生重读/完整产物比较尚在进行。
  - 实时记录（2026-10-11 12:21 +08:00）：SourceSlot→真实InterruptibleFsm反例确认未传递SourceWeight，蒙太奇0.75时底层machine记录1而源应0.25，无蒙太奇对照1通过；fresh AB_Als_Standing存在两处Machine Weight真实转换consumer。登记P2-13，有效累计12项；完整重读/转换审计继续。
  - 实时记录（2026-10-11 12:24 +08:00）：完整角色进入ragdoll后，owner倍率0/实际animation.delta_seconds=0，但非同步Flail序列仍收到全局0.016666668并从0.01987938推进到0.028212715。根因是root查询仍用全局Time，未受FSM内部时钟或同步group覆盖的SequencePlayer直接消费该值，登记P2-14，有效累计13项。此前Rifle候选没有捕获符合条件的活跃player，前置条件失败不算语义证据；改用实际Flail链后确认。
  - 最终source/static闭环：39个本轮原生记录、15root/23AnimBP/236子图完整转换，359RON+15manifest与安装资产全byte一致（新增/删除/变化均0）；当次完整source闭包53.931秒通过，覆盖98pose图、14FSM、104条件图、907节点、737pose连接。345类型规格对应的源码/安装资产自上一份规格起零变化，且逐graph_source核对通过；510文件编辑器与runtime boundary通过。误把类型规格传给旧scoped-cache专用合同得到schema错误，未当作资产缺陷或验收通过，已使用当前生成树的完整原生闭包核对。22个有效最小测试13.61秒为3PASS/19预期语义FAIL、13项P2；原始报告保留，生产代码/转换器/原始资产零改动。

- [x] BF139.06 通知 / 脚步贴花 / Overlay物件行为：开始：2026-10-11 12:43 +08:00；预估结束：2026-10-11 13:28 +08:00；实际结束：2026-10-11 13:27:42 +08:00，偏差−18秒，审计Gate通过，扩展P2-05/08，独立差异仍13项。依据本地ALS Notify/NotifyState、UE通知队列与蒙太奇采样语义、原生Effects配置和现动画事件/贴花/附着物件消费者，检查触发权重/顺序/生命周期、空间/附着/设置及重建。复用刚完成的原生Overlay图闭包，仅为本批资产补充必要原生读取；声音/Niagara按既有刻意边界只审挂钩，不要求完整渲染实现。Gate为fresh source和实际最小反例，确认即登记。
  - 实时记录（2026-10-11 13:23 +08:00）：137个源动画/蒙太奇、177通知/60marker重建清单与安装文件byte一致；fresh FES/B_Als_Character读取成功且原始/安装副本一致，脚步配置与附着规则已对照。QuickStop两个当前函数反例确认P2-05/08的额外消费者：90°父级或已消费的0.0002右输入时，实际选左侧1.75、源选右侧2.375；正常输入/无父级对照通过，总数仍13项。
  - source/static收口：fresh Blueprint确认两Overlay组件Location/Rotation/Scale均SnapToTarget、Clear置空mesh/class、Mantle非Low及Ragdoll开始清除/结束恢复，当前行为一致；FES轴、trace0.5m、35°与Decal4+2s对照完成。UE多圈通知MaxLoopCount与目标一致，未误报；177通知当前均NoFiltering/Chance1。两个QuickStop实际函数反例0.09秒确认失败，源播放率范围由当次AIS证明；原状态/清理、蒙太奇/普通序列及同步leader/follower分支已审阅。网格/socket/材质静态数据明确归BF139.08联合核对，不计作本批数据验收；剩余相机/场景分为两个≤60分钟独立批次，目标不缩减。

- [x] BF139.07 Camera / CameraShake：开始：2026-10-11 13:28 +08:00；预估结束：2026-10-11 14:08 +08:00；实际结束：2026-10-11 14:00:10 +08:00，偏差−7分50秒，审计Gate通过，新增P2-15未修复，有效累计14项。依据原始ALSCamera组件/动画实例/Settings、CameraShake Notify及UE CameraManager摇镜时钟/空间闭包，结合刚完成的原生AB_Als_Camera完整图，检查view/pivot/lag/collision/肩位/一三人称/时钟/owner切换和最终Camera消费者。Gate为fresh source→实际消费→必要最小反例，确认后实时登记。
  - 实时记录（2026-10-11 13:55 +08:00）：fresh CameraSettings/Shake/Skeleton及其余6个camera定义依赖读取完成；发现世界暂停+owner CustomTimeDilation=0时，真实Camera player仍推进0.05 s，FirstPersonOverride从0.07407408到0.7407408，view移动1.9981213 m。源USkeletalMeshComponent明确bTickEvenWhenPaused=false、Pause scheduler不调度；登记P2-15，有效累计14项，owner=1暂停时钟对照PASS。相机数据全量重建核对继续。
  - 最终source/static闭环：10个当次原生camera定义来源重建/源自测通过，所有消费参数、骨架/socket/震动/FOV/碰撞设置一致；仅旧生成器的未使用graph metadata仍输出generated/前缀，当前运行通过固定scene wrapper引用正确安装图，未当作P2或完整byte一致。第一轮批读取误用外层LASTEXITCODE把两次成功误报失败，已按各reader的success/exit_code报告确认完成、从未读项继续，没有重跑成功读取。P2-15完整角色反例1.74秒通过对照且语义失败；无生产改动。
- [x] BF139.08 Playground / 材质与Overlay静态数据 / HUD与PlayerStart：实际开始：2026-10-11 14:00 +08:00；预估结束：2026-10-11 14:55 +08:00（前批提前完成，相应提前原计划14:08/15:03）；实际结束：2026-10-11 15:00:06 +08:00，偏差+5分06秒，在误差窗口内，审计Gate通过、新增4项P2未修复。依据剩余场景/碰撞/材质导入链、已导出主角/Overlay图及source骨架、既有材质通道审计工具与近期场景批记录，保留场景/几何与碰撞、移动环境、光照/纹理通道/材质、Overlay网格socket数据、HUD/菜单/GameMode/PlayerStart及已接受刻意差异；只记录不修复。新增EngineSky源闭包与环境接触材质反例解释本批偏差，没有扩大到实现。
  - 实时记录（2026-10-11 14:12 +08:00）：fresh Playground世界、HUD、GameMode读取完成；UE FindPlayerStart先把IncomingName转FName再比较。当前choose_player_start在`DoorA`/`Other`两候选、ticket=1下，精确DoorA对照成功，`doora`却回退Other，原生函数断言FAIL；登记P2-16，有效累计15项。出生点world transform/unit scale候选继续验证，未提前定级。
  - 实时记录（2026-10-11 14:16 +08:00）：fresh GameMode父类为Engine.GameModeBase，未覆盖Find/Spawn；当前真实admission生成的pawn继承PlayerStart scale2，且parent x20/local x−5时生成x−5而源world x15。登记P2-17，有效累计16项；最初夹具在finish后add_plugins的配置错误已改为安装对应系统，只有后续实际出生结果计入反例。
  - source/static进展（2026-10-11 14:31 +08:00）：fresh世界记录与旧原生记录完整一致；169静态部件mesh/transform、180物件Visibility响应、source light 3 lux、HUD生成数据及主角79骨骼逐字段核对通过。当前27个反例/对照16.15秒完成，3PASS/24预期语义FAIL归并16项P2，所有失败均已核对断言、无加载或watchdog失败；Overlay递归静态数据及环境mesh/material/UI剩余读取继续。
  - source闭包补充（2026-10-11 14:35 +08:00）：fresh世界中可见SkySphere的实际MID参数与Actor编辑参数不同，目标manifest目前消费后者；继续读取其已明确引用的EngineSky Blueprint/Material，核对最终颜色消费者后再定级。34个Overlay来源已闭合，13模式/8物件完整生成核对及self-test通过；22处文字文本/对齐/尺寸/缩放/间距/颜色通过，FText转义解析的夹具错误已纠正，未计作差异。
  - 实时记录（2026-10-11 14:41 +08:00）：fresh EngineSky Blueprint→Zenith曲线→材质输出闭包确认太阳pitch−45映射time0.5，RGB曲线已进入常量尾段(0.0228,0.048994,0.12)；实际Playground生成天空网格顶点为(0.034046054,0.10924697,0.29500008)，0.91秒生产setup反例FAIL。登记P2-18，有效累计17项；源期望由当前曲线重算，并非只引用已保存MID。
  - source闭包补充（2026-10-11 14:48 +08:00）：四张环境/Footprint纹理逐RGBA像素一致，两个packed shader的G/A消费及缺Alpha/漏采样反例通过，Overlay菜单生成数据一致。fresh Arch BodySetup/世界组件/材质均无物理材质覆盖，UE回退DefaultPhysicalMaterial(0.7 friction/0.3 restitution)；目标环境桥未赋材质，继续沿当前场景mesh加载与Avian接触组合消费者验证该候选。
  - 实时记录（2026-10-11 14:49 +08:00）：实际Playground加载Arch并完成生产mesh physics bridge后，环境friction/restitution为0.5/0，与已正确移植的ragdoll材质按生产Average组合得到0.6/0.15，源应0.7/0.3；0.92秒反例FAIL，登记P2-19，有效累计18项。五个静态mesh的15个源凸包重建与安装数据byte一致，缺陷在环境材质消费者。
  - 最终source/static闭环：34项Overlay依赖、34项环境依赖和5项EngineSky依赖完成；13 Overlay模式/8物件、150物件材质参数、22文字、HUD/菜单、2移动/1旋转平台/4曲线key核对通过。20环境mesh的8Box/5Convex/7TriMesh源碰撞模式一致、15凸包全byte一致；严格Box检查仅S_Crate出现约0.0014 mm半尺寸向量取整差异，原阈值/失败保留、未冒称完全一致或定为P2。最终29测试17.69秒为3PASS/26已核对的语义FAIL，无夹具/加载/无进展失败；118个当次native唯一资产中110个插件资产原始/安装副本全部byte一致，生产源码/资产/转换工具/Cargo相对基线零变更。天空云纹理/滚动遗漏归并P2-18；余下只是以上18项待修实现，不继续自动展开。

### BF139 已确认待修差异（只登记，未授权实现）

以下条目修复均未开始；本轮审计时点及 Gate 属对应 BF139 批次，修复被选为当前任务后再按 fresh source 依赖拆分估时。详细反例与漏检原因见审计记录的同号条目。

源身份更正（2026-10-11 12:01 +08:00）：BF139-P2-01已撤销，当前前两批有效差异8项。工程插件链接指向含本地调试改动的Build副本，原始 `b754d6f0` 的Sprint仍按输入bool设置Sprinting/Running，目标一致；初期记录的6/9项分别更正为5/8项。146个原始C++/header完整比较仅CharacterExample cpp/header有语义差异、AnimationInstance仅换行；6个已使用uasset字节一致，其余记录不受影响。保留初期Gate结果以说明错误源期望的漏检；后续只从原始插件根读C++，P2-01编号不复用。

- [ ] **BF139-P2-02 DesiredRotationMode=Aiming 无效**：源 `ALS/Private/AlsCharacter.cpp:675` 将 desired Aiming 与 Aim 按钮独立合并；目标 `src/als/state/aim.rs:55–60` 强制把存储 Aiming 改为 ViewDirection。Grounded/Standing、desired=Aiming、按钮未按下时实际反例 FAIL。验收覆盖 desired Aiming、普通自由模式、按键覆盖/释放、空中和第一人称，并修正旧测试中“stored mode never Aiming”的源合同。
- [ ] **BF139-P2-03 普通运动忽略角色 CustomTimeDilation**：UE `Actor.h:4877` 的组件 tick 使用 owner 倍率；目标 `dynamics.rs:441`/`motion.rs:3447` 的 Walking/Falling 事务使用全局 Delta。3.75 m/s、50 ms 帧、倍率0.5时源水平位移0.09375 m，目标0.1875 m，实际物理反例 FAIL。验收应覆盖 Grounded/Air、0/0.5/1/2 倍率、起跳/落地、基座和动作移交；已完成 Roll/Mantle/GetUp 不因名称相似而复开。
- [ ] **BF139-P2-04 落地丢失当帧剩余行走时间**：UE CMC `PhysFalling:5050→ProcessLanded:6373→StartNewPhysics` 传递 remainingTime；目标 `motion.rs:2697` 发现地面后直接 break。200 ms帧/50 ms子步、3.75 m/s水平速度、从1 m以3 m/s下落时源约0.75 m，目标0.10516852 m且已Grounded；无落地对照0.75 m PASS。验收覆盖子步内落地后的Walking预算、速度/制动/输入及落地动作仲裁，不以“最终Grounded”代替位移证明。
- [ ] **BF139-P2-05 普通 locomotion/provider 混用局部与世界旋转**：源 Character/AnimationInstance 使用 actor world transform；目标 `motion.rs:415,577–579`、`actual_motion.rs:90`、`ab_als_provider.rs:319`、`locomotion_layer.rs:209` 以局部旋转计算世界朝向/局部速度并写物理 Rotation。90°父级下前进2 m/s被识别为左移；真实移动管线中渲染world yaw约0°而物理yaw−90°，两个反例 FAIL，无父级对照 PASS。验收覆盖父实体当帧平移/旋转/缩放及普通 Grounded/Air 的最终朝向和动画输入，不复开已修动作/相机项。
  - 12:10补充：`head_layer.rs:72–118,166` 同样把actor局部Transform作为world rotation/yaw/pitch输入。完整角色在物理后固定local identity、parent yaw90，实际 `locomotion_state.rotation_yaw=0`，源应90；该值继续进入真实Head/Spine消费者，已稳定FAIL，归并本项。
  - 13:23补充：`stop_link.rs:398–421,869–903` 与 `transition_notify.rs:334–350` 的QuickStop也传入局部body。无输入、world body yaw相差90°而目标yaw0时，源选右侧/2.375，实际左侧/1.75；直接生产选择函数反例FAIL，普通无父级对照通过。
- [ ] **BF139-P2-06 普通 StepUp 忽略组件跨越许可**：UE CMC `CanStepUp:7525` 必须检查 `CanCharacterStepUp`；目标 `motion.rs:3365` 的台阶事务不消费该许可，现 `AlsMantleTargetPermission.allows_step_up` 仅在 `mantle.rs:569` 使用。20 cm台阶许可true/false两次都走到z=−3.4291668，false反例 FAIL。验收覆盖普通低台阶、允许/禁止、actor/child collider许可所有权及Mantle原有分支。
- [ ] **BF139-P2-07 Mantle 用局部位置计算世界 root 速度**：源 `AlsRootMotionSource_Mantling.cpp::PrepareRootMotion` 用目标world减actor world；目标 `mantle_drive.rs:727` 用目标world减局部Transform，`:729,795` 也直接写world到local。完整加载角色仅增加x=5父级并保持世界起点不变，横向速度从约0变成299.99997 m/s，实际反例 FAIL。验收应包括父级平移/旋转/缩放、移动目标基座、最终world姿态/速度及退出惯性；BF137的owner-clock已通过边界不重复修复。
- [ ] **BF139-P2-08 动作方向绕开源已消费输入状态**：源 Roll/Mantle 使用 `LocomotionState.bHasInput/InputYawAngle`（安全归一化阈值平方1e−8）；目标 `roll.rs:57`/`roll_drive.rs:1931` 再用1e−6拒绝有效小输入，Mantle的 `motion.rs→mantle.rs:469` 则先归一化原始输入，使无效微量输入变为有效方向。原生反例：0.0002向右输入已has_input=true，Roll实际yaw0而源−90°；0.00001输入已has_input=false，Mantle trace却向右50°。普通0.01输入对照PASS。验收应从实际消费后的输入状态推导动作方向，覆盖阈值两侧、无输入、真实player-view及已有速度分支。
  - 13:23补充：Source `PlayQuickStopAnimation:1590` 同样使用已消费输入；`stop_link.rs:407` 又以平方f32::EPSILON过滤原始输入。实际movement已has_input=true/input_yaw=−90的0.0002右输入，QuickStop在stored target yaw0时错误选左侧1.75，源右侧2.375；0.01对照通过。源AIS当次原生确认QuickStop范围1.75–3，归并本项。
- [ ] **BF139-P2-09 Roll/GetUp 自然混出时序及剩余预算不同**：UE `AnimInstance.cpp:573–578` 先UpdateWeight后Advance；`AnimMontage.cpp:2626–2636` 首次进入自然混出窗口才Stop，并用当前剩余播放时间作为混出时长。目标 `roll.rs:48` 与 `get_up.rs:112–130` 从连续elapsed直接计算默认长度的fade。100 ms帧下，Roll到0.9 s的首个Stop帧源weight仍1、实际0.93627644；GetUp到1.0 s源仍1、实际0.92592555，完整角色slot两个反例FAIL，进入窗口前weight1对照PASS。验收应覆盖窗口跨越、不同帧长/RateScale、自然与中断、slot pose/曲线/notify权重，并核对下一帧使用剩余预算。
  - 12:10补充：源Transition使用自定义TriggerTime=0，首次Stop也保留已UpdateWeight的当前weight；`stop_link.rs:252–279` 却直接将越过末点的时间用作fade elapsed。实际StandingDynamic duration1.533333、rate1.5，结束前20 ms推进100 ms后weight=0.64799964/stop_elapsed=0.08000004，源本帧应1/0；反例FAIL，归并同一蒙太奇生命周期根因。Turn的惯性混出走不同源合同，不据此推断缺陷。
- [ ] **BF139-P2-10 初始非等比角色缩放被物理配置丢弃**：UE SkeletalMeshComponent以含世界缩放的骨骼Transform初始化body，各形状GetFinalScaled保留尺寸语义；目标 `ragdoll_physics_profile.rs:1055` 对非等比输入返回1。完整角色初始scale(2,3,4)时applied=1、root球半径0.1 m，源min-abs应为0.2 m；动态pelvis质量也退回7.9265018 kg，scale2对照37.70501 kg。验收应分别覆盖初始创建的形状/质量/关节frame与源运行中缩放边界，不将源对运行中非等比变更的拒绝误用于初始化。
- [ ] **BF139-P2-11 PA正数睡眠参数无消费者**：native线阈值0.01 m/s、角阈值0.05、计数4、倍率1；`gen_ragdoll.py:64`只生成禁睡bool，runtime只切换SleepingDisabled，实际使用Avian默认0.15/0.15和0.5 s。实际profile插件下无外力/阻尼的0.03 m/s刚体位移0.015 m后睡眠，源应保持运动；0.2 m/s对照通过。验收覆盖正/零阈值、倍率、计数与帧长、连接/孤立刚体；旧零阈值修复本身不复开。
- [ ] **BF139-P2-12 Ragdoll退出读取过时的地面结果**：源StopRagdolling现场RagdollTraceGround；目标 `ragdoll_link.rs:604` 读取上帧GroundFollow。稳定ragdoll后删除唯一支撑并当帧退出，实际Air+GettingUp，源应Air且无起身动作，完整角色反例FAIL。验收覆盖支撑移除/移动/新增同帧退出、当前速度/位置、Grounded/Air路由与输入解除。
- [ ] **BF139-P2-13 Slot未向底层状态机传递剩余更新权重**：UE `AnimNode_Slot.cpp:86–92` 用SourceWeight构造FractionalWeight上下文，StateMachine `:382` 记录最终上下文权重；目标 `source_slot.rs:130` 直接 `data_back("Source")`，绕过现有machine weight scope。实际生产Slot+FSM节点在slot=0.75时machine=1、源应0.25，slot=0对照通过；fresh Standing的Movement Details T8/T9读取Standing States MachineWeight并比较1，确有行为消费者。验收覆盖普通/加法Slot、部分/全权重、always-update、嵌套blend/缓存pose及实际转换条件，不能只核验最终pose/curve alpha。
  - 12:33补充：完整已安装角色开始Roll时，PostLocomotion slot=0.07407408，真实Locomotion States machine仍1，源应0.9259259，实际管线反例FAIL。fresh根图明确PostLocomotion→AB_Als_Locomotion；UE LinkedAnimGraph以WithOtherProxy保留更新上下文权重。最初按Grounded子machine寻找的前置条件未成立，已改为当帧实际活跃Locomotion machine，未将缺失样本算作差异。
- [ ] **BF139-P2-14 非同步序列播放器绕过owner动画时钟**：UE Actor组件tick乘CustomTimeDilation，SequencePlayer使用同一UpdateContext Delta；目标 `post_physics_animation.rs:190–202` 用全局Time运行root查询，`source_sequence_player.rs:350–362,488` 非同步分支直接消费context时间。实际ragdoll、5 m/s骨盆速度、owner倍率0时Flail播放率0.5仍推进0.008333秒，animation.delta_seconds确为0，反例FAIL。验收覆盖root/linked layer中未由FSM或sync覆盖的真实player、0/.5/1/2倍率、通知与pose/驱动结果；已修Roll/GetUp/Mantle时钟不重复复开。
- [ ] **BF139-P2-15 世界暂停时零owner倍率相机仍按真实时间推进**：UE `SkeletalMeshComponent.cpp:440` 设置bTickEvenWhenPaused=false，`TickTaskManager.cpp:1626` 暂停只调度允许暂停tick的组件；目标 `camera_component.rs:146–250` 未区分world pause，owner倍率0触发 `camera_delta_seconds_from_times` 的real-delta回退。实际一三人称切换后暂停3帧：camera elapsed增加0.049999997 s、混合0.07407408→0.7407408、view移动1.9981213 m，源应冻结；owner=1时elapsed保持0对照PASS。验收覆盖暂停/恢复×owner0/.5/1×ignore_time_dilation开关、真实view/动画/lag；不能用“delta=0一律冻结”破坏世界运行时零owner倍率下的源忽略时间倍率行为。
- [ ] **BF139-P2-16 PlayerStart标签失去FName大小写身份**：UE `GameModeBase.cpp:1156–1160` 以FName比较IncomingName和PlayerStartTag；目标 `player_start_types.rs:111` 与 `game_mode.rs:232–233` 用大小写敏感字符串比较。DoorA/Other、random_ticket=1时请求`doora`错误选择Other，精确DoorA对照PASS，生产选择函数反例FAIL。验收覆盖named selection和reserved分支、不同大小写、FName None/数字后缀及未找到标签回退，不只改一个比较点。
- [ ] **BF139-P2-17 出生变换使用PlayerStart局部坐标并继承标记缩放**：UE `GameModeBase.cpp:1214–1222` 取StartSpot世界位置/world yaw，构造默认unit-scale的新Transform；目标 `player_start_types.rs:98–101,354–406` 读取局部Transform并只去pitch/roll，`game_mode.rs:248` 直接传给pawn。实际admission：marker scale2产生pawn scale2；parent x20/local x−5产生pawn x−5，源应unit scale/world x15。验收覆盖Root/parented标记、父级旋转、标记与父级缩放、占用查询及最终pawn，不能只检查fit里yaw被清零。
- [ ] **BF139-P2-18 天空静态渐变丢失源有效参数与云层着色**：fresh `BP_Sky_Sphere.RefreshMaterial` 在ColorsDeterminedBySunPosition=true时以pitch/−90采样Clamped Linear Color曲线再设置MID，`M_Sky_Panning_Clouds2` 的Zenith参数可达材质输出。Playground太阳pitch−45，`C_Sky_Zenith_Color` 的RGB常量尾段为(0.0228,0.048994,0.12)；目标 `playground_scene.ron:221` 保留Actor编辑默认色，`playground.rs:1939–1970` 写入实际天空顶点(0.034046054,0.10924697,0.29500008)，生产setup反例FAIL。该静态替代还遗漏源Time×CloudSpeed→Panner→Cloud纹理→输出链：native云纹理R范围0..255、默认CloudSpeed2/Opacity1，源UV每秒变化0.002；实际目标为无纹理unlit StandardMaterial和不随时间变化的顶点渐变。验收须覆盖源构造分支/太阳角度/自动颜色开关、颜色与云纹理/滚动的最终材质输出，不能只校正默认颜色或硬编码旧MID。
- [ ] **BF139-P2-19 环境碰撞体未保留源默认物理材质**：fresh Arch/世界组件/材质无覆盖，UE `BodyInstance.cpp:3178–3244` 回退DefaultPhysicalMaterial，当次原生数据为friction0.7/restitution0.3、Average；目标 `playground_collision.rs:169–170`、`playground.rs:90` 与 `manual_playground.rs:451–452` 建立环境碰撞体时未赋材质，全局也未映射源默认值。实际加载Arch后解析为Avian0.5/0，与ragdoll组合得到0.6/0.15，源应0.7/0.3，生产加载/组合反例FAIL，会改变滑动和反弹。验收覆盖静态box/convex/trimesh、运动平台和landscape的默认/覆盖材质与真实接触；不能只测试PA body的0.7/0.3。
