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

## BF140 ALS 运动系统第二轮只读审计

用户范围（2026-10-12）：重新审计 ALS 运动系统本身；周边场景、材质、UI、相机和示例物件排除，截图关闭、性能验收继续暂停。只登记有当次本地 source、实际 target 消费者和可重复反例支撑的差异，不实施修复或重开历史实现。报告见[运动系统复审](docs/locomotion-audit-2026-10-12.md)。

- [x] BF140.01 Character / CMC / 状态：输入、步态、旋转、蹲起、跳跃、地面/空中事务、移动基座与时钟。开始：2026-10-12 15:00 +08:00；预估结束：2026-10-12 15:50 +08:00；实际结束：2026-10-12 15:12:25 +08:00；偏差 −37分35秒。估时依据：BF139.01 的同类 C++/ECS 消费链及已有独立生产库反例入口；本轮重用工具，只补遗漏边界。结果：审计 Gate 完成，确认 4 项 P2，未实施修复；无需拆分。重用生产库避免了完整构建，同类只读复审后续估时可缩短。
  - source/static 摘要：原始 Character/CMC、UE Character/CMC/Capsule/Actor 与 3 个 fresh 原生资产闭环；15 个实际消费者反例/对照为 9 PASS、6 预期语义 FAIL，归并为跳跃按住时钟、父级缩放地面尺寸、静止速度朝向旋转、方向配置 ActualGait 四项。已排除输入归一化、加速度 owner delta、基座升降起跳与默认 InAir 枚举差异；watchdog 自测通过。下一步只查动作/Root Motion/Ragdoll 闭包；生产实现及 BF139 已完成项保持不变。
- [x] BF140.02 动作 / Root Motion / Ragdoll：Roll、Mantle、GetUp、动作抢占与物理移交。开始：2026-10-12 15:14 +08:00；预估结束：2026-10-12 15:49 +08:00；实际结束：2026-10-12 15:26:28 +08:00；偏差 −22分32秒。估时依据：BF139.02–03 源闭包、本轮 .01 的生产库重用耗时与必要原生动作/PA 读取。结果：审计 Gate 完成，新增 2 项 P2，未实施修复；无需拆分。
  - source/static 摘要：fresh Actions/Mantling RMS/UE RootMotionSource、PA 与 4 个动作蒙太奇闭环；PA 21/20/49 与 Ragdoll/GetUp 设置重建一致。新增 8 个完整角色测试，累计 23 项为 12 PASS、11 预期语义 FAIL；新增 Root Motion 发布速度时钟和 Ragdoll 恢复 Pitch/Roll 两项。下一步仅沿 Character 动画输入到运动图/Rig 消费闭包审计；截图与性能暂停边界不变。
- [x] BF140.03 运动动画：AnimationInstance 输入、Grounded/Standing/Crouching/Air、转身/停止、图求值/曲线/同步及角色 Rig 消费边界。开始：2026-10-12 15:28 +08:00；预估结束：2026-10-12 16:13 +08:00；实际结束：2026-10-12 15:48:46 +08:00；偏差 −24分14秒。估时依据：BF139.04–05 完整生成闭包、核心原生重读与消费者反例。结果：11 个 fresh native、9 AnimBP / 155 子图 / 256 产物及 Rig 165 指令/79骨骼/43曲线闭环；未新增已确认 P2，无需拆分。转换规则未改，完整重转不适用。
  - source/static 摘要：247 份当前规格、完整默认角色 source closure、256 editor 资产、运行时资产边界与审计工具负例全部通过；CR_Als 重建完全一致。Air max-braking 候选没有实际消费证据，已排除；有效反例最终 23 项为 12 PASS / 11 预期语义 FAIL，六项 P2 均未修复。下一步为审计交付；不自动开始修复、截图或性能验收。

完成摘要：目标基线 `5952afcd`，原始 ALS 插件 `b754d6f0`；三个只读批次完成，确认六项 P2，未确认新 P0/P1。19 个 fresh native 与 15 个 C++/header 快照取证完成；C++ 固定读取原始插件根，资产均核对原始/Build 一致。BF139 已修问题只作去重，未修改生产代码、资产或转换器，无新增生产设计需要消融；截图关闭、性能验收继续暂停。

## BF141 运动系统复审差异修复

