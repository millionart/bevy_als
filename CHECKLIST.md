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

## BF110 结构化动画上下文图资产（当前执行批次）

范围限定为本项目的动画图作者资产、`als-ext` 图节点、离线审计工具和 `manual_playground` 集成验证。不得修改 `bevy_animation_graph`、`bevy_animation_graph_editor`、Cargo dependency revision 或 `docs/als-ref-animation-asset-map.md`；`NodeTime`、事件和独立控制输入不得被错误并入上下文。BF110.01–04 的外部合同／模板／SVG 只证明了展开等价，**不满足实际 `.animgraph.ron`／现有图编辑器可读性**，不得作为本模块完成依据。BF110.05 起必须以项目内、按 `GraphContext`／state key 隔离的 animation-context node 在实际运行图中消除同一 pose 分支的逐曲线 edge，并重新完成全部 Gate。

- [x] BF110.01 动画上下文作者合同：建立项目内结构化连接 schema、开放曲线 ABI 发现与静态解析／自测，明确 pose-context、time、event、控制输入的边界。验收：合同拒绝未声明曲线、重复字段、跨上下文混接和非法 time/event 打包；对当前 `AB_Als` 根图建立完整基线清单，且不修改生成图。
  - 已实现（2026-09-01）：`tools/animgraph_contexts/ab_als.animcontext.json` 的每个 context 只声明 source/target pose、独立 time target 与 `curves: inherit` 的 target prefix；曲线集合由 source 图已声明的 F32 output 与 target 图已声明的 F32 input 自动求交，不允许逐字段 curve list。`attributes`、`events` 与 control 字段在合同中一律拒绝，避免伪造不存在的 runtime payload。
  - 基线与证据（2026-09-01）：`animgraph_context_contract.py` 对当前 `AB_Als` 记录 18 个节点、348 条 edge、11 条 context 与 118 条未托管 edge；11 条 context 分别自动发现 7/7/11/11/12/19/12/18/37/37/37 条曲线，完整 edge 和 node position 基线写入 `docs/bf110/ab_als.context-baseline.json`。`--self-test` 覆盖开放继承、固定字段拒绝、重复 context、非法 time/event 打包、跨 context 混接及未声明曲线；`--check`、Python 编译和 161 个 graph asset 只读校验均通过，生成 `.animgraph.ron`／`.fsm.ron` 零改动。
- [x] BF110.02 离线展开器与审计：实现结构化源图到标准 `.animgraph.ron` 的确定性展开、只读等价审计和幂等性检查；生成物保持 editor 元数据、node position、input order 和现有 runtime ABI。验收：最小 fixture 与 malformed fixture 全部自测通过，展开图可通过既有 editor asset validator，审计逐字段报告零差异。
  - 已实现（2026-09-01）：`expand_animgraph_context.py` 可从去除 context edge 的 `.structure.ron` 模板恢复标准图；它使用 context 合同自动发现曲线，并用完整基线恢复已有 edge 顺序。审计把 `edges_inverted` 外的 compact RON 内容与有序 edge 列表分别比较，因此 metadata、node position、IO/default 数据和全部连接均受保护。
  - 证据（2026-09-01）：自测覆盖 template round-trip、重复展开幂等、新增 source/target 声明后的自动 curve edge 与将展开图误作 template 的拒绝。真实根图 smoke 在 `target/bf110/context-expander-smoke/` 中从 348 条 edge 制作含 230 条 context edge 的模板并重新展开；审计输出 reference=348、actual=348、differences=[]，生成运行时图零改动。
- [x] BF110.03 `AB_Als` 根图试迁移：把 Standing、Crouching、Grounded、两个 Slot、Overlay/Inertialization 等同一分支的 pose/curve edge 迁入结构化作者源，生成正式根图；保留所有独立 pose 分支、`NodeTime`、event 与控制 edge。结构化作者布局必须精确保留 ALS-Refactored `AB_Als` AnimGraph 的主链、Overlay 下支、节点顺序和相对 position；普通生成 RON 保留相同 node position。验收：展开结果与迁移前基线的 node、position、edge、pin、curve/time/control ABI 精确等价，根图静态合同与运行时 contract gate 通过。
  - 已迁移（2026-09-01）：非运行时 manifest 位于 `tools/animgraph_contexts/`，结构模板位于 `assets/graph_sources/`，避免 AssetServer 将合同作为未知图资产加载。模板保留 18 个运行节点、全部 source-matched position、IO 与 118 条独立 edge；11 个 context 自动材化 230 条 pose/time/curve edge，正式 `assets/graphs/ab_als/ab_als.animgraph.ron` 恢复为 348 条标准 edge。`--check-output` 强制证明模板重新展开与正式图零差异。
  - 布局与等价证据（2026-09-01）：`ab_als.structural-layout.svg` 只展示 13 个可见节点／12 条结构链，严格保留 Standing/Crouching→Grounded→Transition→Locomotion→PostLocomotion→Layering→Head→ControlRig→Ragdolling→Output 主链和 Overlay→Inertialization 下支及源 position。根图迁移前后审计为 reference_edges=348、actual_edges=348、differences=[]；合同 baseline、`audit_ab_als_root.py --check`、定向 node-position verifier 及 editor validator fix/read-only（161 文件、0 repair/0 failure）均 PASS。
  - 运行时证据（2026-09-01）：`RUST_LOG=info cargo run --example ab_als_contract_gate --jobs 2` 在加载正式根图后输出全部 linked/stance/root_chain/overlay/slots/inertialization/view/control-rig/IO/scene architecture=true，并依次通过 idle、Transition Slot、PostLocomotion Slot、Torch Overlay、Ragdolling、restore，最终 `linked=true -> PASS`；日志为 `target/bf110/logs/BF110.03.ab-als-contract-gate.log`。
