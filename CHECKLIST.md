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

## 附录 B：风险与后置项

- **资产格式决策（2026-08-21）**：Bevy 官方推荐 glTF；M0.5/M0.6 管线定为 UE → glTF（Interchange 导出）→ bevy_animation_graph 自带的 glTF 导入器（GraphClip/Skeleton）。**不引入 bevy_ufbx**：其 v0.19.0（bevy 0.19 兼容）虽支持蒙皮数据，但动画曲线未接入 Bevy 动画系统、无社区验证，且 FBX 中转对 ALS 曲线/虚骨/单位换算风险更大；仅当未来需要直接加载 FBX 时再评估。

- bevy_animation_graph additive/mask 语义若与 ALS layering 不完全一致，在 M6 前验证并决定扩展方式（优先 als-ext 扩展点）。（已于 M6.1 验证通过，记录备查）
- Distance matching / motion matching 类高级特性仅实现 Stop 所需子集（M8.3）。
- ALSExtras（AI 控制器示例）不在范围内。
- 音频（Metasound）、Niagara 特效仅保留挂钩点。ALSCamera 完整移植已纳入 BF116.01。

## BF116 相机、Overlay 与场景 Actor 移植

BF115 已按用户指定截至 `BF115.09g2b` 归档，完整源事实、验收、失败和性能暂停记录见 [BF115 归档](docs/archive/2026-09-12/BF115-2026-09-12.md)。性能工作继续暂停。

以下为用户已明确授权的完整模块清单，按依赖顺序执行。未启动模块的开始/预估结束时间在进入 source 阶段时登记，并拆成可独立验收的执行子项；登记不代表已调研或完成。

| 编号 | 任务 | 验收边界 | 状态 |
|---|---|---|---|
| BF116.01 | `/ALS/ALSCamera` 全目录及 Actor 相机链 | 覆盖 `AB_Als_Camera`、`B_Als_CameraComponent`、`PA_Als_Camera`、骨架/网格、设置、混合曲线、震动等全部9个资产；完成 Actor 挂载、摇臂距离/偏移、滞后、碰撞回缩、视图切换、FOV和生命周期，并通过真实入口。 | 进行中 |
| BF116.02 | 12个 overlays 的移植和修复 | 逐图对照本地源，完成转换、实际动画/曲线/层混合及对应 Actor 消费和功能验收。 | 待执行 |
| BF116.03 | `OverlayObjects/Bow/AB_Als_Bow` | 完成弓物件动画图及其实际引用、挂载、输入、生命周期与 Actor 调用链。 | 待执行 |
| BF116.04 | `SM_Als_Stair` 上下楼梯 | 修复速度和动画表现；明确避免源版本上楼后永久改写并锁住 `rotation mode` 的 bug，进入/离开楼梯后快捷键仍可正常切换。 | 待执行 |
| BF116.05 | `B_Als_MovingObject2` 移动平台跟随 | 角色站在移动平台上能按源规则正确跟随，核对平台、运动基座与角色调用链。 | 待执行 |
| BF116.06 | `B_Als_RotatingObject` 旋转平台 | 平台旋转而角色未主动移动时，按源表现原地踏步保持位置，修复脚在平台上滑动的表现。 | 待执行 |
| BF116.07 | `B_Als_MovingObject1` 陷地 | 核对平台自身运动、位置和碰撞的源生命周期，修复陷入地面，与源场景表现一致。 | 待执行 |

### BF116.01 相机执行子项

