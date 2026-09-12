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

以下为用户已明确授权的完整模块清单，按依赖顺序执行。未启动模块的开始/预估结束时间在进入 source 阶段时登记，并拆成可独立验收的执行子项；登记不代表已调研或完成。

| 编号 | 任务 | 验收边界 | 状态 |
|---|---|---|---|
| BF116.01 | `/ALS/ALSCamera` 全目录及 Actor 相机链 | 覆盖 `AB_Als_Camera`、`B_Als_CameraComponent`、`PA_Als_Camera`、骨架/网格、设置、混合曲线、震动等全部9个资产；完成 Actor 挂载、摇臂距离/偏移、滞后、碰撞回缩、视图切换、FOV和生命周期，并通过真实入口。 | 进行中 |
| BF116.02 | `SM_Als_Stair` 上下楼梯 | 修复速度和动画表现；明确避免源版本上楼后永久改写并锁住 `rotation mode` 的 bug，进入/离开楼梯后快捷键仍可正常切换。 | 待执行 |
| BF116.03 | `B_Als_MovingObject2` 移动平台跟随 | 角色站在移动平台上能按源规则正确跟随，核对平台、运动基座与角色调用链。 | 待执行 |
| BF116.04 | `B_Als_RotatingObject` 旋转平台 | 平台旋转而角色未主动移动时，按源表现原地踏步保持位置，修复脚在平台上滑动的表现。 | 待执行 |
| BF116.05 | 弓箭 Overlay 与物件动画整套 | 一并移植 `Overlays/AB_Als_Bow` 和 `OverlayObjects/Bow/AB_Als_Bow`，核对两图联动、实际动画/曲线/层混合，以及弓物件引用、挂载、输入、生命周期与 Actor 调用链。 | 待执行 |
| BF116.06 | 剩余11个 overlays 的移植和修复 | 除前项弓箭 Overlay 外，逐图对照本地源，完成转换、实际动画/曲线/层混合及对应 Actor 消费和功能验收。 | 待执行 |
| BF116.07 | Footprint 移植 | 对照本地源完成 Footprint 资源、触发与足迹显示、Actor/通知消费及生成/清理生命周期，并通过真实场景验收。 | 待执行 |
| BF116.08 | `B_Als_MovingObject1` 陷地 | 核对平台自身运动、位置和碰撞的源生命周期，修复陷入地面，与源场景表现一致。 | 待执行 |

### BF116.01 相机执行子项

