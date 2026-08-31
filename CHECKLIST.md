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
```

## BF109 Character Base 动画图全量等价（当前执行批次）

范围限定为 `docs/als-ref-animation-asset-map.md` 的 Character Base 九个 package、`PA_Als`、`CR_Als` 与 `AB_Als_Default` 三个 `AB_Als` 直接依赖项，以及它们直接消费的 BlendSpace 图；该对照表保持只读。每项开始时重新从本地 ALS-Refactored 工程提取该 package 的实时事实，逐节点核验逻辑、常量、pin 链路、状态机、播放器、clip 选择和图拆分边界，以及每个节点在源图和目标编辑器中的 position；每个图必须保留完整节点、全部 pin 的类型/默认值/方向/索引、全部连接、状态边、转换属性和节点 position 的可复查原始证据，不得只保留压缩计数、摘要或代表样本。MCP 未暴露的数据必须由基于本机 UE 源码的原生读取工具直接反序列化，不得标记为可忽略或未核验。不得以此前审计结论代替当次取证。每项的真实入口覆盖路径均为 `run_manual.bat` → `manual_playground` → BF109 自动角色 probe，角色必须留在可见 Playground 地形与正常相机链中。

- [x] BF109.01 `AB_Als_Standing`：对标 Standing 根图、嵌套状态机、Movement/Stop 子状态、六向 WalkRun BlendSpace 与 Lean 链，逐项修复图节点、输入输出、时间/同步、曲线、转换条件和拆分颗粒度。验收：静态源/目标节点矩阵零未解释差异，图资产校验通过，真实入口 probe 覆盖 idle、六向 walk/run/sprint、pivot、stop、rotate/turn 与 lean，并输出逐状态、逐播放器 PASS。
  - 当次 ALS 事实（2026-08-31）：用本仓库 `tools/ue_graph_source_reader` 以 UE 5.8 的 `UBlueprint`、`UEdGraph`、`UEdGraphNode`、`UEdGraphPin` 和反射属性直接读取本地 `AB_Als_Standing`；完整输出为 258 个图、1157 个节点、2886 个 pin，包含每个节点的 source position、全部 pin 默认值/类型/方向/索引/连接和全部反射属性。MCP 仅作交叉读取，不作为完整性上限；原始可复查输出写入 `target/bf109/source_native/AB_Als_Standing.ue.json`。
  - 当前目标事实（2026-08-31）：`stances/standing.animgraph.ron` 与 `stances/standing/` 的完整 RON 采集共 38 个文件、416 个 animgraph 节点、22 个 FSM state、51 条 FSM transition；全部 416 个 animgraph 节点的 `editor_metadata.node_positions` 缺失，22 个 FSM state position 均存在。该 position 缺口必须随节点/链路等价一并修复，不得以自动布局替代源坐标对标；完整目标采集写入 `target/bf109/target/AB_Als_Standing.ron.json`。
  - 当次 BlendSpace 事实（2026-08-31）：六个 `BS_Als_WalkRun_*` 均为 loop=true、AnimLength=1.133333、X=`Stride Amount`、Y=`Walk Run Amount`，Cubic 插值窗口分别为 0.2/0.4 秒，四个样本坐标为 (0,0)/(1,0)/(0,1)/(1,1)；`BS_Als_Lean` 为 loop=false、AnimLength=0.033333、X=`Lean Right Amount`、Y=`Lean Forward Amount`、五个样本坐标为中心/前/后/左/右。所有七个源资产均由原生读取器重新导出；随后 `verify_runtime_blendspace.py` 对当前 Standing 的 Lean 与六向 WalkRun consumer 输出全 PASS。
  - 已修复并复验（2026-08-31）：四个同构 FSM 的 22 个 state position 已逐 label 写为源坐标；`standing_contract_gate` 同时断言该位置合同、源 X/Y Cubic 窗口和所有运行时 state/player 链，输出 `linked=true -> PASS`。`manual_runtime_gate` 的完整角色运行输出 PASS；源 C++ 证明 Running 的 ViewDirection 可保留 view-relative 朝向，故 probe 已仅在 VelocityDirection 或 ViewDirection+Sprinting 时对 capsule-facing/velocity 提示 mismatch，复跑真实 Gate 无未解释 probe WARN。
  - 完整节点/位置矩阵复验（2026-08-31）：`audit_source_graph_coverage.py` 的逐行输出保留 34 个图范围、416 个目标节点、1072 个非注释源节点以及每个节点的 source node、source position、target position、offset、pin 数和图范围；结果为 `missing_source_graphs=0`、`missing_source_anchors=0`、`unexplained_authored_graphs=0`、`unexplained_source_nodes=0`。其中直接 asset/title anchor 与 source-graph projection 都显式标注，projection 仅表示 Bevy 对同一源节点/图范围的拆分偏移而非无来源自动布局。`apply_source_layout.py --check` 对 416 个 position 零 mismatch；`verify_animgraph_node_positions.py` 无缺项/重叠/非法坐标。
  - Gate 证据（2026-08-31）：`cargo test --lib facing_probe_only_tracks_source_velocity_facing_modes --jobs 2` 通过；`cargo run --example standing_contract_gate --jobs 2` 覆盖四姿态 detail、29 条 Movement、14 条 Details、12 条 Outer transition、Idle/Forward/RightForward/Sprint/FirstPivot/PlantLeft/PlantRight/Rotate/Turn/QuickStop/Lean，结论 PASS；`cargo run --example manual_runtime_gate --jobs 2` 的原始完整日志在 `target/bf109/logs/manual_runtime_gate_cargo.stdout.log`，结论为 forward/backward/sideways/crouch/run/airborne/standing/crouching/jump/fall/land/overlay 全部 PASS。此前同一真实入口 `run_manual.bat` 的 `Base real_entry_paths` 结论为 PASS；本项末尾只改 editor position 元数据，未改变运行链语义。
- [x] BF109.02 `AB_Als_Crouching`：对标 Crouching 根图、Movement 子状态、六向 crouch 方向/播放链和 Lean 链，逐项修复图节点、输入输出、时间/同步、曲线、转换条件和拆分颗粒度。依赖：BF109.01。验收：静态源/目标节点矩阵零未解释差异，图资产校验通过，真实入口 probe 覆盖 crouch idle、六向移动、方向切换、stop、rotate/turn 与 lean，并输出逐状态、逐播放器 PASS。
  - 当次 ALS 事实（2026-09-01）：原生 UE 读取器重新读取实际对象路径 `/ALS/ALS/Character/AnimationInstances/Stances/AB_Als_Crouching`，完整原始输出为 147 个图、578 个节点、1458 个 pin；其中 Crouching States 的 state position 为 Idle=(176,0)、Move=(-16,-128)、Stop=(352,-128)、RotateRight=(352,128)、RotateLeft=(-48,128)，Movement States 六个 state position 与 Standing 的六向布局一致。根图六个 crouch walk SequencePlayer 都是 `Movement` SyncGroup、loop=true、rate=1；CrouchPose/WalkPose evaluator 都是 explicit frame=0、loop=true、teleport/reinitialize；Lean evaluator 的 X/Y 是 Lean Right/Forward Amount。`BS_Als_Lean` 同时重新原生导出，并经现有 runtime BlendSpace 审计复核。
  - 完整节点/位置矩阵复验（2026-09-01）：目标采集为 24 个 RON 文件、22 个 animgraph、177 个节点、11 个 FSM state、36 条 transition。`crouching_source_layout_map.json` 与 `audit_source_graph_coverage.py` 逐行记录 22 个图范围、177 个目标节点和 559 个非注释源节点的 source node、source position、target position、offset、pin 数和图范围；结果 `missing_source_graphs=0`、`missing_source_anchors=0`、`unexplained_authored_graphs=0`、`unexplained_source_nodes=0`。22 个图的 source layout check 零 mismatch，全部 177 个 node position 无缺项、重叠或非法坐标；两个 FSM 的 11 个 state position 由 contract gate 精确断言。
  - Gate 证据（2026-09-01）：`verify_runtime_blendspace.py --asset BS_Als_Lean` 输出 PASS；`crouching_contract_gate` 覆盖 12 条 outer、24 条 movement transition、Idle/Forward/Rotate/Turn/DynamicTransition 的 clip、loop、rate、curve、slot 和 pose 链，输出 PASS；`crouching_sync_gate` 覆盖六向 marker phase/sample time/pose，输出 PASS。真实 `run_manual.bat` 的 `ALS_MANUAL_INTEGRATION_CASE=crouching` 场景在可见原 Playground 中输出 state 0–5、graph input、实际速度、lean、rotate、Crouch 180° turn 全部 PASS；Base 场景随后完整通过 `real_entry_paths -> PASS`。Base 中 canopy 前增加可见场景内稳定重置，修复了 100 次 crouch 循环后物理弹跳导致的原有 canopy timeout，未替换 Playground 地形或相机链。
- [x] BF109.03 `AB_Als_Grounded`：对标 Grounded 根图、Standing/Crouching 路由、stance-change、roll 和 transition 链，逐项修复图节点、输入输出、时间/同步、曲线、转换条件和拆分颗粒度。依赖：BF109.01–02。验收：静态源/目标节点矩阵零未解释差异，图资产校验通过，真实入口 probe 覆盖 stance 往返、movement transition、roll 与回归，并输出实际状态/播放器 PASS。
  - 当次 ALS 事实（2026-09-01）：原生 UE 读取器重新读取 `AB_Als_Grounded`，完整原始输出为 120 个图、582 个节点、1573 个 pin。Grounded States 的 position 为 EntryConduit=(176,0)、Standing=(416,-128)、Crouching=(416,128)、StandingToCrouching=(416,256)、CrouchingToStanding=(416,-256)、Roll=(416,0)；源图包含 20 条 state transition（包括两个有效 self transition）、Roll evaluator 与 StandToCrouch/CrouchToStand clip 链。
  - 完整节点/位置矩阵复验（2026-09-01）：目标采集为 14 个 RON 文件、54 个 animgraph 节点、6 个 FSM state、20 条 transition。`grounded_source_layout_map.json` 与完整 coverage 输出记录 13 个图范围、54 个目标节点、579 个非注释源节点及逐行的 source node/position/offset/pin/范围；零 missing graph、anchor 或 unexplained source 项。13 个 position map 零 mismatch，所有 node position 与 FSM source position 经 `grounded_contract_gate` 断言。
  - Gate 证据（2026-09-01）：`grounded_contract_gate` 输出 root/states/curves/inertial/evaluator/outputs/architecture 全 true，覆盖 20 条 direct transition、两个 self edge、Roll frame=45/endpoint=1.5、所有 stance/roll 回路并 PASS；`grounded_dynamics_gate` 输出 source samples、start/stop speed、响应性全部 PASS。BF109.02 的最新完整 `run_manual.bat` Base 真实 Playground 日志继续覆盖 Grounded 的 stance 往返、roll、land、recovery 及最终 `real_entry_paths -> PASS`；本项只改 source 对标的 editor position 与 Gate 断言，未改运行语义。
- [x] BF109.04 `AB_Als_Locomotion`：对标 Locomotion 根图、Grounded/Air/Jump 状态机、Air Lean、Jump/Fall/Land 图链，逐项修复图节点、输入输出、时间/同步、曲线、转换条件和拆分颗粒度。依赖：BF109.03。验收：静态源/目标节点矩阵零未解释差异，图资产校验通过，真实入口 probe 覆盖 grounded、jump、fall、flail、land、air lean 与每条转换，并输出实际状态/播放器 PASS。
  - 当次 ALS 事实（2026-09-01）：原生读取 `AB_Als_Locomotion` 得到 147 个图、534 个节点、1331 个 pin。Locomotion States 的 position 为 Grounded=(176,0)、Jump=(576,-288)、Fall=(768,-288)、Land=(448,-64)、LandMovement=(400,64)、LandConduct=(656,0)；Jump States 为 Entry=(176,0)、Left=(416,-64)、Right=(416,64)、Loop=(640,0)、Flail=(816,0)。源同时重新导出 `BS_Als_Air_Lean`，保留五个 sample、轴、evaluator 和 air lean 路由事实。
  - 完整节点/位置矩阵复验（2026-09-01）：目标采集为 22 个 RON 文件、179 个 animgraph 节点、11 个 FSM state、21 条 transition。`locomotion_source_layout_map.json` 的 20 个映射图与 full coverage 输出逐行记录 179 个目标节点、516 个非注释源节点、source node/position/offset/pin/范围；所有 missing/unexplained 数为零，20 个 position map 零 mismatch。两个 FSM 的 11 个 state position 由 `locomotion_contract_gate` 精确断言。
  - Gate 证据（2026-09-01）：`verify_runtime_blendspace.py --asset BS_Als_Air_Lean` 为 PASS；`LOCOMOTION_CONTRACT_DIRECT=1 locomotion_contract_gate` 覆盖 16 条 Locomotion、5 条 Jump transition、Grounded 输入、Jump/Fall/Land/LandConduct/LandMovement、Air Lean/curve/loop/pose 链与最终 pose telemetry，输出 PASS。最新 Base 实景 Gate 已覆盖 jump、fall、land、jump variant 和最终链，输出 `real_entry_paths -> PASS`；本项只改 editor position 及 position contract。
- [x] BF109.05 `AB_Als_Layering`：对标 Layering 根图、Base Poses、Additive、slot、骨骼 mask 与曲线层链，逐项修复图节点、输入输出、曲线、混合顺序和拆分颗粒度。依赖：BF109.04。验收：静态源/目标节点矩阵零未解释差异，图资产校验通过，真实入口 probe 覆盖 idle/movement/air/action 的 layer 权重、骨骼区域与曲线值，并输出 PASS。
  - 当次 ALS 事实（2026-09-01）：原生读取 `AB_Als_Layering` 得到 5 个图、171 个节点、348 个 pin；唯一 authored AnimGraph 包含 Locomotion/Overlay input、Base Poses、local/mesh additive、slot curve、骨骼 section 与最终层叠顺序。目标采集为 11 个 RON 文件、89 个节点。
  - 完整节点/位置矩阵复验（2026-09-01）：`layering_source_layout_map.json` 覆盖根图与 10 个拆分图，逐行 audit 为 11 个 source scope、89 个 target 节点、147 个非注释 source 节点，missing/unexplained 均为零；所有 89 个 node position 均有源锚点且无 mismatch、重叠或非法值。
  - Gate 证据（2026-09-01）：`layering_contract_gate` 验证 root/slots/additive/curves/io/architecture 全 true；实际输出记录 curve accumulate、slot override、locomotion time 连续、Overlay swap 后 spine mask 权重，结论 PASS。最新 Base 真实 Playground Gate 覆盖 layering、overlay 与 action 组合并 PASS；本项仅补齐 source 对齐的 editor position。
- [x] BF109.06 `AB_Als_Head`：对标 Head 图、Look BlendSpace、head/neck 混合与曲线/输入链，逐项修复图节点、输入输出、播放器、坐标、混合与拆分颗粒度。依赖：BF109.05。验收：静态源/目标节点矩阵零未解释差异，图资产校验通过，真实入口 probe 覆盖 yaw/pitch 边界、回正和移动中 look，并输出 clip/sample/pose 链 PASS。
  - 当次 ALS 事实（2026-09-01）：原生读取 `AB_Als_Head` 得到 11 个图、23 个节点、57 个 pin；主链是 View Input → Mesh Space Additive，BS_Als_Look evaluator 位于 (-1104,112)，RefreshHead/InitializeHead 事件和 Output Pose 坐标均完整保留。`BS_Als_Look` 同时原生导出，保留 pitch axis、三个 sample、evaluator 与默认属性。
  - 完整节点/位置矩阵复验（2026-09-01）：目标 `head.animgraph.ron` 为 23 个节点；`head_source_layout_map.json` 与 full coverage 输出逐行对齐一个 source scope、23 个 target 节点、23 个非注释 source 节点，missing/unexplained=0，所有 position 有源锚点且零 mismatch。
  - Gate 证据（2026-09-01）：`verify_runtime_blendspace.py --asset BS_Als_Look` PASS；`head_contract_gate` 覆盖 View redirect 输入、look evaluator、reference delta、pitch=0/90 与 yaw blend pose 链并 PASS；`head_yaw_gate` 验证 body/view aligned 时 head neutral 并 PASS。最新 Base 真实 Playground Gate 覆盖 view_head 和最终 pose 链并 PASS；本项仅补齐 editor position。
- [ ] BF109.07 `PA_Als`：对标 Physics Asset 的全部 body、constraint、collider、质量、limit、collision filter 与到 ragdoll runtime 的映射；图逻辑保持在后续 `AB_Als_Ragdolling` 项中。依赖：BF109.06。验收：实时导出的资产 manifest、拓扑与数值矩阵零未解释差异，真实入口 probe 覆盖 body handoff、ground settle、collision 与 recovery，并输出 PASS。
- [ ] BF109.08 `AB_Als_Ragdolling`：对标 Ragdolling 图的 pose 输入、混合与退出链，消费 BF109.07 已验证的 `PA_Als` 合同；逐项修复图节点、输入输出、曲线和拆分颗粒度。依赖：BF109.07。验收：静态源/目标节点矩阵零未解释差异，图资产校验通过，真实入口 probe 覆盖 ragdoll enter、settle、recovery 的图/pose 链并输出 PASS。
- [ ] BF109.09 `ALI_Overlay`：对标 Overlay interface 的输入 pin、linked layer 选择、fallback 和实例生命周期，逐项修复图节点、输入输出、调用链和拆分颗粒度。依赖：BF109.08。验收：静态源/目标节点矩阵零未解释差异，图资产校验通过，真实入口 probe 覆盖 interface 绑定、切换、fallback 与重入，并输出 PASS。
- [ ] BF109.10 `CR_Als`：对标 Control Rig 的输入结构、执行顺序、骨骼/虚骨引用、足部与手部 IK、曲线读写、节点粒度和 `AB_Als` 接入边界；通用底层桥接放入 `crates/als-ext`，ALS 专用映射留在项目侧。依赖：BF109.09。验收：静态源/目标节点与输入输出矩阵零未解释差异，图资产校验通过，真实入口 probe 覆盖每个 rig stage、curve 与实际骨骼 target，并输出 PASS。
- [ ] BF109.11 `AB_Als_Default`：对标默认 Overlay 的 linked layer、状态机、基础/secondary motion、曲线、mask、节点链和 `ALI_Overlay`/`AB_Als` 消费边界。依赖：BF109.09–10。验收：静态源/目标节点矩阵零未解释差异，图资产校验通过，真实入口 probe 覆盖 default overlay 的所有状态、切换、curve 与最终 pose 链，并输出 PASS。
- [ ] BF109.12 `AB_Als`：对标 Character 根图的各 linked graph/layer 路由、最终混合顺序、ControlRig 边界和输出，逐项修复图节点、输入输出、路由与拆分颗粒度；本项只消费 BF109.10 已验证的 `CR_Als` 等价物。依赖：BF109.01–11。验收：静态源/目标节点矩阵零未解释差异，图资产校验通过，真实入口 probe 记录九个 Character Base package、`PA_Als`、`CR_Als` 和 `AB_Als_Default` 的实际执行路径与最终 pose 链 PASS。
- [ ] BF109.13 Character Base 总 Gate：从本地 ALS-Refactored 重新提取九个 Character Base package、`PA_Als`、`CR_Als` 与 `AB_Als_Default` 的最终事实并全量复审，确认不存在未解释的逻辑、值、节点、pin、状态、转换、clip、BlendSpace、时间/同步或拆分粒度缺口。依赖：BF109.01–12。验收：全量审计输出、图资产只读校验、相关 Rust 测试、所有 BF109 runtime probe 与 `run_manual.bat` 自动退出真实 Playground 日志均为 PASS，提交说明记录命令及证据摘要。

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