用户已于 2026-10-12 授权修复 BF140 报告的六项差异。按下列顺序逐项完成自己的 Gate 和提交；截图关闭，性能验收继续暂停，公共运行使用 `-CaptureCount 0 -FunctionalOnly`。旧 Gate 只对报告已证明的具体消费者有限复开，保留历史失败和通过记录。

- [x] BF141.01 / P2-01 跳跃按住使用 owner delta。开始：2026-10-12 16:00 +08:00；预估结束：2026-10-12 16:55 +08:00；实际结束：2026-10-12 16:37:17 +08:00；偏差 −17分43秒。依据：已有最小失败、BF139 角色时钟修复和 BF134 非零按住合同；范围仅 ALS Jump 输入、计时及原始入口。结果：修复及本项 Gate PASS，无需拆分；旧 Gate 的非零按住×角色倍率漏检已补入维护回归。
  - source/static 摘要（16:03）：当次原生 B_Als_Character 与原始 Character/UE Character/CMC/Actor tick 闭环；CMC 的 ClearJumpInput 消费 owner delta，Tnua action input 可逐帧更新。维护回归已稳定 FAIL：倍率 .5、第12帧仅经过角色 .1 秒时，速度4.0365而源应4.11825。下一步直接传递帧 delta 给现有 Jump input，并验证中途倍率变化，保留普通物理时钟。
  - 实现/消融摘要（16:20）：仅给现有 Jump input 增加当帧 delta 标量，首次启动与活动期间均写入 owner delta，由原 hold_elapsed 单独累计；没有额外时钟资源或修正计时的系统。恢复世界计时、去掉逐帧刷新两项消融均为语义 FAIL；精确字节恢复后同 target 的 6 项相关回归 PASS。消融首版夹具在 ctx 移交后读取字段导致编译失败，已纠正并排除该结果。下一步同启动器正式构建、声明式审计与 startup/base 功能 Gate。
  - 运行前闭包（16:30）：声明式 manifest、source 哈希、图位置、editor 与资产边界通过；同 `target_game`、play 优化级别3的 manual_playground/spec/test 正式构建通过，私有 Cargo 增量缓存用于后续编译，运行入口继承同一设置。两个消融对应失败与恢复字节已核对；开始 startup，再进入原 Base 动作流程，均关闭截图并只验收功能。
  - runtime 摘要（16:32）：startup 功能通过，graph/fault/temporal 为空、primary_failure=null，图根姿态有效；原三个生产库跳跃反例/对照全部 PASS。完整性能结果仍为 false，eval 预算仅延期、不修改阈值。接着执行原 Base 动作功能 Gate。
  - 完成摘要（16:37）：Base 原场景功能 PASS，53 项覆盖齐全，graph/fault/temporal 为空、primary_failure=null；截图为0，帧/动画求值预算保持延期。6项定向恢复回归、两个有效消融、原3个生产库反例/对照、声明式合同和正式构建均通过；源哈希及字节恢复记录在 `target/bf141/fix01/`，功能证据在 `target/animation_visual/bf141_fix01/`。下一项为世界胶囊尺寸。
