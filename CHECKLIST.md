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

- [x] BF111.01 项目内动画图对标 skill 与通用作者性验收工具：创建项目内 skill，提供 source fact、可见链路／position、context 边界、root ABI 例外、完整 RON／FSM 元数据与 runtime Gate 的统一工作流；新增声明式、可复跑的通用审计工具和自测。验收：skill 结构校验、工具自测和现有 `AB_Als` manifest smoke 全部 PASS。
  - 已实现（2026-09-01）：项目内 skill 为 `skills/animation-graph-equivalence/SKILL.md`；`tools/validate_animgraph_authoring.py` 以声明式 manifest 精确校验 `.animgraph.ron` 的 node 顺序、position、edge、root IO、source-evidence SHA-256、非 Pose root-output 例外和 runtime-context residual，并同样覆盖 `.fsm.ron` 的 state、transition、position 与 IO。验证命令只能运行项目内 Python 只读脚本，拒绝 `--fix`／`--write`。
  - AB_Als smoke（2026-09-01）：`tools/animgraph_authoring/ab_als.authoring.json` 登记实际 12 node／13 edge、无 root input/time、Pose 与唯一 `ragdoll_config` output，以及此 ABI 的 Avian evidence；通用工具同时运行 root source audit、node-position 与只读 editor metadata check，结果为 zero failure。`ragdoll_config → Outputs` 未改动。
  - Gate 证据（2026-09-01）：`validate_animgraph_authoring.py --self-test` 覆盖完整 graph/FSM 合同、未登记 output、position mismatch 和 mutating command 拒绝，全部 PASS；skill quick validator、runtime-context tool self-test、editor validator fix/read-only（161 files、0 repair/0 failure）和实际 AB_Als manifest check 均 PASS。
- [x] BF111.02 规则接入：更新 `agents.md`，强制今后任何 `.animgraph.ron`／`.fsm.ron` 创建或修改使用项目内 skill、声明式作者性审计和已有 editor asset validator；明确 source fact、position、ABI exception、runtime/real-entry evidence 的最低要求。验收：规则链接、工具命令和例外登记路径均可静态核查。
  - 已实现（2026-09-01）：`agents.md` 规则 22 强制在 graph/FSM 改动前完整读取 `skills/animation-graph-equivalence/SKILL.md`，在 `tools/animgraph_authoring/` 更新覆盖全部变更图的 manifest，并依次运行 authoring gate、规则 18 的 fix/read-only editor validator、position verifier、source audit、runtime Gate 和真实 Playground case。规则明确每个非 Pose root output 的 ABI exception、curve/time/event/control/physics 边界、空 `Inputs` editor decoration 及 tool/skill 自身变更的自测要求。
  - Gate 证据（2026-09-01）：静态检索确认 skill、manifest、gate、`ragdoll_config` 和 `run_manual.bat` 路径均在规则 22 中；skill quick validator、authoring gate self-test 与 AB_Als manifest smoke 全部 PASS。
- [x] BF111.03 / BF109.01 `AB_Als_Standing`：按作者性规范重新提取本地 UE source fact，逐链路检查 Standing 根图、状态机、Movement/Stop、六向 BlendSpace 和 Lean，并修订实际图／合同／位置与 context 边界。验收：声明式审计、source/layout audit、editor validator 与 Standing 真实入口路径 PASS。
  - 当次 ALS 事实（2026-09-01）：本机 UE 原生读取器重新导出 `AB_Als_Standing` 到本轮临时 source evidence，完整 payload 为 258 graph、1,157 node、2,886 pin、2,506 asset object；其 canonical JSON SHA-256 为 `1b0a3c20ab5c5baf969a53e9dbb393364393d68def18cbe98eeaadac8757f507`。完整原始 node/pin/link/property/position payload 保留在可重导的本地 evidence，未以摘要替代事实或提交 UE uasset。
  - 作者性合同（2026-09-01）：`ab_als_standing.recipe.json` 以当前本地 source evidence 生成 `ab_als_standing.authoring.json`，完整登记 34 个实际 animgraph（416 node、734 edge）和 4 个 FSM（22 state、51 transition）的有序 node/state、position、edge/transition、IO 与每个非 Pose boundary output。合同 generator 的 `--check` 为 zero difference；source graph coverage 为 34 mapped graph、1,072 source node、0 missing/unexplained，source layout 为 416 positioned node、0 mismatch；新增 FSM source-position audit 对 4 个 FSM 为 0 difference。
  - 链路处理（2026-09-01）：实际 RON 与 fresh source mapping 已零差异，故未为了制造变更而改写已有 Pose、time、event、control 或 graph-boundary ABI。Standing 的曲线/telemetry output 被完整声明为 project-side graph boundary，而父 `AB_Als` 已按 BF110 context 规则将它们内聚，不向顶层可见图泄漏。修复 `standing_sync_gate`：它现在查询实际 `walk run cubic axis filter` 节点，且失败路径以 exit code 7 结束，避免 FAIL 日志被错误报告为成功。
  - Gate 证据（2026-09-01）：authoring gate、generator、FSM position audit 与 root manifest smoke 全部 PASS；editor validator fix/read-only（161 files、0 repair/0 failure）和 Standing position verifier PASS。`standing_contract_gate` 覆盖 29 Movement、14 Detail、12 outer transition 与 idle/六向/run/sprint/pivot/stop/rotate/turn/quick-stop/lean，PASS；修复后 `standing_sync_gate` 覆盖 walk/run、五个方向、sprint enter/reset/exit、marker/pose，PASS。真实 `run_manual.bat` Base 在可见原 Playground 中记录 `standing_graph`、direction/sprint 等 EXECUTED，最终 `scenario=Base real_entry_paths -> PASS`。