- [x] BF116.01a AB_Als_Camera与B_Als_CameraComponent源闭包：开始2026-09-12 18:40:39、预估2026-09-12 19:25:39。依赖51acc0c及用户16:38授权，用户本轮进一步明确覆盖完整B_Als_CameraComponent和Actor相机挂载/摇臂逻辑。已定位Camera AnimBP/组件资产、589行源CameraComponent、NativeInitialize/Update、CameraSettings及CharacterExample的Mesh挂载/相对旋转/CalcCamera输出；目标仍以独立curve profile system驱动。先健康检查Editor并提取当次完整原生图/CDO/递归资产，核对视图/旋转/姿态/步态/动作/肩侧输入、动画曲线时序、挂载/初始化/销毁、pivot/距离偏移/滞后/碰撞回缩/FOV/第一第三人称/震动及时间膨胀，建立完整source-target差异合同。首个明确事实为第一人称Camera动画使用DesiredRotationMode，而非被强制的当前ViewDirection；尚不据此局部修改。估时依据为首个AnimBP、组件/Actor及设置直接依赖、原生reader每份约35–78秒和现有相机入口；源闭包明确后按转换/资产、图曲线消费、组件算法与真实Actor装配拆成独立验收段。当前只做源/失败合同，不启动游戏、不改已完成角色图或其它用户新模块。
  - 用户范围补充/首轮源闭包2026-09-12 18:52:00：明确纳入PA_Als_Camera及整个/ALS/ALSCamera目录；实际共9个资产：AB、B组件、SK、SKM、PA、CS_Default、Quick/Smooth两CF及Sprint CameraShake，并沿挂载引用补取B_Als_Character。MCP健康0.590s通过，OpenEditorForAsset成功，图列表/图读取正常但AnimGraph DSL为空，采用同次完整native保留结构。首两份33.065/32.089s通过，Camera为8个可达子图/107节点（含4注释），另有空EventGraph；源包含肩侧/视图/动作以及三种Look状态内的移动分支，目标只有三组固定配置和统一0.5秒线性混合。已读完整CameraComponent、头文件/Settings、CharacterExample挂载/CalcCamera及Shake通知；第一人称DesiredRotationMode在目标已有对应选择，不作为差异修复。当前其余原生资产继续提取，尚未修改生产代码或运行游戏。
  - 01a完整目录/转换失败2026-09-12 19:04:00：9个目录资产及Actor共10份原生记录全部通过（31–36秒/份）。Camera骨架/网格为cameraboom_root→cameraboom_end两骨；PA为一个根骨盒体、零约束，但Actor实际B_Camera组件BodyInstance为NoCollision且不模拟，不能用动态碰撞摇臂替代。真实组件继承B_Als_CameraComponent，挂在CharacterMesh0、相对Yaw90度，引用完整AB/SKM/CS；公开uasset转换106.296s按预期阻断，28项问题仅属LocalRefPose、BlendListByBool及其绑定、ResetChildOnActivate、HermiteCubic四组，未输出部分RON。源Bool明确true→0/false→1，肩侧Scale实际输入pin为+1/−1；六条Look转换按Quick0.35秒或Smooth1秒，不能合并成目标0.5秒线性。完整组件还证明偏移/滞后需源坐标与单位数据、源InvExpApprox和逆时针角度边界，CameraShake允许多实例；目标需按这些实际差异逐段修复。Camera GetSocketLocation直接依赖角色SK/SKM的三个相机插槽，现将这两个只读源资产纳入本段闭包，角色动画/材质算法不复开。
  - 01a完成2026-09-12 19:22:57：角色SK/SKM追加源记录33.962/32.955s通过，三个head相机插槽与现有目标偏移逐项一致，予以复用。目录、12份native、9张live图（8可达及空EventGraph）/107节点、两骨、一PA盒体、真实NoCollision实例及28项转换阻断的交叉检查0.567s PASS，无部分RON。补读Engine RefPose/BlendList/AlphaBlend及CameraManager→Shake/Wave/ApplyOffset完整调用链：ResetChildOnActivate仅在新分支原权重不相关时初始化，HermiteCubic可复用已有smoothstep；Shake在游戏时钟按实例顺序叠加CameraLocal结果，和组件的未缩放动画/滞后时钟分开。现有LocalRefPose辅助节点只输出空Pose，不能直接冒充源两骨参考姿态。开始18:40:39、预估19:25:39、实际19:22:57、偏差-162s，源与失败合同段完成；范围已包含整个ALSCamera和Actor挂载输出。后续按通用Bool/Hermite、参考姿态/子分支重置、九资产离线导入、实际图/组件/Actor消费及公共Gate分别执行，不以现有固定profile或元数据当作完成。19:14 MCP再次连接拒绝，但完整当次源已保存，不阻塞本地转换修复；尚未改生产代码或启动新Playground。
- [x] BF116.01b1 通用Bool与Hermite混合转换：开始2026-09-12 19:26:08、预估2026-09-12 20:16:08。依赖7dd8c37的完整Camera源闭包及106.296s公开转换失败；本段只解决BlendListByBool的真实bool绑定、true→0/false→1、每目标blend time/Custom CF及HermiteCubic枚举映射。复用现有source Pose/Curve混合、BlendList权重状态及曲线解析/采样，先源合同/真实图反例，再最小通用实现与消融；不伪造GameplayTag选择，也不按Camera资产/GUID分支。每次共享规则最终修复后对全部9个已生成AnimBP及递归依赖做完整重转并比较全256产物，核对影响后再迁移；Camera的LocalRefPose和ResetChildOnActivate仍明确阻断，不输出部分RON。估时依据为已有SourceGameplayTagBlendNode/权重状态、Cubic已用相同smoothstep、原生bool/AlphaBlend/三键CF合同及约30秒扩展回归/33秒全量转换记录；本段不运行游戏，实际九资产导入和Actor/组件完整接入在后续依赖段完成。
  - 01b1转换/实际图闭环2026-09-12 20:17:15：Hermite合同/renderer反例0.306/0.309s先FAIL，映射到已有smoothstep后0.308/0.307s PASS；全9重转46.020s及完整256产物比较0.837s无变化。Bool合同反例0.308s FAIL后0.299s PASS，明确可见blend-time pin为0.5而Node存储默认0.1；绑定、常量及换图/GUID保持语义。新增Bool节点复用原BlendList权重状态与Pose/Curve混合、现有RichCurve/AlphaBlend采样，未伪造Tag；旧十项混合回归69.069s、实际图Pose/Curve/正反四元数29.055s及Custom中断反向18.665s通过，renderer/转换器0.307/0.576s通过。当前源Camera候选4.242s只剩19个LocalRefPose和6个子分支重置阻断，未输出RON；选择反号消融18.908s按预期失败，继续Custom曲线消融及最终全量转换/回归。没有安装资产或新Playground运行。
  - 01b1完成2026-09-12 20:30:32：Custom曲线移除19.082s稳定FAIL；恢复后374扩展23.174s、323项目84.437s全部通过，Bool经反射RON往返后真实图Pose/Curve与中断反向20.587s通过。删除从FSM计划额外查找曲线的分支，直接复用统一CurveFloat读取；4.519s候选报告与改前完全相同，故保留更少代码。最终四项转换自测及全9重转45.196s、全256产物比较0.817s均PASS，增/删/改全部0；恢复后同类全量检查46.922s/0.830s亦保持。复用一个权重状态、原Pose/曲线上下文和RichCurve公式，无新Tag镜像、依赖或安装资产；只新增源Bool节点及唯一的通用转换规则。开始19:26:08、预估20:16:08、实际20:30:32、偏差+864s，本段静态完成；Camera仍有两类25项明确阻断，完整目录资产与Actor/组件接入未完成，不启动新游戏或宣称相机通过。
