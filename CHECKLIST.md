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
─► BF111 Character Base 图作者性规范化（当前执行批次）
```

## BF111 Character Base 图作者性规范化（当前执行批次）

将 BF110 的动画图作者性方法固化为项目内 skill 和通用离线验收工具，并对已归档 BF109 的 .01–.11 项重新按该规范取证、修订和验收。每个图类项必须以实际 `.animgraph.ron`、完整 source fact、source-matched position、可见 pose/control 边界和真实入口执行链为依据；仅当同一 pose 分支的曲线／时间 relay 经运行时合同证明可安全内聚时才使用 animation context，不得为了减少连线删除独立 time、event、控制或物理 ABI。`PA_Als`、`CR_Als` 不伪造为图资产，但必须登记其 source/runtime 边界、坐标／生命周期证据和真实入口 Gate。不得修改 `docs/als-ref-animation-asset-map.md`、`bevy_animation_graph`、`bevy_animation_graph_editor` 或 Cargo dependency revision；本轮保留 `ragdoll_config → Outputs`，其 Avian root-output ABI 不在范围内。空 `Inputs` UI 壳不是 asset input，不得以虚假 input 或依赖改动掩盖它。

**BF111 通用视觉运行时 Gate（适用于 .03–.14）**：每项影响可见角色姿态、additive、BlendSpace、状态机、IK、Control Rig、ragdoll、root motion 或动画资产选择的工作，必须在真实 `manual_playground` 场景运行 `tools/run_animation_visual_validation.ps1`，并用 `tools/validate_animation_visual_evidence.py --check` 验收。证据目录必须含自动场景关键帧 PNG、最终 post-propagation key-bone transform／scale／肢段连续性、graph error、wall-frame 与动画求值 p50/p95/max 和机器可读 report。不得把进程能运行、`graph_err=none`、单一静态截图或人工临时目测作为通过；塌缩、扭曲、additive 异常、非有限 transform、关节跳变、空／无效截图或性能超阈值必须失败并保留故障帧。发现异常后，必须先重新提取当次本地 ALS-Refactored 的对应资产／可见图／Control Rig／C++ 算法并逐项比较；视觉报告只定位症状，不能授权独立优化、补偿、平滑、阈值放宽或算法重写，只有 source 已证实差异才可改 target。baseline 只能来自独立已验证 run，不得在当前改动中自我接受。各条目的 Gate 记录须列出 scenario、动作覆盖、evidence 目录与 verdict。

**BF111 共用前置（.05–.14）**：本文件只列模块特有 scope 与 Gate；source fact、递归 source-visible 合同、manifest、editor/layout 验证、source-first 修复边界和首错报告一律由 `agents.md` 与 animation-graph-equivalence skill 强制执行。

- [x] BF111.01 项目内动画图对标 skill 与通用作者性验收工具：创建项目内 skill，提供 source fact、可见链路／position、context 边界、root ABI 例外、完整 RON／FSM 元数据与 runtime Gate 的统一工作流；新增声明式、可复跑的通用审计工具和自测。验收：skill 结构校验、工具自测和现有 `AB_Als` manifest smoke 全部 PASS。
  - 已实现（2026-09-01）：项目内 skill 为 `skills/animation-graph-equivalence/SKILL.md`；`tools/validate_animgraph_authoring.py` 以声明式 manifest 精确校验 `.animgraph.ron` 的 node 顺序、position、edge、root IO、source-evidence SHA-256、非 Pose root-output 例外和 runtime-context residual，并同样覆盖 `.fsm.ron` 的 state、transition、position 与 IO。验证命令只能运行项目内 Python 只读脚本，拒绝 `--fix`／`--write`。
  - AB_Als smoke（2026-09-01）：`tools/animgraph_authoring/ab_als.authoring.json` 登记实际 12 node／13 edge、无 root input/time、Pose 与唯一 `ragdoll_config` output，以及此 ABI 的 Avian evidence；通用工具同时运行 root source audit、node-position 与只读 editor metadata check，结果为 zero failure。`ragdoll_config → Outputs` 未改动。
  - Gate 证据（2026-09-01）：`validate_animgraph_authoring.py --self-test` 覆盖完整 graph/FSM 合同、未登记 output、position mismatch 和 mutating command 拒绝，全部 PASS；skill quick validator、runtime-context tool self-test、editor validator fix/read-only（161 files、0 repair/0 failure）和实际 AB_Als manifest check 均 PASS。
- [x] BF111.02 规则接入：更新 `agents.md`，强制今后任何 `.animgraph.ron`／`.fsm.ron` 创建或修改使用项目内 skill、声明式作者性审计和已有 editor asset validator；明确 source fact、position、ABI exception、runtime/real-entry evidence 的最低要求。验收：规则链接、工具命令和例外登记路径均可静态核查。
  - 已实现（2026-09-01）：`agents.md` 规则 22 强制在 graph/FSM 改动前完整读取 `skills/animation-graph-equivalence/SKILL.md`，在 `tools/animgraph_authoring/` 更新覆盖全部变更图的 manifest，并依次运行 authoring gate、规则 18 的 fix/read-only editor validator、position verifier、source audit、runtime Gate 和真实 Playground case。规则明确每个非 Pose root output 的 ABI exception、curve/time/event/control/physics 边界、空 `Inputs` editor decoration 及 tool/skill 自身变更的自测要求。
  - Gate 证据（2026-09-01）：静态检索确认 skill、manifest、gate、`ragdoll_config` 和 `run_manual.bat` 路径均在规则 22 中；skill quick validator、authoring gate self-test 与 AB_Als manifest smoke 全部 PASS。
- 已归档完成条目见 [归档索引](docs/archive/archivedchecklist.md)（2026-09-03：BF111.03、BF111.04）。
- [ ] BF111.06 / BF109.04 `AB_Als_Locomotion`：检查 Grounded/Air/Jump、Air Lean、Jump/Fall/Land 与状态机，并修订实际图／合同／位置与 context 边界。验收：Locomotion 静态链路与真实入口路径 PASS。
  - [x] BF111.06-S1 fresh source closure：开始：2026-09-04 09:44:26 +08:00；预估结束：2026-09-04 10:04:26 +08:00；实际结束：2026-09-04 09:55:55 +08:00。fresh native SHA-256 为 `a8071e6214719a34c088737e038ac549ae4c316fea07024bd8ce2fcec3dd09e3`；live MCP 读取 32 个 graph ref、顶层 DSL 为空但已记录为能力边界；native authored closure 为 30 entries（10 graph、2 FSM、18 transition），并生成 30 份 source-visible contract。顶层为 9 node/4 link，外层 FSM 为 23 node/23 link；后续 map/manifest 只落在 `tools/animgraph_authoring/`，运行时图仍限 `assets/graphs/`。
  - [ ] BF111.06-SS source-to-Bevy AnimGraph converter：以 fresh source contract 为唯一输入，生成 source-visible RON/FSM、runtime child、source-visible map/manifest 和 unsupported report；未知 source 节点、函数、pin、property、transition 或生命周期必须由生成器以机器可读错误拒绝，随后由代理补齐 source mapping、fixture 与 self-test，不得要求用户手工映射。依据为 Locomotion 的 30 entry closure、既有 authored graph extractor/map/audit 工具以及 BF111.03／.04 已归档 manifest。
    - [x] SS.1 converter core：开始：2026-09-04 10:43:22 +08:00；预估结束：2026-09-04 11:35:00 +08:00；实际结束：2026-09-04 11:10:10 +08:00。public input 必须为 `.uasset`，转换器自行经仓库 native UE source reader 读取包内容并在临时目录构造 JSON intermediate；不得要求调用者先导出 JSON，也不得把 intermediate 当作 source authority 或提交输入。转换核心不计算或依赖 SHA-256；它直接比较解析出的 source IR。source digest 仅由最终 authoring manifest/audit Gate 在转换完成后记录。实现通用 UE AnimBP source-contract IR、已注册 mapping registry、deterministic output、unsupported report 与自测；registry 只能按 source node class、pin schema、function/lifecycle signature、asset category 或可证明的通用语义分派，禁止按某个 AnimBP 名称、source asset path 或单图 node GUID 特化。未知 source node/function/pin/property/transition/lifecycle 必须只输出机器可读错误并阻断 IR/RON/FSM/materialized 输出。输出仅在 `tools/`／`target/` 生成，运行时资产须显式 materialize 后才可替换。验收：core self-test 能拒绝未注册 node/function 与 asset-name special case，且 source IR 的 node/pin/link/position/property 值会使 `--check` 失败。
      - Gate 证据：`python tools/animgraph_authoring/ue_animgraph_converter.py --self-test` 通过；以 `.uasset` direct input 的 Locomotion `--write` 与重新读取包内容的 `--check` 均 PASS，source IR 覆盖 30 entry closure 且无未知 node/function。首次 CRLF function title 被误报的 parser 缺陷已补规范化与 self-test；工具不再将 transient JSON hash 当成 conversion 输入或输出。
      - 复开与复验（2026-09-04 11:25:57 +08:00）：初版错误地把 native JSON 当作 public input，并将 SHA-256 bookkeeping 放进转换核心；用户指出这会偏离“直接读取 uasset 并转换”的目标。已改为仅接受 `.uasset` public input、由工具临时调用 native reader、直接比较 parsed IR；direct Locomotion `--write`／`--check` 重跑 PASS。此经验已固化到 animation-graph-equivalence skill 的 source input 与 generator-maintenance 规则。
    - [ ] SS.2 Locomotion iteration：开始：2026-09-04 11:12:21 +08:00；预估结束：2026-09-04 12:07:21 +08:00。以本次 fresh `AB_Als_Locomotion` 30 entry closure 迭代 registry，生成 root／outer FSM 与 runtime child；其余递归图也必须进入 closure ledger，未注册项逐项回写 generator 后重生，不得手工补图。验收：生成结果通过 root/FSM source-visible audit、authoring/editor/position Gate，且生成输出与 materialized runtime asset 一致。
    - [ ] SS.3 BF111.03／BF111.04 conversion regression：预估开始：2026-09-04 12:30:00 +08:00；预估结束：2026-09-04 13:15:00 +08:00。重新提取 `AB_Als_Standing` 与 `AB_Als_Crouching` 的 fresh source closure，运行同一转换器到隔离输出，比较生成 RON/FSM、map、manifest 与已通过资产；差异只能是经 source contract 证明且显式登记的 generator migration。验收：两个 source closure 均无未注册项，isolated conversion 的 static/editor/audit check PASS，不能覆盖已归档运行时资产或把旧 Gate 当作本次 PASS。
    - [ ] SS.4 BF111.05／BF111.06 golden corpus：预估开始：2026-09-04 13:15:00 +08:00；预估结束：2026-09-04 14:00:00 +08:00。重新提取 `AB_Als_Grounded` 与 `AB_Als_Locomotion` 的 fresh source closure，完成同一 isolated conversion；与 SS.3 合并形成 BF111.03–.06 四图 golden corpus。验收：四个 closure 均无未注册项、生成结果与 materialized assets 一致、各自 source/static/layout/editor Gate PASS，并重跑每图已声明的 focused runtime/real-entry Gate。
    - Gate：SS.1–SS.4 均通过；转换器不会在运行时读取 source 数据，不会写入 `assets/` 以外的未声明 runtime 路径，不会以 target 已有 RON 代替 fresh source contract；随后再恢复 S2。
  - 条件性 converter-first 采用：只有 SS golden corpus 的 BF111.03–.06 全部通过后，BF111.07 及之后所有 AnimGraph／FSM 条目必须先由转换器从 fresh source materialize 并验证；我只可迭代 converter registry、source fixture、项目侧扩展和 Gate，不得重新采用逐图手工对照抄写。每项仍必须完成 source-first、离线坐标/资产、生命周期、静态及真实入口 Gate；任一图出现未注册 source 项即回到 SS 式 generator maintenance，而不是要求用户介入或写一次性目标补丁。若 golden corpus 失败，则记录失败边界，转换器仍可辅助但不得取代后续 source/static 对标。
  - [ ] BF111.06-S2 root + outer FSM static parity：开始：2026-09-04 09:55:55 +08:00；预估结束：2026-09-04 10:40:00 +08:00；依据为 fresh source 的顶层 9 node/4 link 与 `Locomotion States` 23 node/23 link。只建立 root／outer FSM 的 source-visible map、target IO、node/state order、position、edge/transition endpoint 与 static property/value 合同；state 内部图、nested Jump FSM 与真实 runtime 留待后续独立条目。验收：这两个 source contract 的 audit/map/position gate PASS，且 manifest 边界不漏列其可达 state/transition 子图。
    - source 基线（2026-09-04）：root 的功能节点为 Grounded Input、Save cached pose Grounded、Locomotion States、Inertialization、InitializeLean、Output（另有 3 个按用户批准忽略的 comment）；outer FSM 为 6 state、2 alias、11 transition。现有 target root 的 telemetry/driver 包装不是 source-visible ABI；必须保留为实际 runtime child/context，而由新 authored root 隐藏。现有 `InterruptibleFsmNode.state_aliases` 已足以项目侧表达 alias，无需修改依赖。
    - 暂停说明（2026-09-04 10:43:22 +08:00）：用户要求先建立 SS 转换器；当前 root/FSM/runtime-child 改动仅作为 generator fixture 的待验证种子，不构成 S2 Gate 证据。SS Gate 通过后必须由生成器重新 materialize 并重跑 S2，不得以本次手工中间态宣称通过。
  - 待执行运行时触发证据（2026-09-02）：当前 `foot_ik` 真实 Playground 在 `FootIkTerrain` phase elapsed `7.39s`、`Grounded`／`Walking`／`Standing`／`ViewDirection` 的 Jump/Land 后首次记录骨架肢段长度塌缩；同帧左右 Foot IK effector/knee residual 约为零，故不得归因于未证明的 IK 补偿。执行本条时必须重新提取本地 `AB_Als_Locomotion`、Land state/clip/additive source fact，比较当前 pose/scale/translation 写入链，再以同一状态标签重跑视觉 Gate；此记录本身不授权在 BF111.03 阶段修改 Locomotion 图或资产。
  - 待执行 Transition Slot consumer 边界（2026-09-02）：本地 ALS `AB_Als_Standing` C++ 已证明 Quick Stop 通过 `Transition` Slot 播放 additive dynamic montage。当前目标 focused gate 已证明 sampled quick-stop delta 非零，`AB_Als` 的 `Slot 'Transition'` 输出也变化；但其后的最终 root pose 未变化，故丢失点位于 Slot 后的 Locomotion/root consumer。本条执行时必须重新提取本地 `AB_Als_Locomotion` 及其 Grounded Input/状态图 source fact，逐链比较 Slot 输入 pose、additive/curve context、Locomotion 输出与最终 root pose；不得以 BF111.03 的中间 Slot 成功宣称 end-to-end 等价，也不得在本条开始前修改该 consumer。
- [ ] BF111.07 / BF109.05 `AB_Als_Layering`：检查 Base Poses、Additive、Slot、mask、curve layer 与可见输入边界，并修订实际图／合同／位置与 context 边界。验收：Layering 静态链路与真实入口路径 PASS。
- [ ] BF111.08 / BF109.06 `AB_Als_Head`：检查 Look BlendSpace、head/neck 混合、曲线与输入边界，并修订实际图／合同／位置与 context 边界。验收：Head 静态链路与真实入口路径 PASS。
- [ ] BF111.09 / BF109.07 `PA_Als`：复核 body、constraint、collider、坐标转换和 ragdoll consumer boundary。验收：资产/transform 静态链路与 ragdoll 真实入口路径 PASS。
- [ ] BF111.10 / BF109.08 `AB_Als_Ragdolling`：检查 pose input、混合、退出、PA consumer boundary 与 position。验收：Ragdolling 静态链路与真实入口路径 PASS；保持 `ragdoll_config → Outputs` 不变。
- [ ] BF111.11 / BF109.09 `ALI_Overlay`：检查 interface pin、linked layer、fallback 与实例生命周期边界。验收：Overlay 静态链路与真实入口路径 PASS。
- [ ] BF111.12 / BF109.10 `CR_Als`：复核 graph/stage、骨骼／虚骨、Construction/runtime 分相、target-space 数据与 `AB_Als` 接入边界。验收：资产/transform/lifecycle 静态链路与 ControlRig 真实入口路径 PASS。
  - 待执行冲刺膝盖回归（2026-09-02）：按住 Shift 前向冲刺时，左右膝盖分别向各自外侧偏移。该症状在修复 `PoseMoving` 曲线 relay 后仍不得默认视为已解决；执行本项时必须重新提取本地 `CR_Als`、`RefreshFootOffset`、`RefreshFootIk`、`ApplyFootIk` 及引擎 Basic IK 源码，以同一前向冲刺输入比较双侧 foot offset rotation、pole location、primary/secondary axis、writeback 与膝盖相对腿平面数值。只有 source 已证实 target 差异才可修改；需新增可复跑的 source-first 双侧膝盖回归 probe，视觉截图仅作最终回归记录，不能替代链路定位。
- [ ] BF111.13 / BF109.11 `AB_Als_Default`：检查 linked layer、状态、基础/secondary motion、曲线、mask 与消费边界。验收：Default Overlay 静态链路与真实入口路径 PASS。
- [ ] BF111.14 Character Base 作者性总 Gate：逐项重跑 .01–.11 的 source/static/layout/editor/runtime/real-entry 证据，确认所有 graph 有声明式合同、所有例外均有依据、所有实际 RON/FSM 通过编辑器校验，且未引入依赖改动或 `ragdoll_config` ABI 改动。验收：全量通用审计、目标图校验、相关 Rust Gate 和真实 Playground 场景全部 PASS。

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
