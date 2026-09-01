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
- [ ] BF111.03 / BF109.01 `AB_Als_Standing`：按作者性规范重新提取本地 UE source fact，递归逐链路检查 Standing 顶层、Movement States、每个 Move * state AnimGraph、Movement Details、Stop、六向 BlendSpace 和 Lean，并修订实际图／合同／位置与 context 边界。验收：所有可达 authored subgraph 的节点、节点数量、顺序、pin、edge、static property/value、node position、cache/状态机层次与 source 精确等价；comment group 按本项明确豁免；递归声明式审计、source/layout audit、editor validator 与 Standing 真实入口路径 PASS。
  - 复开失败（2026-09-01）：用户提供的本地 UE `AB_Als_Standing` AnimGraph 截图证明此前 BF111.03 的验收对象错误。source 顶层应可见六个 WalkRun BlendSpace/Save Cached Pose、Sprint branch、Lean branch、Movement States、Movement Details States、Standing States、Inertialization、refresh nodes、Modify Curve 与 Output Pose；项目目标却把它们拆成大量 child RON 和 project telemetry node。此前 authoring manifest 只从 target 反向生成 node/edge/position，因此即使 target 内部自洽，也不能证明 source-visible layout 等价。
  - 失效结论：此前 34 graph/416 node/734 edge 与 4 FSM/22 state/51 transition 的合同、source coverage、runtime Gate 和 Playground PASS 均只证明拆分实现存在或运行，**不得**作为 BF111.03 的 source top-level visual structure 等价证据。此项恢复未完成；BF111.04 及后续项暂停。
  - 用户决策（2026-09-01）：UE comment group 不纳入本轮等价阻塞项；source comment 仍由 native contract 完整保留，visible map 必须显式声明 `comment_policy: ignore` 及此决策。该豁免不适用于 node、node count、pin、edge、state machine、cache、position 或 Output。
  - 重做要求：重写 skill、authoring schema 与 Gate，使其先从 fresh UE payload 建立 source top-level node/pin/link/comment/position contract，再对实际打开的 target root 图逐节点比较；禁止 target-derived manifest 充当 source layout reference，禁止用 node 拆分/数量汇总或运行日志替代 source-visible graph equivalence。完成该工具修复后重新提取 Standing source、重建 target 根图并重过完整 Gate。
  - source 可见事实（2026-09-01）：本地 native `AB_Als_Standing` payload 经 `extract_ue_visible_graph_contract.py` 提取为 `docs/bf111/source_contracts/AB_Als_Standing.AnimGraph.source.json`（canonical SHA-256 `ddb06b3eed63cec0ad9779dd40c5836a1ec127e92ce377101ece269721342090`）。顶层含 43 个 source object，其中 5 个为保留的 comment group、37 个为可见功能节点；37 个节点以 source 顺序、标题、position、完整 pin/binding 和 28 条 directed link 进入 source-visible contract，`Output Pose` 位于 `(0, 0)` 且仅有 `Result` Pose input。命令行已成功打开本地 UE 资产；Slate Inspector MCP 截图超时，因此按规则从 native contract 渲染 source layout 并保留失败记录。
  - 实现与 source-visible Gate（2026-09-01）：`standing.animgraph.ron` 现在直接承载 37 个 source-visible 非 comment 节点、28 条 source-visible edge、相同节点顺序/position 与唯一 `Result` root output；`standing_states_runtime.animgraph.ron` 和 `standing_runtime_entry.animgraph.ron` 仅承载不可见的 Bevy runtime 适配。`run_ue_visible_animgraph_audit.py` 每次即时导出 Rust `NodeSpec` 后审计 `ab_als_standing.visible.json`，结果 `37/37` node、`28/28` link、`0` difference；map 以用户授权的 `comment_policy: ignore` 保存 comment exception，未豁免任何其他可见结构。
  - 行为事实与运行时 Gate（2026-09-01）：`AB_Als_Standing.RefreshStandingMovement.source.json` 记录本地插件 C++ 缺失时的官方源码后备事实：`SprintBlockAmount` 取当前动画曲线 `SprintBlock` 的 Clamp01，running/sprinting amount 由 `PoseGait`/`PoseGrounded` 的 source 分支计算。Standing layer 以此驱动 Sprint Block、Gameplay Tag 和未加权 gait；`standing_contract_gate` 验证 29 条 Movement、14 条 Movement Details、12 条 Standing 直接转换及 six-direction/sprint/stop/lean probe，PASS。`ab_als_contract_gate` 验证 Standing/Crouching 输入、完整根链、Overlay、ControlRig 与 Ragdolling 边界，PASS。
  - 完整验收（2026-09-01）：skill/tool self-test、Standing authoring manifest（3 graph/40 node/33 edge）、更新后的 AB_Als root manifest（12 node/13 edge）、editor validator fix/read-only（163 files、0 repair/0 failure）、Standing position verifier、source-visible audit、`cargo test -p als-ext --lib --jobs 2`（213/213）、`cargo test --lib --jobs 2`（151/151）和 `cargo check --example manual_playground --jobs 2` 全部 PASS。`run_manual.bat` 的真实 Base Playground case 已记录 `standing_graph` 已执行、`graph_err=none`、`pose_ready=true` 及 `real_entry_paths -> PASS`；本项之后才可开始 BF111.04。
  - property/value 续审（2026-09-01）：native `Modify Curve` 的 `CurveValues=(0.000000)` 与 `ApplyMode=Blend, Alpha=1` 先前未受 source-visible Gate 覆盖；已将 Standing 的 `PoseStanding`／`PoseMoving` target declaration 改为 `0.0`。`audit_ue_visible_animgraph.py` 与 map schema 新增 source/target property assertion，并以修改后的即时 NodeSpec 重跑 Standing，仍为 `37/37` node、`28/28` link、`0` difference；Standing manifest 与 runtime Gate PASS。
  - 递归验收复开（2026-09-02）：用户发现 `Movement States > Move *` state AnimGraph 未在此前 BF111.03 source-visible Gate 中逐图验收，且存在遗漏节点、错误 static input value 与 position 不对标。此前顶层 PASS、runtime PASS 与 manifest 仅能证明顶层，不能覆盖这六个 state graph；本项恢复未完成，BF111.04 及后续项暂停并须按递归 inventory 重新 Gate。