- [x] BF116.01a AB_Als_Camera与B_Als_CameraComponent源闭包：开始2026-09-12 18:40:39、预估2026-09-12 19:25:39。依赖51acc0c及用户16:38授权，用户本轮进一步明确覆盖完整B_Als_CameraComponent和Actor相机挂载/摇臂逻辑。已定位Camera AnimBP/组件资产、589行源CameraComponent、NativeInitialize/Update、CameraSettings及CharacterExample的Mesh挂载/相对旋转/CalcCamera输出；目标仍以独立curve profile system驱动。先健康检查Editor并提取当次完整原生图/CDO/递归资产，核对视图/旋转/姿态/步态/动作/肩侧输入、动画曲线时序、挂载/初始化/销毁、pivot/距离偏移/滞后/碰撞回缩/FOV/第一第三人称/震动及时间膨胀，建立完整source-target差异合同。首个明确事实为第一人称Camera动画使用DesiredRotationMode，而非被强制的当前ViewDirection；尚不据此局部修改。估时依据为首个AnimBP、组件/Actor及设置直接依赖、原生reader每份约35–78秒和现有相机入口；源闭包明确后按转换/资产、图曲线消费、组件算法与真实Actor装配拆成独立验收段。当前只做源/失败合同，不启动游戏、不改已完成角色图或其它用户新模块。
  - 用户范围补充/首轮源闭包2026-09-12 18:52:00：明确纳入PA_Als_Camera及整个/ALS/ALSCamera目录；实际共9个资产：AB、B组件、SK、SKM、PA、CS_Default、Quick/Smooth两CF及Sprint CameraShake，并沿挂载引用补取B_Als_Character。MCP健康0.590s通过，OpenEditorForAsset成功，图列表/图读取正常但AnimGraph DSL为空，采用同次完整native保留结构。首两份33.065/32.089s通过，Camera为8个可达子图/107节点（含4注释），另有空EventGraph；源包含肩侧/视图/动作以及三种Look状态内的移动分支，目标只有三组固定配置和统一0.5秒线性混合。已读完整CameraComponent、头文件/Settings、CharacterExample挂载/CalcCamera及Shake通知；第一人称DesiredRotationMode在目标已有对应选择，不作为差异修复。当前其余原生资产继续提取，尚未修改生产代码或运行游戏。
  - 01a完整目录/转换失败2026-09-12 19:04:00：9个目录资产及Actor共10份原生记录全部通过（31–36秒/份）。Camera骨架/网格为cameraboom_root→cameraboom_end两骨；PA为一个根骨盒体、零约束，但Actor实际B_Camera组件BodyInstance为NoCollision且不模拟，不能用动态碰撞摇臂替代。真实组件继承B_Als_CameraComponent，挂在CharacterMesh0、相对Yaw90度，引用完整AB/SKM/CS；公开uasset转换106.296s按预期阻断，28项问题仅属LocalRefPose、BlendListByBool及其绑定、ResetChildOnActivate、HermiteCubic四组，未输出部分RON。源Bool明确true→0/false→1，肩侧Scale实际输入pin为+1/−1；六条Look转换按Quick0.35秒或Smooth1秒，不能合并成目标0.5秒线性。完整组件还证明偏移/滞后需源坐标与单位数据、源InvExpApprox和逆时针角度边界，CameraShake允许多实例；目标需按这些实际差异逐段修复。Camera GetSocketLocation直接依赖角色SK/SKM的三个相机插槽，现将这两个只读源资产纳入本段闭包，角色动画/材质算法不复开。
  - 01a完成2026-09-12 19:22:57：角色SK/SKM追加源记录33.962/32.955s通过，三个head相机插槽与现有目标偏移逐项一致，予以复用。目录、12份native、9张live图（8可达及空EventGraph）/107节点、两骨、一PA盒体、真实NoCollision实例及28项转换阻断的交叉检查0.567s PASS，无部分RON。补读Engine RefPose/BlendList/AlphaBlend及CameraManager→Shake/Wave/ApplyOffset完整调用链：ResetChildOnActivate仅在新分支原权重不相关时初始化，HermiteCubic可复用已有smoothstep；Shake在游戏时钟按实例顺序叠加CameraLocal结果，和组件的未缩放动画/滞后时钟分开。现有LocalRefPose辅助节点只输出空Pose，不能直接冒充源两骨参考姿态。开始18:40:39、预估19:25:39、实际19:22:57、偏差-162s，源与失败合同段完成；范围已包含整个ALSCamera和Actor挂载输出。后续按通用Bool/Hermite、参考姿态/子分支重置、九资产离线导入、实际图/组件/Actor消费及公共Gate分别执行，不以现有固定profile或元数据当作完成。19:14 MCP再次连接拒绝，但完整当次源已保存，不阻塞本地转换修复；尚未改生产代码或启动新Playground。