- [ ] BF111.04 / BF109.02 `AB_Als_Crouching`：按作者性规范重新提取 source fact，逐链路检查 Crouching 根图、Movement、六向播放链与 Lean，并修订实际图／合同／位置与 context 边界。验收：声明式审计、source/layout audit、editor validator 与 Crouching 真实入口路径 PASS。
- [ ] BF111.05 / BF109.03 `AB_Als_Grounded`：按作者性规范重新提取 source fact，逐链路检查 Standing/Crouching 路由、stance、roll、transition 与状态机，并修订实际图／合同／位置与 context 边界。验收：声明式审计、source/layout audit、editor validator 与 Grounded 真实入口路径 PASS。
- [ ] BF111.06 / BF109.04 `AB_Als_Locomotion`：按作者性规范重新提取 source fact，逐链路检查 Grounded/Air/Jump、Air Lean、Jump/Fall/Land 与状态机，并修订实际图／合同／位置与 context 边界。验收：声明式审计、source/layout audit、editor validator 与 Locomotion 真实入口路径 PASS。
- [ ] BF111.07 / BF109.05 `AB_Als_Layering`：按作者性规范重新提取 source fact，逐链路检查 Base Poses、Additive、Slot、mask、curve layer 与可见输入边界，并修订实际图／合同／位置与 context 边界。验收：声明式审计、source/layout audit、editor validator 与 Layering 真实入口路径 PASS。
- [ ] BF111.08 / BF109.06 `AB_Als_Head`：按作者性规范重新提取 source fact，逐链路检查 Look BlendSpace、head/neck 混合、曲线与输入边界，并修订实际图／合同／位置与 context 边界。验收：声明式审计、source/layout audit、editor validator 与 Head 真实入口路径 PASS。
- [ ] BF111.09 / BF109.07 `PA_Als`：按作者性规范重新提取 source fact，逐项复核 body、constraint、collider、坐标转换和 ragdoll consumer boundary。验收：声明式资产审计、source/runtime transform audit 与 ragdoll 真实入口路径 PASS。
- [ ] BF111.10 / BF109.08 `AB_Als_Ragdolling`：按作者性规范重新提取 source fact，逐链路检查 pose input、混合、退出、PA consumer boundary 与 position。验收：声明式审计、source/layout audit、editor validator 与 ragdoll 真实入口路径 PASS；保持 `ragdoll_config → Outputs` 不变。
- [ ] BF111.11 / BF109.09 `ALI_Overlay`：按作者性规范重新提取 source fact，逐链路检查 interface pin、linked layer、fallback 与实例生命周期边界。验收：声明式审计、source/layout audit、editor validator 与 overlay 真实入口路径 PASS。
- [ ] BF111.12 / BF109.10 `CR_Als`：按作者性规范重新提取 source fact，逐项复核 graph/stage、骨骼／虚骨、Construction/runtime 分相、target-space 数据与 `AB_Als` 接入边界。验收：声明式资产审计、source/runtime transform/lifecycle audit 与 ControlRig 真实入口路径 PASS。
- [ ] BF111.13 / BF109.11 `AB_Als_Default`：按作者性规范重新提取 source fact，逐链路检查 linked layer、状态、基础/secondary motion、曲线、mask 与消费边界。验收：声明式审计、source/layout audit、editor validator 与 Default Overlay 真实入口路径 PASS。
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