- [ ] BF111.04 / BF109.02 `AB_Als_Crouching`：按作者性规范重新提取 source fact，递归逐链路检查 Crouching 顶层、Movement States、每个 Move * state AnimGraph、外层状态、六向播放链与 Lean，并修订实际图／合同／位置与 context 边界。验收：所有可达 authored subgraph 的节点、节点数量、顺序、pin、edge、static property/value、node position、cache/状态机层次与 source 精确等价；comment group 按 BF111.03 的本轮明确豁免；递归声明式审计、source/layout audit、editor validator 与 Crouching 真实入口路径 PASS。
  - source 可见事实（2026-09-01）：fresh local native payload 由 `extract_ue_visible_graph_contract.py` 写入 `docs/bf111/source_contracts/AB_Als_Crouching.AnimGraph.source.json`（canonical SHA-256 `d7d54aee5ef2cdc519500e6385ed21a36c5787f26cdb9f8febf061de1614b234`，source graph SHA-256 `201f92f510a923a7c95354be85a77b4377a172eba9cf5ecea749e9280b96c616`）。顶层含 32 个 source object，其中 4 个 comment group、27 个非 comment 功能节点与 20 条 directed link；`Output Pose` 位于 `(0, 0)` 且仅有 `Result` Pose input。命令行已打开本地 UE asset；MCP 在已打开资产后的 `list_toolsets` 握手连续 90 秒无响应，因此按规则记录截图失败并从同一 native contract 渲染 source layout。
  - source graph 与 runtime 边界（2026-09-01）：`crouching.animgraph.ron` 重建为 source 顺序的 27 节点／20 edge 根图，保留 six Crouch Walk player、six cache、Movement States、stride Two Way Blend、lean Apply Additive、curve、reroute 与 refresh 链。six cache 实际通过 `cached_pose_inputs` 注入隐藏的 Movement States runtime FSM，随后 `Save cached pose 'Movement'` 注入 Crouching States runtime FSM；外层 Move/Stop 因而消费同一缓存而非重算。runtime entry 只解决 Bevy root output ABI，`AB_Als_Crouching` 自动发现 `als_crouching_curve_*`，并在曲线首帧未产生时以明确 `0.0` fallback 保护 Grounded context。
  - source state/value 取证（2026-09-01）：fresh native FSM position audit 覆盖 Crouching States 与 Movement States（2 FSM、0 difference）；Crouching authoring manifest 记录 12 graph／41 node／69 edge 和 1 FSM／6 state／24 transition。source-visible map 的 property assertion 校验两处 `Modify Curve` 的 native `CurveValues=(0.000000)` 对 target `value: 0.0`，并登记 UE `ExplicitFrame` 固定整数零值映射为 Bevy F32 零值的 ABI type exception。本地发布包缺少动画实例 C++ source，后备官方源码事实记录于 `AB_Als_Crouching.Refresh.source.json`：On Update 的 crouching stride/play-rate 公式、grounded yaw/hips lock 刷新与 rotate-in-place 的单帧 guard。
  - 完整验收（2026-09-01）：source-visible audit 为 `27/27` node、`20/20` link、`0` difference；editor validator fix/read-only（167 files、0 repair/0 failure）、两张 stance position verifier、FSM source position audit、Crouching/AB_Als/Standing authoring manifest、source property audit 工具 self-test、skill quick validator 与 AB_Als root audit 全部 PASS。`cargo test -p als-ext --lib --jobs 2` 为 214/214，`cargo test --lib --jobs 2` 为 151/151；Crouching contract gate 覆盖 outer 12 与 Movement 24 source transitions、Idle/Move/Rotate/Turn/Dynamic Transition，AB_Als root gate PASS。最终 `run_manual.bat` Crouching Playground case 记录六向、lean、rotate、turn、`graph_err=none`、所有 required executed coverage 与 `real_entry_paths -> PASS`；本项之后才可开始 BF111.05。
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
