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
─► BF108 BlendSpace 运行图等价（已归档）
─► BF109 Character Base 动画图全量等价（已归档） ─► BF110 结构化动画上下文图资产（已归档）
─► BF111 Character Base 图作者性规范化（移至 TODO.md）
```

> BF111 Character Base 图作者性规范化的完整任务、Gate 与状态已移至 [TODO.md](TODO.md)。

## BF112 ALS-ref AnimBP converter completion（当前批次）

目标：将 `AB_Als_Standing`、`AB_Als_Crouching`、`AB_Als_Grounded`、`AB_Als_Locomotion`、`AB_Als_Layering`、`AB_Als_Head`、`AB_Als_Ragdolling`、`AB_Als_Default` 从原始 `.uasset` 完整转换为可接入的 RON 闭包。每个节点类别按 source-first 合同实现；未知节点/功能保留 blocker，修复后才生成完整产物。每个蓝图必须通过 direct `--write`/`--check`、手工图交叉对比、source/runtime Gate 与 integration report。

- [x] BF112.01 pose reroute contraction：开始：2026-09-04 20:22:50 +08:00；预估结束：2026-09-04 20:42:50 +08:00；实际结束：2026-09-04 20:39:50 +08:00（偏差 -3m00s）。将 `K2Node_Knot` 作为 editor-only relay 自动折叠为直接边，不生成目标节点或运行时 ABI。验收：Standing/Crouching 各两条 reroute 不再成为 blocker，直接 source→target edge 保留，renderer fixture 与 fresh direct report PASS。
  - source fact：两图顶层的 relay 均连接 PoseLink；Standing 为 `StateMachine_2.Pose → Knot → Knot → ApplyAdditive.Base`，Crouching 为 `TwoWayBlend_0.Pose → Knot → Knot → ApplyAdditive_0.Base`。
  - Gate summary：renderer contraction fixture、converter/corpus self-test、Crouching corpus write/check和两份 fresh direct `--write`/`--check`均PASS；Knot blocker Standing/Crouching各2→0，未注册节点分别9→7、6→4。消融：不创建passthrough runtime node，只在目标边生成前递归归约source relay。

- [x] BF112.02 transition NOT/Absolute/GameplayTag scalar dispatch：开始：2026-09-04 20:40:48 +08:00；预估结束：2026-09-04 21:40:48 +08:00；实际结束：2026-09-04 21:39:11 +08:00（偏差 -1m37s）。注册 `NOT Boolean`、`Absolute (Float)` 与 GameplayTag 谓词；目标使用 `bevy_gameplay_tag` 的 0.3.0（Bevy 0.19）实现，而非枚举或 `f32` 索引。验收：Standing/Crouching/Grounded 对应函数 blocker 清零、transition source contract、`als-ext` Tag adapter fixture、corpus direct report PASS。
  - source fact：所有可达 `NOT Boolean` 均为 `A → ReturnValue` bool；所有 `Absolute (Float)` 均为 `A → ReturnValue` float。`Is Gameplay Tag Valid`接受`GameplayTag → ReturnValue`；Standing 的相邻比较是 `GetParent.Gait == Als.Gait.Walking`。`bevy_animation_graph` 的 `DataValue` 无 GameplayTag 变体，因此 `als_ext::SourceGameplayTagInputNode` 通过 player entity 读取真实 `bevy_gameplay_tag::GameplayTag`，在节点内完成 `is_valid`／`matches_tag_exact` 后才输出 Bool；报告保留 source property/tag text 与写入 API。
  - Gate summary：fresh Crouching/Standing/Grounded direct `--write` 及 Grounded `--check` 均无 drift；Standing 的 8 条 `GetParent.Gait`、Grounded 的 `GetParent.Stance` 和 `GetParent.GroundedEntryMode` 均进入 `gameplay_tag_providers`，Tag/NOT/Absolute blocker 均为 0。已通过 transition generator、converter、`als-ext` real-tag node 与 runtime plugin table 的 focused Gate。当前阻断边界仅为其余未实现 source 节点/生命周期；blocked Markdown 已收缩，详细 Tag 接入合同仅在 JSON。消融：删除未有 source 证据的 Tag-to-Tag 比较分支，遇到该形态立即 fail-closed。

- [x] BF112.03 GameplayTagsBlend real-tag conversion：开始：2026-09-04 21:42:55 +08:00；预估结束：2026-09-04 22:32:55 +08:00；实际结束：2026-09-04 22:16:35 +08:00（偏差 -16m20s）。将 `/Script/ALSEditor.AlsAnimGraphNode_GameplayTagsBlend` 注册为可转换节点；以真实 `bevy_gameplay_tag::GameplayTag` 读取 `ActiveTag`，替换仅处理 sprint 的 `f32` 适配路径。验收：source C++ child-index 合同、generic node fixture、Standing fresh direct report 与 corpus diff PASS。
  - source fact：`FAlsAnimNode_GameplayTagsBlend::GetActiveChildIndex()` 在 `ActiveTag.IsValid()` 时返回 `Tags.Find(ActiveTag) + 1`，否则返回 0；因此无效或不在 `Tags` 内的有效 Tag 都回退默认 pose，且为 exact match。`FAnimNode_BlendListBase::Update_AnyThread()` 先执行 exposed inputs，才读取 `GetBlendTimes()`；Standing 的序列化 `BlendTime=(0.1,0.1)`保留作 source record，但生效的可见 pin 默认是 `(0.3,0.2)`。当前 `SourceGameplayTagBlendNode` 的 sprint 专例已替换为任意 tag/pose 数组实现。
  - Gate summary：contract/renderer/converter self-test、`als-ext` real-tag selection test、Standing contract runtime Gate 均PASS。fresh Standing direct `--write`/`--check`无 drift，`AlsAnimGraphNode_GameplayTagsBlend` blocker 1→0，报告要求在 `AnimationGraphPlayer` 实体以 `als_ext::set_gameplay_tag_input("gait", GameplayTag)`提供真实 tag。消融：未保留 sprint-only state 或 f32 fallback；多 Tag 的 exact-index/权重路径复用通用 child blend 数据。

- [x] BF112.04 overlay GameplayTagBlend real-tag ABI：开始：2026-09-04 22:17:40 +08:00；预估结束：2026-09-04 23:07:40 +08:00；实际结束：2026-09-04 22:40:23 +08:00（偏差 -27m17s）。将现有 `GameplayTagBlendNode` 的 `active_index: f32` 输入改为真实 Tag key，迁移 barrel/binoculars/hands-tied/injured overlay 图及 root writer 的 locomotion-action/rotation-mode provider。验收：source tag constants、focused node selection test、受影响 RON 资产加载 Gate PASS。
  - source fact：`AlsGameplayTags.cpp`定义 `Als.LocomotionAction.{Mantling,GettingUp,Rolling}` 和 `Als.RotationMode.{VelocityDirection,ViewDirection,Aiming}`；当前 12 个 overlay `GameplayTagBlendNode`/`GameplayTagLayerBlendNode`均由 `als_overlay_*_index` 的 f32 edge 驱动。只有这些 Tag node 及其直接 input edge 属于迁移范围，其他 f32 state/curve 输入保持原样。
  - Gate summary：generic tag selection、root source-tag mapping、tag table parse 和 Box/Barrel/Binoculars runtime asset Gate均PASS；12 个 overlay node均改用`locomotion_action`或`rotation_mode`真实 Tag key，overlay图的`active_index` edge为0。消融：删除全部仅供这些节点使用的`GlobalF32InputNode`及edge；RagdollingTagBlend仍是独立 source node，保留为下一边界而不在本项复用/伪装。

- [x] BF112.05 RagdollingTagBlend real-tag ABI：开始：2026-09-04 22:41:22 +08:00；预估结束：2026-09-04 23:21:22 +08:00；实际结束：2026-09-04 22:53:51 +08:00（偏差 -27m31s）。将 RagdollingTagBlend 的唯一 `active_index` 输入改为 `locomotion_action`真实 Tag，并以 `LocomotionState::Ragdolling` 写入 `Als.LocomotionAction.Ragdolling`。验收：focused ragdoll tag selection、Ragdolling contract runtime Gate、无 active-index edge PASS。
  - source fact：`AlsGameplayTags.cpp`定义 `Als.LocomotionAction.Ragdolling`；本地 `AB_Als_Ragdolling` 原始图中 inner/outer `GameplayTagsBlend`均以该 exact Tag 选择，outer/inner 的有效 blend time 分别为 `(0.4,0.5)`/`(0,0)`。
  - Gate summary：`source_tag_selects_the_ragdolling_child`、`SourceGameplayTagInputNode` real-identity、root state→Tag mapping、带本地 UE 原始导出的 Ragdoll contract runtime Gate均PASS；position/coverage audit为9 target node、零 missing/mismatch/unexplained。消融：删除 `als_ragdolling_action_index` 的图节点、edge、output ABI、writer、fixture输入和最后一个 f32 selector helper；Ragdoll selection只读取`GameplayTag`，无索引兼容路径。

- [x] BF112.06 linked AnimInstance lifecycle provider dispatch：开始：2026-09-04 23:02:45 +08:00；预估结束：2026-09-04 23:42:45 +08:00；实际结束：2026-09-04 23:18:37 +08:00（偏差 -24m08s）。将当前三个直接转换图中已由手工 ALS runtime 覆盖的无参 `UAlsLinkedAnimationInstance`调用按函数签名映射为 lifecycle passthrough + 明确 runtime provider；未知签名与 StateResult lifecycle 仍 fail-closed。验收：lifecycle fixture、converter self-test、Standing/Crouching/Grounded fresh direct report 的对应 `unregistered_source_function`清零，其他 blocker 不被吞掉。
  - source fact：本地 `AB_Als_Standing.uasset`的9类调用均为 self-context、无参数、`OnBecomeRelevant`或`OnUpdate`；`AlsLinkedAnimationInstance.cpp`只转发给 parent。`AlsAnimationInstance.cpp`定义 grounded/standing/crouching/dynamic/turn 的状态写入；当前 `GroundedLayerPlugin`、`StandingLayerPlugin`、`CrouchingLayerPlugin`、`TurnLayerPlugin`已消费相同状态和生命周期。
  - Gate summary：lifecycle fixture、converter self-test、JSON parse、Standing/Crouching/Grounded direct `--write`/`--check`均PASS（blocked exit=3且无 drift）；三图的 `unregistered_source_function`均为0，已映射函数在 integration JSON 指向实际 runtime plugin。保留 Standing 5、Crouching 1、Grounded 1 个 `source_lifecycle_unsupported`及其余 node/property/sync blocker。消融：临时移除`RefreshStandingMovement` provider使 dispatch 0→1缺失，故保留最小函数签名映射；未按 asset path 建白名单。

- [x] BF112.07 StateResult MemberName lifecycle lowering：开始：2026-09-04 23:25:21 +08:00；预估结束：2026-09-05 00:05:21 +08:00；实际结束：2026-09-04 23:43:24 +08:00（偏差 -21m57s）。以 StateResult binding 的真实 `MemberName`取代编译期占位 `FunctionName`，递归验证已知 stop/roll函数图并映射到现有 transition runtime；其它 StateResult binding 保持 blocker。验收：binding precedence fixture、function-graph fixture、Standing/Crouching/Grounded fresh direct report 的 StateResult lifecycle blocker清零且 actual function name可人工对照。
  - source fact：Standing 的 `OnStateEntry`/`StopLeft`/`StopRight`分别绑定到`PlayStopLeftTransitionAnimation`或`PlayStopRightTransitionAnimation`；Crouching Stop绑定`PlayStopTransitionAnimation`；Grounded Roll exit绑定`PlayRollToGroundedTransitionAnimation`。这些 function graph 均是 FunctionEntry→GetParent→transition call；Standing Left/Right固定 `.2/.2/1.5/.4`，Crouching/Roll为 right transition `.2/.2/1.5/.2`。
  - Gate summary：state binding precedence/ablation、transition-function、pose-contract、converter self-test和三图 fresh direct `--write`/`--check`均PASS（blocked exit=3且无 drift）；integration JSON明确列出实际函数名，Standing/Crouching/Grounded 的`source_lifecycle_unsupported`均为0。消融：去掉 binding 时 fixture立即退回`CompiledPlaceholder`外部依赖；未按占位名映射或吞掉未知 binding。

- [x] BF112.08 Grounded States source label provider：开始：2026-09-04 23:47:45 +08:00；预估结束：2026-09-05 00:17:45 +08:00；实际结束：2026-09-04 23:54:31 +08:00（偏差 -23m14s）。注册 source `Grounded States` FSM label key，并由现有 `GroundedLayerPlugin`以与 source structural order 一致的状态 index 写入；不为其它 FSM 预注册 key。验收：focused label-index/registry fixture、Grounded direct `--write`/`--check`到 ready、生成闭包静态可读。
  - source fact：本地 Grounded FSM structural order为 `Entry Conduit(0) → Standing(1) → Crouching(2) → Standing To Crouching(3) → Crouching To Standing(4) → Roll(5)`；现有`GroundedState::index()`和`grounded_state_index()`已有完全相同的映射，只缺 target key `grounded_states.state_index`写入和 converter registry 声明。
  - Gate summary：writer label test、registry/key materialization ablation、converter self-test、Grounded direct `--write`/`--check`均PASS，conversion/integration status均为`ready`；生成35个 RON/FSM/condition 文件，静态 editor validator为0 failure。当前手工 `grounded_contract_gate`的 static predicates全为false；该 Gate不读取新 key，故非本项回归信号，保留为下一 runtime-audit 边界而不计入本项验收。

- [x] BF112.09 eight-asset fresh direct inventory：开始：2026-09-04 23:55:44 +08:00；预估结束：2026-09-05 00:40:44 +08:00；实际结束：2026-09-05 00:06:05 +08:00（偏差 -34m39s）。逐一从本地 8 个目标 `.uasset`生成 fresh direct report，记录 ready/blocker 类别与共同根因；不把已有 target/probe 结果当作当前 source 证据。验收：八个 output root 均有确定性 direct integration report，清单只据新报告建立后续实现项。
  - source fact：8 个目标 asset 均在本地 ALS Content 中可读；Standing/Crouching/Grounded已完成本轮 source re-read，其中 Grounded为`ready`，前两者仍为显式 blocker。
  - Gate summary：8/8 direct `--write`均完成；Grounded（40 files）和 Locomotion（55 files）为`ready`，其余六图均为受控`blocked`报告。Locomotion 的 pose-graph GameplayTag scalar path起初因缺 graph contract traceback；修复后 direct `--write`/`--check`为 ready，Head 同类 Windows temporary cleanup race也重试为受控 blocked。共同 blocker 为 Standing/Crouching属性、sync/AnimGetter、Layering renderer/property、Head renderer/function/asset、Ragdolling节点与 Default linked layer；未按 asset path 放行或吞掉未知 consumer。

- [x] BF112.10 PoseSnapshot source-node lowering：开始：2026-09-05 00:09:04 +08:00；预估结束：2026-09-05 00:54:04 +08:00；实际结束：2026-09-05 00:19:18 +08:00（偏差 -34m46s）。以已存在的 global pose input 承载 source `AnimGraphNode_PoseSnapshot`的`SnapshotPin`模式，严格保持`RagdollingState.FinalRagdollPose → ragdolling_state.final_ragdoll_pose`命名；删除手工图的旧 snapshot alias。验收：pose snapshot contract/renderer fixture、Ragdolling source tag+snapshot runtime Gate、direct `--write`/`--check`到 ready、无旧 alias。
  - source fact：本地 Ragdolling `PoseSnapshot`唯一可见输入为`Snapshot`，`Mode=SnapshotPin`，property binding为`GetParent.RagdollingState.FinalRagdollPose`，输出连接 inner GameplayTagsBlend 的`BlendPose_0`；原手工 `RagdollingLayerPlugin`在退出时捕获同一 snapshot pose。
  - Gate summary：PoseSnapshot contract/renderer、global-pose output、converter self-test、Ragdolling direct `--write`/`--check`和静态 RON validator均PASS，conversion/integration status为`ready`；带本地 UE source的 Ragdoll enter/stable/recovery contract Gate PASS。消融：移除注册映射立即回到`unregistered_source_node`；旧`als_ragdolling_snapshot_pose`/`als_ragdolling_flail_play_rate`为0引用，手工 graph/runtime使用`ragdolling_state.*` source-style key。

- [x] BF112.11 LocomotionMode/Stance real-tag providers：开始：2026-09-05 00:22:23 +08:00；预估结束：2026-09-05 00:52:23 +08:00；实际结束：2026-09-05 00:25:01 +08:00（偏差 -27m22s）。将 Locomotion direct graph 已报告的`locomotion_mode`/`stance`真实 Tag 写入 root player entity，并补全 plugin tag table；没有 source `LocomotionMode`等价的 target Ragdolling 保持 invalid Tag。验收：source tag mapping fixture、tag table parse、Locomotion direct runtime provider smoke、无 f32/index alias。
  - source fact：`AlsGameplayTags.cpp`只定义`Als.LocomotionMode.{Grounded,InAir}`和`Als.Stance.{Standing,Crouching}`；`AlsAnimationInstance.cpp`每帧从 Character 复制 LocomotionMode/Stance。ALS Ragdolling是`LocomotionAction`，不是第三个 LocomotionMode Tag，因此 target `LocomotionState::Ragdolling`不能伪造模式 Tag。
  - Gate summary：source-tag mapping、tag table parse和 root runtime input smoke均PASS；root在同一 AnimationGraphPlayer entity 写入`locomotion_mode`/`stance`，Ragdolling mode明确无效。消融：未增加 f32/index Tag alias；移除 Ragdolling invalid branch将伪造不存在的 source Tag，故保留该唯一空 Tag 映射。

- [x] BF112.12 Head lifecycle provider dispatch：开始：2026-09-05 00:27:06 +08:00；预估结束：2026-09-05 00:42:06 +08:00；实际结束：2026-09-05 00:29:34 +08:00（偏差 -12m32s）。将无参`InitializeHead`/`RefreshHead`按签名映射到已存在`HeadLayerPlugin`，不触及独立的 BlendSpace evaluator blocker。验收：lifecycle fixture、Head fresh direct report无对应 unregistered function，剩余 evaluator/asset blocker不被吞掉。
  - source fact：`InitializeHead()`只重置`HeadState.bInitializationRequired`；`RefreshHead()`更新 source head yaw/pitch/switch-side state，均经`AlsLinkedAnimationInstance`转发。现有`HeadLayerPlugin`拥有相同的`AlsHeadState`初始化与每帧更新通道。
  - Gate summary：lifecycle/converter/pose renderer self-test均PASS，Head fresh direct report中两个 function均为`mapped`；BlendSpace evaluator仍报告`unresolved_source_animation_asset`、`source_pose_renderer_unsupported`和`source_renderer_unimplemented`。消融：缺少 evaluator semantic 时现在返回`PoseRenderError`→JSON blocker，不再抛出 Python `AttributeError`。

- [x] BF112.13 1D dynamic BlendSpace evaluator：开始：2026-09-05 00:33:46 +08:00；预估结束：2026-09-05 01:23:46 +08:00；实际结束：2026-09-05 01:00:27 +08:00（偏差 -23m19s）。支持一维 `X + NormalizedTime` evaluator schema，将动态 normalized time 写入 existing `NormalizedBlendSpaceNode` phase；保留二维 evaluator与其它 schema的 fail-closed 行为。验收：1D fixture/ablation、Head direct `--write`/`--check`到 ready、手工 Head contract runtime Gate。
  - source fact：`BS_Als_Look`是`BlendSpace1D`，唯一有效 axis为`Pitch Angle[-90,90]`，三个 sample为 Down/Forward/Up；Head evaluator pin为`X(Pitch Angle)`和`NormalizedTime(Normalized Time)`，分别绑定`HeadState.PitchAngle`与`HeadState.YawAmount`，且`bTeleportToNormalizedTime=true`。
  - Gate summary：resolver/renderer/converter self-test、Head direct `--write`/`--check`、生成 RON 静态验证、Head contract 与 yaw runtime Gate均PASS；Head report为`ready`且零 blocker。消融：删除无消费者的`normalized_time_is_input`/`normalized_time_key`和缺失 pin schema 的二维默认值后结果不变；额外 evaluator input与缺失 schema均由 fixture fail-closed 拒绝。

- [x] BF112.14 Linked Anim Layer self-implementation closure：开始：2026-09-05 01:03:25 +08:00；预估结束：2026-09-05 01:53:25 +08:00；实际结束：2026-09-05 01:37:29 +08:00（偏差 -15m56s）。仅支持当前 AnimBP 以单一 `ImplementedInterfaces` layer 实现自身 `AnimGraphNode_LinkedAnimLayer`的 source 语义，递归物化该 layer graph，并复用既有`als_ext::LinkedAnimLayerNode`；多实现、动态实例或未声明实现继续 fail-closed。验收：fresh Default/interface source record、fixture/ablation、Default direct `--write`/`--check`到 ready、生成 RON 静态 Gate。
  - source fact：fresh reader确认 Default root只有`ALI_Overlay - Overlay` Linked Anim Layer，`Node`为`Interface=ALI_Overlay`、`Layer=Overlay`，无 lifecycle functions或输入 pin；同一 AnimBP 的`ImplementedInterfaces`声明该`Overlay` graph，interface asset也只声明此 layer root。该子图唯一 sync player是`A_Als_Idle`的`Secondary Motion`，loop、零 start、2.666667 秒且无 markers。
  - Gate summary：contract/reachability/renderer/materializer/converter self-test、复用`LinkedAnimLayerNode`的 focused Rust test、Default direct `--write`/`--check`、2 graph static editor/position Gate均PASS；Default report为`ready`、零 blocker，明确列出`secondary_motion.sync_phase`和五个 target runtime input。同期重写/确定性检查并静态审计 Grounded、Locomotion、Ragdolling、Head；补齐 synthetic state editor position 后四项均PASS。消融：移除自身 interface 声明会被 fixture拒绝；缺少物化 layer target也会被 materializer拒绝；未新增 runtime node。

- [x] BF112.15 Make Dynamic Additive：开始：2026-09-05 01:40:22 +08:00；预估结束：2026-09-05 02:10:22 +08:00；实际结束：2026-09-05 01:51:22 +08:00（偏差 -19m00s）。按 source `bMeshSpaceAdditive`选择既有 mesh/local difference 算法，保留`Base`/`Additive`连接及时间语义；新增最小 Pose-only`SourceMakeDynamicAdditiveNode`以匹配 source graph ABI。验收：source contract/renderer fixture和 mesh/local 消融、Layering direct blocker减少且无 partial RON。
  - source fact：fresh Layering source有两个`Make Dynamic Additive`，同为`Base`/`Additive`→`Pose`且无 lifecycle/binding；一个明确`bMeshSpaceAdditive=True`，另一个省略该字段即 local。现有`als-ext`已有对应 mesh/local difference node。
  - Gate summary：contract/renderer/converter self-test、focused Rust mesh/local branch + NodeLike registration test均PASS；Layering fresh `--write`和 blocked `--check`输出稳定，两个 Dynamic Additive不再是 blocker，且目录仅保留三行 blocked Markdown/JSON报告。消融：同一骨骼父子旋转 fixture证明 mesh/local差分不同；不以带 Time pin 的既有 node硬接 source Pose-only ABI。

- [x] BF112.16 Curves Blend：开始：2026-09-05 05:18:08 +08:00；预估结束：2026-09-05 06:08:08 +08:00；实际结束：2026-09-05 05:26:32 +08:00（偏差 -41m36s）。以 source pose context 的动态曲线键集合实现`Accumulate`/`Override`，保留 BasePose为输出 pose 和 CurvePose的 source 曲线语义；其它 source mode继续 fail-closed，不硬编码 ALS 曲线名单。验收：C++/source contract、context merge fixture/ablation、Layering direct blocker减少且无 partial RON。
  - source fact：`FAlsAnimNode_CurvesBlend::Evaluate_AnyThread`先 Evaluate BasePose，再以 BlendAmount Evaluate CurvePose；`Accumulate`调用`Curve.Accumulate(curves, alpha)`，`Override`调用`Curve.Override(curves, alpha)`。当前 Layering仅有两个节点，分别为`BlendAmount=1`的`Accumulate`与`Override`，无 property binding。
  - Gate summary：source contract/renderer/converter self-test、focused context merge + NodeLike registration test均PASS；Layering fresh `--write`/blocked `--check`稳定且不再包含 CurvesBlend/MakeDynamicAdditive，目录只含三行 blocker Markdown和JSON。消融：`Lerp` contract fixture仍拒绝；动态 context key merge替代曲线白名单；缺 semantic renderer现在受控为`PoseRenderError`而非 traceback。

- [x] BF112.17 Layered Bone Blend branch filters：开始：2026-09-05 05:29:41 +08:00；预估结束：2026-09-05 06:24:41 +08:00；实际结束：2026-09-05 05:51:50 +08:00（偏差 -32m51s）。保留每 layer 的 source `BoneName + BlendDepth`及`BlendWeights_n` source pin/property binding，复用已有 branch-filter mask与 SourceLayeredBoneBlendNode；旧手工 RON默认 ABI不变。验收：depth/weight fixture与消融、Layering direct `--write`/`--check`到 ready或剩余首个 source blocker、静态 RON Gate。
  - source fact：Layering的九个 standard LayeredBoneBlend使用`BlendDepth`为`-1/0/1/3`，并有`LayeringState.{ArmLeftMeshSpaceBlendAmount,ArmRightMeshSpaceBlendAmount,HandLeftBlendAmount,HandRightBlendAmount,ArmLeftLocalSpaceBlendAmount,ArmRightLocalSpaceBlendAmount}`绑定到`BlendWeights_n`。现有`build_branch_filter_bone_mask`已有正/负深度 source rule test，但`SourceLayeredBoneBlendNode`尚未携带depth或权重 pin。
  - Gate summary：branch-filter、SourceLayeredBoneBlend、contract/renderer/materializer/converter self-test均PASS；Layering fresh direct `--write`/`--check`为ready，editor/position Gate和手工 Layering runtime contract均PASS，六个ready corpus的确定性/静态回归均PASS。消融：正/负depth rule与weight helper fixture保持 source差异；未达根输出的 reroute edge不会被物化，旧手工 RON保持`expose_blend_weight_pins=false`默认ABI。


## 附录 A：与 ALS 的刻意差异（非缺陷）

| ALS | bevy_als | 理由 |
|---|---|---|
| AnimBP 状态机 | 显式 system 路由 + animation graph RON | ECS 数据流，无隐藏图执行 |
| GameplayTags | `bevy_gameplay_tag` 的真实层级 Tag | 保留 UE Tag identity 与 exact/hierarchical query 语义 |
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