- [x] BF141.02 / P2-02 普通 CMC 读取世界胶囊尺寸。开始：2026-10-12 16:40 +08:00；预估结束：2026-10-12 17:35 +08:00；实际结束：2026-10-12 17:50:45 +08:00；偏差 +15分45秒，在允许误差内。依据 BF139 父空间修复、.01 实测构建/消融成本与本轮等世界尺寸反例。结果：世界尺寸及必要的坐标写回闭环 PASS，无需拆分；额外 current-transform 漏检与夹具最终传播修正增加了工作量，同类估时需计入完整坐标消费者。
  - source/static 摘要（16:49）：fresh B_Als_Character、CapsuleComponent 的 world scaled getters、ALS ComputeFloorDist 和 UE Crouch/UnCrouch 闭环。新增维护反例先红：父级 .5 倍胶囊悬空约45cm，父级2倍无法通过40cm台阶；同世界尺寸的自身缩放对照通过。蹲起夹具目前被错误 Air 前置阻断，不将其计作独立语义失败；先修地面/步阶三个尺寸消费者，再检查 world shape scale 的蹲起净空消费者。
  - 根因补闭（17:06）：地面9组分解/缩放与台阶3组已 PASS；有效低顶棚反例仍 FAIL：父级2倍错误允许 Standing。fresh UE UnCrouch 明确读取世界位置/胶囊大小；当前 TransformHelper 反例另证实 CMC 把 world 写入 local，旧父空间 Gate 的缓存 Global/Physics 一致掩盖了当帧变换错误（current=(5,3.847,-3)、physics=(0,.924,0)）。有限复开该普通父空间测试的 current-transform 边界，修复 CMC/状态同步/蹲起的局部写回，并以正确 world 输入执行净空检查。
  - 实现/消融摘要（17:40）：world getter 与 inverse-parent 写回覆盖 CMC、状态同步、蹲起和基座携带；9组地面、3组台阶、3组低顶棚及整帧/物理前 current-transform 合同通过。7个消费者消融均语义 FAIL，精确字节恢复后同 target 10项回归 PASS。运动夹具补上与真实入口一致的最终 Transform 传播，排除新增移动输入后读取旧缓存造成的夹具假失败。真实 stairs 夹具以父级2/局部.5保持原角色世界大小1，新增实际 local/world scale 观测；下一步正式构建与 startup/stairs 功能 Gate。
  - 运行前闭包（17:48）：fresh 世界尺寸/相对写回 source、7个有效消融和字节恢复、10项回归、声明式合同、source 哈希及正式 target_game 构建全部通过；原资产与转换规则未改。开始 startup→stairs，三座原楼梯的六条路线保留世界角色尺寸1，实际父级分解由数值报告核对；截图0、仅功能验收。
  - 完成摘要（17:50）：原生产库反例转 PASS，startup/stairs 功能通过，三楼梯上下六路线及9项入口覆盖齐全；2275个实际样本 world scale=1、local scale=.5，原角色世界尺寸/相机/环境保持。graph/fault/temporal为空、primary_failure=null；原阈值和性能延期保留。证据在 `target/bf141/fix02/` 与 `target/animation_visual/bf141_fix02/`，下一项只处理静止 VelocityDirection 的半衰期分支。
- [x] BF141.03 / P2-03 静止 VelocityDirection 固定旋转 half-life。开始：2026-10-12 18:00 +08:00；预估结束：2026-10-12 18:35 +08:00；实际结束：2026-10-12 18:09:44 +08:00；偏差 −25分16秒。依据现成算法反例、BF139 旋转入口与前两项构建实测；结果：仅 stationary VelocityDirection 提前返回、Gate PASS，无需拆分。增量缓存与单分支修复缩短了正式构建，同类小分支估时可降低。
  - source/static 摘要（18:00）：fresh Character 静止 VelocityDirection 明确固定 .1 秒；只有 moving 计算函数应用 ViewState.YawSpeed 倍率。维护回归先红：镜头变化 .01 弧度，首帧 yaw=.117828，而源 .109366。修改现有提前返回条件，保留移动/ViewDirection 倍率；下一步静止两姿态×正负镜头速度、移动对照、消融与公共 standing_response 功能 Gate。
  - 实现/消融摘要（18:05）：两姿态×五档镜头变化的维护反例恢复通过，移除提前返回条件后稳定语义 FAIL；精确字节恢复、同 target 重编后 11项运动回归 PASS。已有移动响应测试扩展 VelocityDirection 两姿态及 moving/stationary 对照并 PASS，确认 moving/ViewDirection 仍有源镜头倍率。fresh MS_Als_Normal 与 Character、声明式 manifest/位置/editor/资产边界通过；下一步正式构建与公共 startup→standing_response。
  - 运行前闭包（18:08）：同 target_game/play 优化级别3正式构建通过，源码字节与消融恢复一致；source 哈希、声明式合同及相关回归已闭合。开始公共 startup→standing_response，保留原 Playground 角色/相机/环境，CaptureCount=0、FunctionalOnly；未展开相邻图或其他旋转分支。
  - runtime 摘要（18:09）：startup 功能通过，graph/fault/temporal为空、primary_failure=null；正式优化构建中的静止旋转维护回归亦 PASS。继续原 standing_response 场景，性能预算保持延期。
  - 完成摘要（18:09）：standing_response 原场景6项覆盖通过，图求值/姿态/时序有效，graph/fault/temporal为空、primary_failure=null。两姿态×五档静止镜头变化、移动对照、11项恢复回归、分支消融、正式构建和声明式合同通过；截图0、性能延期。证据在 `target/bf141/fix03/` 与 `target/animation_visual/bf141_fix03/`，下一项 ActualGait。