- [x] BF110.04 外部预览／展开子 Gate：全量运行结构化展开／审计、graph asset fix/read-only validator、相关 Rust／Python 自测、`AB_Als` contract gate 和真实 `run_manual.bat` Playground 路径。该项只证明旧普通 RON 的展开等价，不能证明实际图编辑器可读性；最终验收由 BF110.06 承担。
  - 工具／资产证据（2026-09-01）：三个 BF110 Python 工具 self-test 全 PASS；context `--check` 记录 11 context、348 edge、118 unmanaged edge、零 baseline difference，expander `--check-output` 记录 reference=348、actual=348、零 difference，layout SVG check 为 13 nodes/12 links。全量 editor asset validator 先 `--fix` 再只读检查 161 文件、0 repair/0 failure；root source audit 与定向 node-position verifier 均 PASS，结构 SVG XML 解析通过。
  - Rust／真实入口证据（2026-09-01）：`cargo test --lib --jobs 2` 为 149/149，`cargo test -p als-ext --lib --jobs 2` 为 206/206，`cargo check --all-targets --jobs 2` 通过（仅既有 example/test warnings）。这三个命令使用当前根 worktree 内的 `target_game` 隔离目录，未指向其他 checkout；`RUST_LOG=info cargo run --example ab_als_contract_gate --jobs 2` 通过。`ALS_MANUAL_INTEGRATION_GATE=1`、`ALS_MANUAL_INTEGRATION_CASE=base` 的 `run_manual.bat` 在真实可见 Playground、PlayerStart、原始地形、正常相机和完整角色根图链上以 exit code=0 完成，输出全部 EXECUTED 项及 `scenario=Base real_entry_paths -> PASS`；完整日志为 `target/bf110/logs/BF110.04.manual-base-background.stderr.log`。
- [x] BF110.05 实际 animation-context 边界节点：在 `als-ext` 实现通用、按 graph context/state key 隔离的 curve context 捕获、继承和子图 IO 注入；ALS 根图配置只声明 pose/time/context prefix，不在同一分支手接 F32 curve edge。不得使用跨角色 global map、裸字符串全局 bridge 或依赖修改。验收：`AB_Als` 的 Standing/Crouching→Grounded、两个 Slot、Overlay/Inertialization、Layering、Head、ControlRig、Ragdolling 实际 RON 仅保留图示所需 pose／必要 time／独立控制连线；配置 context 的任意 source/target ABI 新曲线会自动发现；逐 context state 隔离、curve 值、time、slot／ControlRig／ragdoll 路径均有测试和真实运行证据。
  - 已实现（2026-09-01）：`AnimationContextState` 是附着在当前 `GraphContext` 的 node/state key 上的 `HashMap<String, f32>`，不使用 Bevy resource、跨角色 global map 或依赖修改。每个 context consumer 先经 Pose edge 驱动 source node，再优先读取同一 state key 的 state；首层普通节点则从 node output cache 读取。`AnimationContextIoEnv` 只对 child graph 的 prefix F32 input 注入曲线值，未匹配的 data/time 继续走原 parent IO；source/target 图新增匹配名称的 F32 ABI 后自动被枚举，不需添加 root edge。
  - 根图结果与证据（2026-09-01）：`apply_runtime_animation_context.py` 将 `AB_Als` 从 18 node/348 edge 改为 16 node/94 edge，删除 254 条逐曲线／零值 bridge edge，静态审计为 `residual_context_curve_edges=0`。主 Pose/Time 链、Overlay/selector/reference 支、slot weight telemetry 与最终 root output 保留；`root zero curve` 和未使用 root reference bridge 已移除。runtime source audit、定向 position verifier、editor asset validator fix/read-only 和 208 个 `als-ext` 单元测试均 PASS。`AB_Als` contract gate 运行时实际覆盖 idle、两个 slot、Torch overlay、ControlRig、Ragdolling/restore，并以 `linked=true -> PASS` 收尾；日志为 `target/bf110/logs/BF110.05.runtime-context-contract.log`。