- [x] BF116.01b1 通用Bool与Hermite混合转换：开始2026-09-12 19:26:08、预估2026-09-12 20:16:08。依赖7dd8c37的完整Camera源闭包及106.296s公开转换失败；本段只解决BlendListByBool的真实bool绑定、true→0/false→1、每目标blend time/Custom CF及HermiteCubic枚举映射。复用现有source Pose/Curve混合、BlendList权重状态及曲线解析/采样，先源合同/真实图反例，再最小通用实现与消融；不伪造GameplayTag选择，也不按Camera资产/GUID分支。每次共享规则最终修复后对全部9个已生成AnimBP及递归依赖做完整重转并比较全256产物，核对影响后再迁移；Camera的LocalRefPose和ResetChildOnActivate仍明确阻断，不输出部分RON。估时依据为已有SourceGameplayTagBlendNode/权重状态、Cubic已用相同smoothstep、原生bool/AlphaBlend/三键CF合同及约30秒扩展回归/33秒全量转换记录；本段不运行游戏，实际九资产导入和Actor/组件完整接入在后续依赖段完成。
  - 01b1转换/实际图闭环2026-09-12 20:17:15：Hermite合同/renderer反例0.306/0.309s先FAIL，映射到已有smoothstep后0.308/0.307s PASS；全9重转46.020s及完整256产物比较0.837s无变化。Bool合同反例0.308s FAIL后0.299s PASS，明确可见blend-time pin为0.5而Node存储默认0.1；绑定、常量及换图/GUID保持语义。新增Bool节点复用原BlendList权重状态与Pose/Curve混合、现有RichCurve/AlphaBlend采样，未伪造Tag；旧十项混合回归69.069s、实际图Pose/Curve/正反四元数29.055s及Custom中断反向18.665s通过，renderer/转换器0.307/0.576s通过。当前源Camera候选4.242s只剩19个LocalRefPose和6个子分支重置阻断，未输出RON；选择反号消融18.908s按预期失败，继续Custom曲线消融及最终全量转换/回归。没有安装资产或新Playground运行。
  - 01b1完成2026-09-12 20:30:32：Custom曲线移除19.082s稳定FAIL；恢复后374扩展23.174s、323项目84.437s全部通过，Bool经反射RON往返后真实图Pose/Curve与中断反向20.587s通过。删除从FSM计划额外查找曲线的分支，直接复用统一CurveFloat读取；4.519s候选报告与改前完全相同，故保留更少代码。最终四项转换自测及全9重转45.196s、全256产物比较0.817s均PASS，增/删/改全部0；恢复后同类全量检查46.922s/0.830s亦保持。复用一个权重状态、原Pose/曲线上下文和RichCurve公式，无新Tag镜像、依赖或安装资产；只新增源Bool节点及唯一的通用转换规则。开始19:26:08、预估20:16:08、实际20:30:32、偏差+864s，本段静态完成；Camera仍有两类25项明确阻断，完整目录资产与Actor/组件接入未完成，不启动新游戏或宣称相机通过。
- [x] BF116.01b2 相机网格与参考骨架资产：开始2026-09-12 20:55:00、预估2026-09-12 21:45:00。依赖01a/01b1；fresh RefPose源码要求实际参考骨骼，当前LocalRefPose辅助节点为空Pose，现有SkeletonLoader从glTF场景构建骨架，因此先沿项目已有UE glTF导出/骨架描述流程导入SKM_Als_Camera和SK_Als_Camera。保留实际两骨层级、参考变换、几何与材质引用，和源PA引用/实际NoCollision边界；不手造空骨架或替代网格。先复核健康Editor/原生数据并导出独立原始记录，验证坐标单位、骨骼与目标加载；源到target的转换由可复跑工具执行。估时依据为两个已提取源资产、既有glTF导出器和SkeletonLoader、约33秒原生读取及最近20–84秒加载/编译记录。本段只完成资产依赖，参考姿态节点与子分支重置在后续独立Gate闭合；不启动相机Playground或提前研究后续模块。
  - 01b2资产/加载闭环2026-09-12 21:36:00：Editor健康1.892s，MCP工具无网格导出接口后使用本地UE glTFExporter，89.627s完成新只读导出；实际两骨、888顶点、1264三角、零动画，二进制与仓库既有网格一致。新JSON仅补出M_Als_Main名称与metallicFactor=0；既有原始模型保持不变，材质引用/实际实例覆盖留待组件装配。离线工具核对源骨名、父级、坐标单位及参考变换，只生成独立static.skn.ron；移除复制/覆盖原始网格的多余步骤。旧加载器依赖AnimationPlayer，无动画资产53.910s真实加载失败；通过公开Skeleton API按源根骨构建，36.035s真实资产加载通过。当前只补参考骨架依赖，继续层级变换反例、加载入口消融与扩展回归；Camera转换仍有19个LocalRefPose和6个子分支重置阻断，未安装部分图或启动游戏。
  - 01b2完成2026-09-12 21:39:03：移除静态加载入口26.930s稳定重现原加载器失败，恢复后376项扩展回归28.059s全部通过；包含真实零动画相机资产、非单位父子变换及缺失/重名根骨拒绝。导入自测拒绝六类错误骨名/层级/坐标变换，移除验证亦按预期失败；重复导入产物一致，原始glTF/bin无修改。复用现有Skeleton与glTF数据，只增加明确根骨的加载入口和离线描述生成，未改依赖、共享图转换规则或旧角色资产。开始20:55:00、预估21:45:00、实际21:39:03、偏差-357s，本段资产依赖完成；M_Als_Main实例引用、PA/组件装配和完整图仍在后续闭环。下一段处理源LocalRefPose的实际骨骼输出，再处理ResetChildOnActivate。