- [x] BF141.04 / P2-04 ActualGait 使用源最大前后速度阈值。开始：2026-10-12 18:11 +08:00；预估结束：2026-10-12 18:40 +08:00；实际结束：2026-10-12 18:29:54 +08:00；偏差 −10分06秒。依据独立 Character/CMC 合同、错误测试与 .03 增量编译实测；结果：公共 helper/运行消费者修复、Gate PASS，无需拆分。GaitAmount/限速保留方向插值，有限复开并重新验收旧 ActualGait oracle；夹具借用/文件恢复成本已计入实际记录。
  - source/static 摘要（18:15）：fresh Character、MovementSettings、CMC 与原生 MS_Als_Normal 闭环；Character 使用 max(forward,backward)+10cm，CMC 的 GaitAmount/限速保留方向插值。两个维护反例先红：运行系统倒退1.2误判 Running；公共 helper 在后退上限高于前进时误判 Running。只扩展现有 helper 的最大值合同并接回运行消费者，下一步前后方向/上下限/开关矩阵、独立消融与真实 standing_direction 功能。
  - 实现/消融摘要（18:22）：最大值开关/高低后退上限与7组方向速度矩阵通过；GaitAmount、方向限速及 sprint permission 对照保留。运行调用与最大值分支两项消融均语义 FAIL；第一次 Python 字节恢复遇到文件写入错误，经属性/读回诊断后改用原生 .NET 写回，SHA-256与备份一致，未回退mtime。同 target 重编后11项步态回归 PASS；测试夹具 AssetMut 借用作用域编译失败已修正且不计语义证据。有限复开归档 BF102.03 的 ActualGait oracle，保留其他历史证据；下一步正式构建、原生产库4个反例/对照、startup→standing_direction。
  - 运行前闭包（18:28）：正式 target_game/play 优化级别3构建通过；源哈希、声明式合同、两个有效消融、字节恢复及11项步态回归已闭合。开始公共 startup→standing_direction，原 Playground 的方向/源状态覆盖保持，截图0、仅功能验收；未改原始资产或转换器。
  - runtime 摘要（18:29）：正式库中的原四个步态反例/对照全部 PASS，startup 功能通过，graph/fault/temporal为空、primary_failure=null；开始 standing_direction，保留性能延期。
  - 完成摘要（18:29）：原 standing_direction 五项覆盖含六个源状态全部通过，graph/fault/temporal为空、primary_failure=null；11项步态回归、7组方向/GaitAmount/限速矩阵、最大值开关/后退较高对照、两个有效消融、正式库4个反例、声明式合同及正式构建通过。BF102.03 的 ActualGait oracle 重新验收，其他历史边界保留；截图0、性能延期。证据在 `target/bf141/fix04/` 与 `target/animation_visual/bf141_fix04/`，下一项 Roll 根运动速度。
- [ ] BF141.05a / P2-05 Roll 源速度与物理桥时钟。启动时登记开始/预估结束；依赖 .01–.04 Gate。依据 BF139 角色速度恢复桥、当前完整角色反例，预估窗口不超过 55 分钟。Gate：fresh UE CMC、根增量/位移/发布速度/退出惯性与倍率矩阵、消融、真实 Grounded/Roll 功能。
- [ ] BF141.05b / P2-05 Mantle 源速度与移交。启动时登记开始/预估结束；依赖 .05a 的共用速度语义。依据 BF139 父空间/基座修复和现有反例，预估窗口不超过 55 分钟。Gate：fresh Mantling RMS、静止/活动基座与父空间/倍率/终止矩阵、消融、真实 Mantle 功能。
- [ ] BF141.06 / P2-06 Ragdoll 恢复保留 actor Pitch/Roll。启动时登记开始/预估结束；依据已确认的源只改 Yaw 分支和 BF139 恢复夹具，预估窗口不超过 45 分钟。Gate：fresh Actions、倾斜/父空间/落地与空中恢复、消融、真实 Ragdoll/GetUp 功能。

活动摘要：工作区基线 `517dbb8d`；审计的23个有效反例/对照保留，六项修复均已授权。.01–.04 已通过自身 Gate 并分别提交，下一项 .05a Roll 根运动速度；不修改依赖或转换规则。
