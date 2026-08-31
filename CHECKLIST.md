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

## 模块依赖顺序

```
M0 环境与资产管线 ─► M1 ECS核心状态模型 ─► M2 曲线基础设施 ─► M3 tnua运动基座
─► M4 动画图基座 ─► M5 Locomotion状态机动画 ─► M6 Overlay/Aim ─► M7 FootIK/CR等价物
─► M8 特殊动作(跳跃翻滚) ─► M9 Ragdoll ─► M10 相机 ─► M11 调试与总验收
─► BF103 Notify 事件管线（已归档） ─► BF104 方向／步态闭环（已归档） ─► BF105 蹲起事务（已归档）
─► BF106 Foot IK 闭环（已归档） ─► BF107 状态机语义与持续回归（按用户指令归档，未完成）
```

## BF108 BlendSpace 源资产逐项等价修复

> 范围：以本地 ALS-Refactored 的 9 个 BlendSpace 为逐项源事实，修正实际运行的 `.animgraph.ron` 节点、其项目本地 `ClipNode` 引用、图内轴／样本／播放设置及 Rust 输入 writer。`assets/animations/blendspaces/*.json` 只是导出元数据，不是本段验收对象。每个子项开始时重新提取该资产的源事实；不得以此前报告或记忆代替。本批按用户指令仅做静态验证：不编译、不运行、不执行 `manual_playground`。

- [x] BF108.01 `BS_Als_Air_Lean`：核对并修复 Air Fall／Jump 实际图的轴、5 个项目 ClipNode、坐标、loop／播放与 locomotion 输入 writer；验收为静态源对照零差异。
  - 当次 ALS 事实（2026-08-31）：源 X/Y 分别为 Lean Right Amount／Lean Forward Amount，范围 [-1,1]；中心、前、后、左、右五个样本坐标为 (0,0)/(0,1)/(0,-1)/(-1,0)/(1,0)，rate scale=1。资产 loop=false，但 `AB_Als_Locomotion` 在 Fall 与 Jump 的两个 evaluator 均覆盖为 loop=true、rate=1、start=0、DoNotSync；轴 smoothing time=0。
  - 实际图静态证据：`air_lean.animgraph.ron` 的五个 `ClipNode` 均指向项目本地对应 `.anim.ron`，坐标、`LoopNode`、`LinearizedInterpolate` 与 Absolute sync 完整；`state_fall`／`state_jump` 都链接该图；`locomotion_layer.rs` 将 `runtime.air_lean.y` 写入 right、`.x` 写入 forward。`python tools/import/verify_runtime_blendspace.py --source-root <ALS Content/ALS> --asset BS_Als_Air_Lean` 输出 pass=true。按本批用户指令，未编译、未运行。
- [x] BF108.02 `BS_Als_Lean`：核对并修复 Standing／Crouching 实际图的轴、5 个项目 ClipNode、坐标、evaluator／播放与两侧输入 writer；验收为静态源对照零差异。
  - 当次 ALS 事实（2026-08-31）：X/Y 为 Lean Right Amount／Lean Forward Amount，范围 [-1,1]、grid=4；五个样本坐标为 (0,0)/(0,1)/(0,-1)/(-1,0)/(1,0)，rate scale=1。资产 loop=false；Standing 与 Crouching 的 source evaluator 都是 loop=true、rate=1、start=0、DoNotSync；轴 smoothing time=0。
  - 实现：`crouching/lean.animgraph.ron` 已有 `als_crouching_lean_right_amount`／`forward_amount` 输入，但 `CrouchingGraphInputs` 与 writer 未写入它们，运行时一直以零输入求值。现将 `runtime.lean.y/x` 纳入 inputs 并写到对应全局 key；Standing 的同源映射本已存在。
  - 实际图与资源静态证据：两张 Lean 图均保留五个项目本地 `ClipNode`、坐标、Absolute BlendSpace、显式 evaluator time；Crouching／Standing 路由和两个 writer 均被检查。每个本地 `.anim.ron` 的 glTF path、animation name、时长与 skeleton 文件也逐样本对照源 `AnimSequence`。`python tools/import/verify_runtime_blendspace.py --source-root <ALS Content/ALS> --asset BS_Als_Lean` 输出 pass=true；Air 资源回归也为 pass=true。按本批用户指令，未编译、未运行。