- [x] BF116.01b3 通用源参考姿势节点与转换：开始2026-09-12 21:40:44、预估2026-09-12 22:30:44。依赖de57c65的真实两骨参考骨架及01a完整源图；本段解决19个LocalRefPose转换阻断。回读本地AnimNode_RefPose实际Pose/Curve/时间语义，复用Skeleton默认变换和现有参考骨骼填充，通过源Skeleton identity解析真实资产；先对实际骨骼输出/源合同建立反例，再唯一通用映射、实际图求值及消融。共享规则修复后必须全9个已生成AnimBP及递归依赖重转，比较全部256产物确定影响。估时依据为单节点类别、一个已通过加载Gate的两骨资产、现有参考Pose与上下文工具、约28秒扩展回归和46秒全量转换；ResetChildOnActivate继续独立阻断，不输出部分Camera图或启动Playground。
  - 01b3源/反例闭环2026-09-12 21:54:35：回读本地AnimNode_RefPose及FPoseContext，源LocalSpace按实际必需骨骼输出参考变换；相机闭包没有Additive consumer，19节点均为LocalSpace。旧空Pose真实求值30.665s失败；新源节点读取Skeleton全部默认变换（包含实际根骨）、独立空曲线上下文，33.821s通过。合同/renderer/resolver先0.294/0.306/0.302s FAIL后0.300/0.296/0.297s PASS，按TargetSkeleton与离线描述的source_skeleton精确绑定，不按资产名或GUID写分支。6.632s相机候选只剩6处ChildUpateMode阻断，无部分RON。首次全9转换42.554s因不可达默认Overlay的骨架被资产解析器误要求而阻断；核对完整源可达性：角色该RefPose为0/1可达，相机为19/37可达（其余为编译重复图）。用原可达闭包统一限制全部类别的资产解析，反例0.295s FAIL→0.296s PASS；全部9图再次重转中，不缩小转换集合。5.310s相机候选仍精确保留6处重置阻断，旧产物未改，继续全产物diff和节点消融。
  - 01b3完成2026-09-12 22:01:34：全部9个已生成AnimBP及递归依赖44.787s重转通过，全256产物0.819s比较增/删/改均0；角色不可达默认Overlay没有复开或安装。全部19个可达RefPose逐节点合同/渲染/真实Skeleton绑定通过，剩余仅6个ChildUpateMode问题且零部分RON。复用旧参考骨骼填充函数的消融22.792s证明会丢失实际根骨；移除空曲线上下文23.256s使实际linked-input合并错误继承外部曲线，按预期失败。恢复后377扩展测试23.105s全通过，含根/子骨位置旋转缩放、时间、缺失骨架可读失败、两个节点反射注册及上下文消费。保留最小新源节点与唯一类别映射，旧合成conduit辅助节点未改语义；骨架解析只消费离线身份数据。开始21:40:44、预估22:30:44、实际22:01:34、偏差-1750s，本段完成，下一段关闭子分支激活重置，不宣称整个相机或真实入口通过。
  - 01b3有限复开2026-09-12 22:32:20：01b4消除6处重置阻断后，5.561s完整物化首次触发render入口的SUPPORTED_RENDERERS检查，LocalRefPose已有合同/inner实现但漏列支持集合；旧Gate只覆盖逐节点inner渲染，因重置阻断未覆盖完整图render。源LocalRefPose→唯一registry→inner→公开render链明确证明本条登记遗漏，现取消勾选，仅补全图渲染反例和支持声明；不复开旧角色图或资产。
  - 01b3复开关闭2026-09-12 22:45:11：完整render反例0.308s先FAIL，补唯一支持声明后0.297s PASS；新读本地Camera及两CF的公开uasset转换121.183s通过，产出完整14个RON、19个真实骨架RefPose和6个重置节点，与本轮已有完整候选逐字节一致。共享规则全9递归重转42.491s通过，全256产物1.356s比较无增删改。复开本轮开始22:32:20、预估22:42:20、实际22:45:11、偏差+171s；旧Gate漏检边界已落实为完整render自测，本条重新勾选。