- [x] BF110.06 实际图可读性总 Gate：重新运行 context 静态审计、editor asset fix/read-only validator、图编辑器 layout 证据、相关 Rust/Python 自测、`AB_Als` contract gate 与真实 `run_manual.bat` Playground。验收：用户直接打开 `assets/graphs/ab_als/ab_als.animgraph.ron` 时，节点链、position 和主要连线与 ALS-Refactored `AB_Als` 图示等价，不存在同一 pose context 的逐曲线连线噪声；运行时数值／链路零缺口、无依赖仓库修改、真实入口 PASS。
  - 实际图／编辑器证据（2026-09-01）：`apply_runtime_animation_context.py --check` 输出 16 node、94 edge、`residual_context_curve_edges=0`；原 348 edge 中 254 条 context curve／zero bridge edge 已移除。`audit_ab_als_root.py --check` 对本地 UE source contract 输出 zero difference，定向 node-position verifier 与 editor asset validator fix/read-only（161 文件、0 repair/0 failure）均 PASS。当前源码构建的 `als_graph_editor` 已启动并加载该实际图；主 Pose/Time 链、Overlay 下支与 source position 可直接在其中检查，不依赖历史 SVG。
  - 运行时／真实入口证据（2026-09-01）：`cargo test --lib --jobs 2` 为 149/149，`cargo test -p als-ext --lib --jobs 2` 为 208/208，`cargo check --all-targets --jobs 2` 通过（仅既有 warnings）。`RUST_LOG=info cargo run --example ab_als_contract_gate --jobs 2` 通过所有 context、slot、overlay、ControlRig、Ragdolling/restore phase；`ALS_MANUAL_INTEGRATION_GATE=1`、`ALS_MANUAL_INTEGRATION_CASE=base` 的 `run_manual.bat` 以 exit code=0 完成全部 EXECUTED 项并输出 `scenario=Base real_entry_paths -> PASS`，日志为 `target/bf110/logs/BF110.06.manual-base.stderr.log`。

> BF110.06 的早期闭环只清除了逐曲线 edge；用户复查确认根 `Outputs`、`ALI_Overlay` 与 `AB_Als_Layering` 仍暴露曲线／`NodeTime` pin，故不能作为本批次最终布局验收。以下两项重新收口该缺口。

- [x] BF110.07 根图曲线输出与时间通道内聚：将 `ALI_Overlay`、Inertialization、Layering、Head、ControlRig、Ragdolling 及其根图消费者改为按 graph context/state key 读取结构化曲线；根 `AB_Als` 不再把曲线、telemetry 或逐分支 `NodeTime` 暴露为 `Outputs`／`Inputs` pin。保留 Pose 主链及由底层 Avian ragdoll runtime 强制要求的 `ragdoll_config` root ABI；所有时间更新必须在项目侧边界节点内部以当前 graph context 的同一请求注入子图，不能冻结、伪造或删除动画同步。验收：实际根图没有 `Inputs` 节点、Overlay 无 F32 输出 pin、Layering 无曲线／`NodeTime` 输入 pin、Ragdolling 到 `Outputs` 只保留 Pose 与必要 `ragdoll_config`；曲线、slot、ControlRig、Foot IK、Head、ragdoll 路径和 overlay transition signal 均从 context state 取得等价运行值。
  - 已实现（2026-09-01）：`AB_Als` 现在为 12 个运行节点、13 条可见 edge。`LayeredGraphNode`、`SkeletonAwareLayeredGraphNode`、Inertialized Linked Layer、Inertialization、ControlRig 和本地 reference pose 支持从当前 `GraphContext` 直接取得时间请求并向 child graph 注入；根图不再声明 input/output time。Overlay 的 selector/reference pose 改为内部 global-data input，删除 selector/reference bridge；曲线和 transition signal 保留在同一 state key 的 context state／node cache 中。最终 Ragdolling/Inertialization 值在 `AnimationGraphSet::Final` 后投影为 `AlsRootAnimationContext`，Layering、Head、ControlRig、Foot IK 和 manual Playground 不再读取 root F32 outputs。
  - 根图与运行时证据（2026-09-01）：`apply_runtime_animation_context.py --self-test` PASS，实际检查为 nodes=12、edges=13、三个 residual=0；本地 UE source audit 为 graphs=8、nodes=43、pins=44、target_nodes=12、differences=0；editor asset validator fix/read-only 为 161 files、0 repair/0 failure，定向 node position verifier PASS。`RUST_LOG=info cargo run --example ab_als_contract_gate --jobs 2` 通过 idle、两个 slot、Torch overlay、transition signal、ControlRig、Ragdolling/restore；ControlRig Contract Gate 与 Canonical Scale Gate 均 PASS。真实 `run_manual.bat` 入口在 `ALS_MANUAL_INTEGRATION_GATE=1`、`ALS_MANUAL_INTEGRATION_CASE=base`、`RUST_LOG=info` 下完成实际 Playground 场景、角色、相机、动画图和完整 Base 覆盖，最终输出 `scenario=Base real_entry_paths -> PASS`。