- [x] BF108.03 `BS_Als_WalkRun_Backward`：核对并修复实际 Backward node 的四个项目 ClipNode、坐标、Cubic axis 语义、loop／Movement sync／start phase 与路由；验收为静态源对照零差异。
  - 当次 ALS 事实（2026-08-31）：Stride Amount／Walk Run Amount 均为 [0,1]；X/Y 为 Cubic `.2s`／`.4s`。四个样本为 Walk_Backward(1,0)、Walk_Forward_Pose(0,0)、Run_Backward(1,1)、Run_Forward_Pose(0,1)，rate scale=1。source player 为 loop=true、rate=1、start=.2、Movement SyncGroup；asset loop=true、marker sync=true。
  - 实现：实际 `state_detail_cycle` 原先以单窗口 `.4s` `CubicVec2FilterNode` 过滤两个轴，X 与 ALS 的 `.2s` 不等价。通用的独立轴滤波属于 `als-ext`，故新增 `CubicVec2AxisFilterNode` 并注册；实际图改接 X `.2s`／Y `.4s`。ALS 专用 Movement start phase、marker phase 和输入 writer 继续保留在项目层。
  - 实际图与资源静态证据：Backward `NormalizedBlendSpaceNode` 的四个项目本地 clip、坐标、loop、duration、Left／Right marker 时间、selector=3、Movement phase、`.2` start phase、root delta writer 与路由均逐项对照 source。`python tools/validate_animgraph_editor_assets.py --root assets/graphs --fix` 与无 `--fix` 均为 160 files、0 repairs、0 failures；`python tools/import/verify_runtime_blendspace.py --source-root <ALS Content/ALS> --asset BS_Als_WalkRun_Backward` 输出 pass=true。按本批用户指令，未编译、未运行。
- [x] BF108.04 `BS_Als_WalkRun_Forward`：核对并修复实际 Forward node 的四个项目 ClipNode、坐标、Cubic axis 语义、loop／Movement sync／start phase 与路由；验收为静态源对照零差异。
  - 当次 ALS 事实（2026-08-31）：Stride Amount／Walk Run Amount 均为 [0,1]，Cubic filter 为 X `.2s`／Y `.4s`；样本为 Walk_Forward(1,0)、Walk_Forward_Pose(0,0)、Run_Forward(1,1)、Run_Forward_Pose(0,1)，rate scale=1。source player 为 loop=true、rate=1、start=.2、Movement SyncGroup，asset loop=true、marker sync=true。
  - 实际图与资源静态证据：Forward `NormalizedBlendSpaceNode` 对照四个本地 clip、坐标、loop、每条资源的 glTF animation／时长／skeleton、图内 duration／markers、selector=0、Movement phase、start phase、双轴 Cubic filter、root delta writer 与路由，均无差异。`python tools/import/verify_runtime_blendspace.py --source-root <ALS Content/ALS> --asset BS_Als_WalkRun_Forward` 输出 pass=true。按本批用户指令，未编译、未运行。
- [x] BF108.05 `BS_Als_WalkRun_Left_Backward`：核对并修复实际 LeftBackward node 的四个项目 ClipNode、坐标、Cubic axis 语义、loop／Movement sync／start phase 与路由；验收为静态源对照零差异。
  - 当次 ALS 事实（2026-08-31）：Stride Amount／Walk Run Amount 均为 [0,1]，Cubic filter 为 X `.2s`／Y `.4s`；样本为 Walk_Left_Backward(1,0)、Walk_Right_Pose(0,0)、Run_Left_Backward(1,1)、Run_Right_Pose(0,1)，rate scale=1。source player 为 loop=true、rate=1、start=.2、Movement SyncGroup，asset loop=true、marker sync=true。
  - 实际图与资源静态证据：Left Backward node 的四条本地资源、坐标、loop、gltf animation／时长／skeleton、图内 duration／markers、selector=4、Movement phase、start phase、已修正的双轴 Cubic filter、root delta writer 与路由均无差异。`python tools/import/verify_runtime_blendspace.py --source-root <ALS Content/ALS> --asset BS_Als_WalkRun_Left_Backward` 输出 pass=true。按本批用户指令，未编译、未运行。
- [x] BF108.06 `BS_Als_WalkRun_Left_Forward`：核对并修复实际 LeftForward node 的四个项目 ClipNode、坐标、Cubic axis 语义、loop／Movement sync／start phase 与路由；验收为静态源对照零差异。
  - 当次 ALS 事实（2026-08-31）：Stride Amount／Walk Run Amount 均为 [0,1]，Cubic filter 为 X `.2s`／Y `.4s`；样本为 Walk_Left_Forward(1,0)、Walk_Left_Pose(0,0)、Run_Left_Forward(1,1)、Run_Left_Pose(0,1)，rate scale=1。source player 为 loop=true、rate=1、start=.2、Movement SyncGroup，asset loop=true、marker sync=true。
  - 实际图与资源静态证据：Left Forward node 的四条本地资源、坐标、loop、gltf animation／时长／skeleton、图内 duration／markers、selector=5、Movement phase、start phase、双轴 Cubic filter、root delta writer 与路由均无差异。`python tools/import/verify_runtime_blendspace.py --source-root <ALS Content/ALS> --asset BS_Als_WalkRun_Left_Forward` 输出 pass=true。按本批用户指令，未编译、未运行。