- [x] BF116.01b4 通用BlendList子分支激活重置：开始2026-09-12 22:02:00、预估2026-09-12 22:52:00。依赖a7515ca及相机仅余的6处ChildUpateMode阻断；回读源BlendListBase的重置条件/权重相关性与Initialize链，先验证切换到无权重分支、正在混合的分支和切回的实际子节点状态/时间，再复用已有公开分支重置能力。只按源字段建立通用规则，保留Bool/GameplayTag/Enum等同源BlendList的一致语义。先失败合同与真实图反例，再实现、消融、全9图递归重转及完整产物diff；已有默认模式保持其已验证行为。估时依据为单个引擎基类、6个相机实例、现有TwoWay激活重置/混合权重和约23秒扩展测试、45秒全量转换。本段完成转换和动态子分支语义，完整相机资产/组件/Actor装配与实际入口仍在后续依赖段进行。
  - 01b4静态/动态闭环2026-09-12 22:45:11：本地BlendListBase及VectorRegister证明仅在active index变化且新分支原权重≤0.00001时Initialize，正在混合的分支不重置。三类BlendList共享状态与公开GraphContext初始化边界，清理整条已求值分支的状态/缓存，同时保留内部子上下文归属；不改依赖或旧TwoWay限制。12组合测试先39.512s失败，基础重置34.094s通过；连续深层重入44.794s再现旧时间0.3而应0.1，保留内部归属并合并Enum重复状态后39.658s通过。379扩展30.107s通过；去掉分支作用域36.409s按预期失败。合同0.310s/renderer0.571s先FAIL后0.306/0.302s PASS，Default不增加目标字段。最新公开源全转换14个RON已无阻断，继续三类嵌套用例、内部归属消融和最终扩展/项目回归；组件、Actor、PA/设置/震动资产与真实入口尚未接入。
  - 01b4完成2026-09-12 22:51:07：移除内部归属保留32.250s稳定重现深层重入时间错误；恢复后379扩展23.806s、323项目85.278s全部通过，嵌套内外三种BlendList均覆盖Default续播、重新激活、混合中切回及多层反复进入，另覆盖0.00001权重边界。复用同一BlendList状态、已有GraphContext/GraphState，删除Enum重复状态及重复上下文读取分支；默认模式不建立新作用域，不改旧TwoWay或依赖。共享规则最终全9重转42.491s与全256产物1.356s比较无增删改，新读本地源121.183s产出的14个Camera RON与完整候选一致，19个RefPose和6个reset字段逐项核对。开始22:02:00、预估22:52:00、实际22:51:07、偏差-53s，本段结构转换与子分支生命周期完成；下一段依据组件consumer完成曲线/设置/PA/震动的目标坐标和单位数据，再安装完整图并装配Actor，当前未启动新Playground。