- [x] BF110.08 实际 `AB_Als` 布局重新 Gate：在 BF110.07 后重新运行 runtime context 静态审计、editor asset fix/read-only validator、node position 对标、相关 Rust/Python 自测、`AB_Als` contract gate 与真实 `run_manual.bat` Playground。验收：直接打开 `assets/graphs/ab_als/ab_als.animgraph.ron` 的布局、节点顺序、position 和可见连接与 ALS-Refactored `AB_Als` 图示等价；无 root `Inputs`、无同一上下文 F32／time 噪声、无依赖仓库改动，真实入口连续运行日志 PASS。
  - 实际图布局证据（2026-09-01）：根图只保留 Standing/Crouching→Grounded→Transition→Locomotion→PostLocomotion→Layering→Head→ControlRig→Ragdolling 的 Pose 主链，Overlay→Inertialization 下支在同一 source position；可见 pin 名与本地 UE source 对齐为 `Standing Input`、`Crouching Input`、`Grounded Input`、`Locomotion Input`、`Overlay Input`、`View Input`、`Source`、`Ragdolling Input`。根图无 `Inputs`、无 `NodeTime`、无 F32 root outputs；`Outputs` 只保留 Pose 和被 Avian runtime 固定读取的 `ragdoll_config`。`apply_runtime_animation_context.py --check` 为 nodes=12、edges=13、三个 residual=0；本地 source audit differences=0，node-position verifier 通过。
  - 最终验证证据（2026-09-01）：editor asset validator fix/read-only 为 161 files、0 repair/0 failure；`cargo check --all-targets --jobs 2` 通过，`cargo test --lib --jobs 2` 为 149/149，`cargo test -p als-ext --lib --jobs 2` 为 208/208。`RUST_LOG=info cargo run --example ab_als_contract_gate --jobs 2` 的静态 linked/stance/root/overlay/slot/inertialization/view/control-rig/IO/scene architecture 全为 true，并以 `linked=true -> PASS` 收尾。真实 `run_manual.bat` 在 `ALS_MANUAL_INTEGRATION_GATE=1`、`ALS_MANUAL_INTEGRATION_CASE=base`、`RUST_LOG=info` 下使用已加载、可见的原始 Playground、正常角色和相机完成所有 Base `EXECUTED` 覆盖并输出 `scenario=Base real_entry_paths -> PASS`。全程未修改 `bevy_animation_graph`、`bevy_animation_graph_editor`、Cargo dependency revision 或 `docs/als-ref-animation-asset-map.md`。

> 复开记录（2026-09-01）：用户复查发现 `AB_Als_Ragdolling` 仍显示两个 Bool output pin。实时本地 UE 资产提取确认原节点只有 Pose output 与 Ragdolling Input，故这两个 pin 为项目侧边界泄漏，必须移除后重过 BF110.08。根图空 `Inputs` 节点经本地 `graph_show.rs` 证实由编辑器无条件创建，即使 `io_spec.sorted_inputs()` 为空也会被 push；它无 pin、无 edge、无运行时语义。BF110 范围禁止修改编辑器依赖，故该 UI 空壳作为已知编辑器限制保留。

  - 复开收口（2026-09-01）：`LayeredGraphNode` 增加 auxiliary-output 可见性白名单；Ragdolling 只保留 Pose 与显式 `ragdoll_config`，隐藏 `ragdoll_dynamic` / `ragdoll_readback` 两个 Bool pin。`ragdoll_config` 继续连接 root `Outputs`，因为本地 `bevy_animation_graph` Avian physics 系统按该固定根输出读取配置。`apply_runtime_animation_context.py --self-test`、root audit、editor validator、209 个 `als-ext` 单测、全目标 cargo check、AB_Als contract 与真实 Playground Base Gate 全部再次 PASS。

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