- [x] BF108.07 `BS_Als_WalkRun_Right_Backward`：核对并修复实际 RightBackward node 的四个项目 ClipNode、坐标、Cubic axis 语义、loop／Movement sync／start phase 与路由；验收为静态源对照零差异。
  - 当次 ALS 事实（2026-08-31）：Stride Amount／Walk Run Amount 均为 [0,1]，Cubic filter 为 X `.2s`／Y `.4s`；样本为 Walk_Right_Backward(1,0)、Walk_Left_Pose(0,0)、Run_Right_Backward(1,1)、Run_Left_Pose(0,1)，rate scale=1。source player 为 loop=true、rate=1、start=.2、Movement SyncGroup，asset loop=true、marker sync=true。
  - 实际图与资源静态证据：Right Backward node 的四条本地资源、坐标、loop、gltf animation／时长／skeleton、图内 duration／markers、selector=2、Movement phase、start phase、双轴 Cubic filter、root delta writer 与路由均无差异。`python tools/import/verify_runtime_blendspace.py --source-root <ALS Content/ALS> --asset BS_Als_WalkRun_Right_Backward` 输出 pass=true。按本批用户指令，未编译、未运行。
- [x] BF108.08 `BS_Als_WalkRun_Right_Forward`：核对并修复实际 RightForward node 的四个项目 ClipNode、坐标、Cubic axis 语义、loop／Movement sync／start phase 与路由；验收为静态源对照零差异。
  - 当次 ALS 事实（2026-08-31）：Stride Amount／Walk Run Amount 均为 [0,1]，Cubic filter 为 X `.2s`／Y `.4s`；样本为 Walk_Right_Forward(1,0)、Walk_Right_Pose(0,0)、Run_Right_Forward(1,1)、Run_Right_Pose(0,1)，rate scale=1。source player 为 loop=true、rate=1、start=.2、Movement SyncGroup，asset loop=true、marker sync=true。
  - 实际图与资源静态证据：Right Forward node 的四条本地资源、坐标、loop、gltf animation／时长／skeleton、图内 duration／markers、selector=1、Movement phase、start phase、双轴 Cubic filter、root delta writer 与路由均无差异。`python tools/import/verify_runtime_blendspace.py --source-root <ALS Content/ALS> --asset BS_Als_WalkRun_Right_Forward` 输出 pass=true。按本批用户指令，未编译、未运行。
- [x] BF108.09 `BS_Als_Look`：核对并修复 Head 实际图的 Pitch axis、3 个项目 ClipNode、坐标、evaluator／播放与 Head 输入 writer；验收为静态源对照零差异。
  - 当次 ALS 事实（2026-08-31）：Pitch Angle 为 [-90,90]、grid=4；样本为 Look_Forward(0)、Look_Down(-90)、Look_Up(90)，rate scale=1，轴 smoothing time=0。asset loop=false；source Head evaluator 为 loop=true、rate=1、start=0、DoNotSync。
  - 实际图与资源静态证据：Head 图三条本地资源的 ClipNode、gltf animation／时长／skeleton、Pitch 点、显式不回绕 evaluator、pitch/yaw 输入、Head writer 与根图路由均无差异。`python tools/import/verify_runtime_blendspace.py --source-root <ALS Content/ALS> --asset BS_Als_Look` 输出 pass=true。按本批用户指令，未编译、未运行。
- [x] BF108.10 Gate：逐项比较 9 个源资产与实际运行图／writer，确认项目动画引用、样本、轴、插值、播放／同步语义和参数映射均无差异；只运行静态图／源码校验，不编译、不运行。
  - Gate 结果（2026-08-31）：通过。实际运行图集为 Air Lean、Standing/Crouching Lean、`state_detail_cycle` 中六个 WalkRun node 与 Head Look；所有 source BlendSpace 都重新由本地 Unreal MCP 读取，source AnimBP player/evaluator 由本地 uasset 读取。校验器逐样本确认实际 ClipNode 的项目本地 `.anim.ron`、glTF animation、时长、skeleton、rate/mirror/single-frame flags、additive type 与 reference pose 对应的实际 difference node；WalkRun 还确认图内 duration／markers、selector、Movement sync、`.2` start phase；Air／Lean／Look 确认 evaluator/loop、路由和 writer。新增的通用 `CubicVec2AxisFilterNode` 以 X `.2s`／Y `.4s` 对齐六向 WalkRun 的源 Cubic filter，并由静态检查确认其导出和注册。
  - 静态证据：`python tools/import/verify_runtime_blendspace.py --source-root <ALS Content/ALS>` 输出 9/9 pass；`python tools/validate_animgraph_editor_assets.py --root assets/graphs --fix` 与无 `--fix` 均为 160 files、0 repairs、0 failures。按本批用户指令，未编译、未运行。

## 附录 A：与 ALS 的刻意差异（非缺陷）

| ALS | bevy_als | 理由 |
|---|---|---|
| AnimBP 状态机 | 显式 system 路由 + animation graph RON | ECS 数据流，无隐藏图执行 |
| GameplayTags | 普通 Rust 枚举组件 | 编译期检查 |
| ControlRig | 分析式 IK systems | 无运行时 rig 编译 |
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
- 音频（Metasound）、Niagara 特效、ALSCamera 动画实例仅保留挂钩点。
