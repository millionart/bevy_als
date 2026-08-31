# bevy_als — 执行清单

> 本文件是唯一权威执行清单；执行纪律见 `agents.md`（仅规则），启动方式见 `goal.md`。
> 完成一项即勾选并一次提交：实现 → 自测 → 勾选 `[x]` → `git commit -m "<前缀><编号>: <条目摘要>"`（勾选与实现同一提交）。
> 基座模块提交前缀沿用 `M<编号>`；缺陷修复清单用 `BF<编号>`。
> 已归档清单：[archivedchecklist.md](docs/archive/archivedchecklist.md)

## BF106 — P0 Foot IK 的空间合同、两骨求解与真实入口调度

目标：重建 foot target 从 world 至 armature-local、parent-local、bone-local 的显式合同，并将两骨 IK、权重／门控和层级传播固定在动画 pose 求值之后、skinning 之前。BF106 仅在 BF105.Gate 通过后开始。

- [x] **BF106.01 — I01：Foot IK 源事实、坐标约定与当前调度基线（P0）**
  - 依赖：BF105.Gate。
  - 验收：当前项开始时从本地 ALS-Refactored 重新提取 ControlRig、foot trace、骨骼／虚骨、曲线、动画门控和执行顺序；记录无私有路径的空间、单位、reference rotation、足部和 knee plane 合同，并以真实入口四向旋转、坡面／台阶边缘、IK 开关输出目标空间、长度误差和 pole sign。
  - 当次 ALS／ControlRig 事实（2026-08-31，重新读取本地 ALS C++、`CR_Als.uasset` 二进制并以 Unreal MCP ControlRigTools 读取 `CR_Als` graph、function graph 与连接）：animation instance 先从 IK foot bone／virtual bone 取 world target transform；foot lock 可在 world、movement-base 和 component-local 三个空间保存，最终输出时再由 component inverse 转回 local。`CR_Als` 主图实际包含 `RefreshFootOffset`、`RefreshPelvisOffset`、`RefreshFootIk`，其 connections 将 foot-offset/pelvis/foot-IK 函数串接；Foot Offset trace 把 VM-space foot target 经 `ToWorldSpace()` 做 Visibility line trace，命中 normal 经 world-to-rig inverse 转回 rig space、命中 location 转回 VM space。source defaults 为 upward 50cm、downward 80cm、walkable angle 45°、foot height 13.5cm。location unit 先限制 pelvis-to-foot distance、再限制 leg 至 `0.99` stretch；rotation 在 calf space 中相对 initial foot rotation 计算／限幅，再转回 rig global。左右脚分别执行同一 trace／location／rotation／IK path，不能以同一无符号 world forward pole 取代镜像 knee plane。
  - 当前项目基线（2026-08-31）：`foot_ground_probe_system()` 用 foot global transform 做世界 Y-up ray（up=.15m、depth=.85m），`foot_ik_apply_system()` 再以 global hip/knee/ankle 和 world target 解两骨 IK、最后经 parent inverse 写 local rotation；它以 character forward 作为左右共用 pole，且 `AlsFootProbe`／`AlsFootIkTargets`／runtime 没有显式 world/armature/parent/bone-local 类型字段或可复查的 target-space、链长误差、pole sign telemetry。调度为 probe/pin/state 在 PostUpdate propagate 后，apply 在 animation target 写入及 propagate 后再执行一次 propagation；需实证其不被后续写入覆盖。当前真实 Base 仅有 `foot_probe`／`foot_lock` coverage；现有 isolated `foot_ik_gate` 在 root global `als_root_post_locomotion_slot_weight` 缺失时失败，不能作为真实入口验收。I01 必须先补只读 telemetry 与真实入口基线，不改变 solver。
  - 归属预定：无 ALS 参数的空间标识、world→armature→parent→bone-local conversion、长度/pole 数值检查应评估归 `als-ext`；ALS trace distances、curve gate、骨名、virtual-bone selection、movement-base/foot-lock 生命周期和 manual scenario 归 `als-core`／主项目。
  - 实现：新增只读 `AlsFootIkTelemetry`／side telemetry，在实际 Foot IK path 中记录 final world target、character armature-local target、foot parent-local target、reference reach error、pole sign、IK weight 与 pin weight；缺失 bones 时 telemetry 与 existing runtime 一起归零。`manual_playground` 新增 `foot_ik` real-entry scenario，沿原始 PlayerStart、可见 terrain 和正常相机执行已有 cardinal/rotated direction regression，不运行未来 I04 的坡面或台阶辅助物；每个稳定 cardinal 及 rotated-90 窗口打印两脚三种空间、error、pole sign、weight，并使 verdict 要求五个样本、真实 foot probe 与 foot lock 同时有效。
  - 真实入口基线（2026-08-31）：`ALS_MANUAL_INTEGRATION_GATE=1 ALS_MANUAL_INTEGRATION_CASE=foot_ik RUST_LOG=<manual integration filter> run_manual.bat` 返回 0；WalkForward、RunBackward、RunRight、RunLeft、RunForwardRotated90 均记录 finite world/armature-local/parent-local targets 和 `IK=1.000,pin=0.000`。本次稳态样本的 reference reach error 为 0–.189m，左/右 pole sign 通常异号，但 rotated-90 sample 中两者同为负值；这实证当前共享 forward pole 不能作为 I03 通过条件。该场景没有将孤立 `foot_ik_gate` 的 root-global 缺失误作通过证据；I04 才建立原始 Playground 内的坡面、边缘和 IK switch 行为场景。
  - 证据：`cargo test -p bevy_als --lib foot_ik_apply --jobs 2` 为 4 passed；真实 `scenario=FootIk real_entry_paths -> PASS` 输出 runtime assembly、animation graph、standing graph、direction regression、foot probe、foot lock 和 foot IK space baseline 全部 `EXECUTED`。I02 必须先以显式空间合同修复当前无类型转换／首次帧误差基线，I03 再解决 mirrored pole 与 solver 稳定性；不得把本项 baseline 视为解算正确性验收。

- [x] **BF106.02 — I02：显式空间转换、足部 trace 与无命中恢复（P0）**
  - 依赖：BF106.01。
  - 验收：世界 hit position／normal 不会直接写入 bone-local transform；以类型或明确字段区分各空间，统一转换 up／forward 轴与 reference pose，并在无命中时平滑回零且不保留陈旧 target。通用空间／骨架桥接归属 `als-ext`，ALS trace 参数和资产映射留在项目；数值 Gate 覆盖旋转不变性和多角色隔离。
  - 当次 ALS／ControlRig 事实（2026-08-31，重新读取本地 `AlsRigUnit_FootOffsetTrace.cpp/.h` 并以 Unreal MCP `ControlRigTools` 重新列出 `CR_Als` graph／`TraceFootOffset` 节点）：source 在 disabled 时直接输出 offset=0、normal=up；trace start/end 以 VM-space FootTargetLocation 的 X/Y 和 +50cm/−80cm Z 构造，经 `ToWorldSpace()` 做 Visibility trace。命中 normal 先 inverse-transform 回 VM space；无 blocking hit 或 VM normal.Z 小于 cos(45°) 时同样输出 0/up，命中位置经 `ToVMSpace()` 后用 foot height=13.5cm 的 slope correction 计算 offset。MCP 仍显示 `CR_Als` 的 `TraceFootOffset` function graph 含 `AlsRigUnit_FootOffset` node，并由 `RefreshFootOffset`／`RefreshPelvisOffset`／`RefreshFootIk` 串接。因此 source 的 trace/offset 合同是 armature/VM-local 输入与输出，而非将 world hit 直接写入 bone-local。
  - 归属决定：不含 ALS 参数或骨名的 WorldPoint、ArmaturePoint、ParentPoint、BoneLocalPoint 与 direction conversion 归 `crates/als-ext::ik_space`；ALS 单位换算、45° slope gate、virtual foot target 选择、Avian trace、foot lock 和角色状态留在 `als-core`／主项目。没有改动依赖 fork，也没有将 ALS 常量下沉到通用扩展。
  - 实现：`als-ext` 新增显式 point/direction wrapper 和 world↔armature／parent／bone-local conversion。`als-core` 将 source trace 参数映射为 `.50m` upward、`.80m` downward、`.135m` foot height、45° walkable gate，改从 `ik_foot_*` virtual target 按 character up/down trace；由于 Avian hit 尚未含 source VM 的 foot-height correction，project adapter 在 armature-space raw offset 中显式加入 `.135m`。`AlsFootIkSpaceState` 为每脚保存 trace target、hit、solver reference、world／armature-local／parent-local／bone-local target、normal、raw offset 与 hit validity；solver target 先在 armature-local 加 smoothed offset，再转回 world，最后才转换为 parent/bone-local，禁止 world hit 直写 local transform。无 hit 时 raw offset=0、hit fields 归零，existing spring 使 target 连续回到 solver reference。
  - 真实入口：`foot_ik` 在原始 Playground 的方向回归稳定窗口记录 target 三空间与 trace-space；随后在同一 PlayerStart、可见角色和正常相机中实际起跳，只有两脚 probe 无 hit、raw offset=0 且 armature target 与 solver reference 的残差均不大于 `.03m` 才标记 no-hit recovered。该场景不引入独立世界、隐形地板或 I04 的坡面/台阶摆件。
  - 证据：`cargo test -p als-ext --lib ik_space --jobs 2` 为 2 passed；`cargo test -p als-core --lib foot_ik --jobs 2` 为 20 passed；`cargo test -p bevy_als --lib foot_ik_apply --jobs 2` 为 5 passed；`cargo run --example foot_ik_space_gate --jobs 2` 输出 rotation_error=.00000066、normal_error=0、isolation_distance=2.980866、PASS。真实 `ALS_MANUAL_INTEGRATION_GATE=1 ALS_MANUAL_INTEGRATION_CASE=foot_ik RUST_LOG=<manual integration filter> run_manual.bat` 返回 0：四向与 rotated-90 输出三种 target 空间、trace/hit/normal/raw offset；真实 Air 输出 `left_hit=false right_hit=false`、两侧 raw=0、target-reference delta 约 .000065，并以 `foot_ik_space_baseline`／`foot_ik_no_hit_recovered`／`scenario=FootIk real_entry_paths -> PASS` 收尾。
  - 全量回归（2026-08-31）：`cargo test -p als-ext --lib --jobs 2` 为 199 passed；`cargo test -p als-core --lib --jobs 2` 为 226 passed／1 ignored；`cargo test -p bevy_als --lib --jobs 2` 为 143 passed；`cargo check --all-targets --jobs 2` 通过（仅既有 examples warnings）。同一版本的真实 Base 在原始 Playground 中完成 CrouchRecovery、100 pairs、canopy、Turn、Mantle、Platform、Roll，并输出 `Base real_entry_paths -> PASS`。

- [x] **BF106.03 — I03：稳定两骨 IK、镜像 pole 与动画后调度（P0）**
  - 依赖：BF106.02。
  - 验收：左右腿使用镜像且稳定的 pole plane，近共线、过伸、零长度与权重切换都有退化保护；IK 固定在动画 pose 后、层级传播前执行，且不会被后续系统覆盖或重复叠加。关闭 IK 的基础动画保持正常，0↔1 权重连续；通用 solver／调度归属 `als-ext`，ALS 门控留在项目。
  - 当次 ALS／ControlRig 事实（2026-08-31，重新读取本地 `AlsRigUnits.cpp`／`AlsMath.cpp`、ApplyFootOffset Location/Rotation C++ 及以 Unreal MCP `ControlRigTools` 重新列出 `CR_Als` 的 `ApplyFootIk` function graph）：source `CalculatePoleVector` 将 B 投影到 A→C 后取垂线方向；A/B 重合失败，A/C 重合时以 AB 方向降级，三点共线时返回失败；Rig unit 在失败时保留上次成功的 pole。MCP 图同时含 `TwoBoneIKSimplePerItem`、`CalculatePoleVector`、`ExponentialDecayVector`、ApplyFootOffset Location/Rotation、curve weight 和 Set Transform。Location unit 以 pelvis distance、pelvis offset、12Hz/2.0 spring 与 .99 leg stretch 限制有效 target；Rotation unit 将 initial foot rotation 记录到 calf space，normal 用 .1s half-life 平滑，在 calf space 限制 swing/twist，再回写 global。故左右腿不得共享单一 world-forward pole，合法 reach clamp 应以有效 target 计误差。
  - 归属决定：projection pole、reference-local/last-world continuity、父骨旋转后的 child world correction、two-bone degeneracy guard、rotation slerp 与命名调度 set 均是无 ALS 常量的通用能力，归 `als-ext`／主项目通用调度；ALS 的 foot bones、curve/lock 输入、target selection、location/rotation source limits 与 manual override 归项目。未修改依赖 fork。
  - 实现：`als-ext::StableTwoBonePole` 为每条腿捕获独立 armature-local reference plane，在共线时重用 last-world plane；新增 child correction，使 calf 根据已应用 thigh rotation 对齐下段 target axis。项目 `FootIkPoleState` 左右独立保存状态，solver 使用 runtime smoothed IK weight 对 thigh/calf/foot local rotation slerp；`FootIkApplySet::Solve→Propagate→Telemetry` 固定在 animation targets 后读取 global、写 local、立即重传 hierarchy、再记录 post-propagation telemetry。`AlsFootIkApplyTelemetry` 记录有效 target、effector、误差、side pole sign 与实际 weight；target 若被 angle/reach 限制，会以 resolved target 验收并记录 adjustment。manual observer 显式排在 telemetry set 后。
  - 真实入口：`foot_ik` 的四向及 rotated-90 样本只在每个 3 秒 case 后段采样，要求左右 applied weight≥.99、resolved-target effector error≤.02m；实际日志显示 mirror-side pole 与 post-propagation error。无 hit jump 后新增在原始地形、可见角色、正常相机中的 `FootIkWeightRamp`：依次驱动 0/.25/.5/.75/1/0 override，并要求实际 telemetry 在每个窗口达到对应范围；它不会影响非 `foot_ik` scenario 的曲线门控。
  - 证据：`cargo test -p als-ext --lib ik --jobs 2` 为 53 passed；`cargo test -p bevy_als --lib foot_ik_apply --jobs 2` 为 5 passed；`cargo run --example foot_ik_solver_gate --jobs 2` 输出 mirror_dot=-.999997、child_alignment=1、blend_angle=.4、PASS。真实 `ALS_MANUAL_INTEGRATION_GATE=1 ALS_MANUAL_INTEGRATION_CASE=foot_ik RUST_LOG=<manual integration filter> run_manual.bat` 返回 0：五个延后窗口均记录 resolved-target error≤.02m；权重日志连续输出 off=0、quarter=.25、half=.5、three_quarters=.75、full=1、released=0，所有误差≤.0085，最终 `foot_ik_weight_ramp` 与 `scenario=FootIk real_entry_paths -> PASS`。
  - 全量回归（2026-08-31）：`cargo test -p als-ext --lib --jobs 2` 为 203 passed；`cargo test -p als-core --lib --jobs 2` 为 226 passed／1 ignored；`cargo test -p bevy_als --lib --jobs 2` 为 143 passed；`cargo check --all-targets --jobs 2` 通过（仅既有 examples warnings）。Base 的 CrouchClearance run 在 90 秒时只完成 75 pairs，属于既有 BF105 自动场景时限波动；不作为 I03 入口证据，也未将其混入本项实现。

- [x] **BF106.04 — I04：真实入口坡面、台阶边缘与动作门控回归（P0）**
  - 依赖：BF106.03。
  - 验收：`manual_playground` 在原始可见地形上以短时可见、可渲染且带 collider 的坡面／台阶摆件验证四向旋转、台阶边缘、crouch、jump、roll、mantle、land 与 IK 开关；逐帧记录所有目标空间、hip／knee／ankle 误差、pole sign、IK weight 与执行阶段，不得以静态截图代替。
  - 当次 ALS／ControlRig 事实（2026-08-31，重新读取本地 `AlsAnimationInstance.cpp`、`AlsCharacter.cpp`、`AlsCharacter_Actions.cpp` 与 ControlRig `ApplyFootIk` graph）：animation instance 从 source setting 选择 `ik_foot_*` 或 foot virtual bone 的 world transform，并分别消费 `FootLeftIk`／`FootRightIk`、lock curves。source Jump 仅在 Standing、无 locomotion action、Grounded 时开始；Grounded roll 仅在无 action 或已结束同一 roll montage时开始，并按 input 或 actor yaw 选择目标；desired stance 改变后重新 ApplyDesiredStance，rolling setting 可请求 crouch。`CR_Als` ApplyFootIk graph 同时包含 per-item two-bone IK、pole、location/rotation offsets、curve value、interpolation 和 set transform。因此 terrain 回归必须同时验证实际 action、curve gate 和 post-IK bone结果，不能只检查 prop 或静态图。
  - 归属决定：短时 slope/step fixture、ALS bone/curve/action gate、角色输入、真实入口状态机和日志均属项目；通用 two-bone/space/pole 能力继续留在 `als-ext`。Fixture 仅使用已有 mesh/material，在原始 Playground ground 上以 Static collider + visible mesh 创建，完成或 timeout 都 despawn；不替代地形、PlayerStart 或相机链。
  - 实现：`manual_playground` 的 `foot_ik` scenario 在 I03 no-hit/weight ramp 后进入 `FootIkTerrain`。它在 PlayerStart 的真实 ground 起步区创建红色宽 slope 与相连 step，Walking 穿越、实际 crouch、Standing jump/land 与 roll，清理 props 后沿已有真实 Mantle 和 Roll 路径继续。`AlsFootIkSpaceState` 保存 resolved knee target，post-propagation telemetry 输出 effective target、hip/knee/ankle positions、hip/knee/ankle error、side pole 和 actual weight；observer 严格排在 `FootIkApplySet::Telemetry` 后，只有有效 target error≤.02m 才记录 terrain action。每个 slope/step/crouch/jump_air/landed/roll/mantle stage 输出完整多空间数值，并分别要求 scenario coverage；terrain cleanup 也纳入 verdict。
  - 证据：真实 `ALS_MANUAL_INTEGRATION_GATE=1 ALS_MANUAL_INTEGRATION_CASE=foot_ik RUST_LOG=<manual integration filter> run_manual.bat` 返回 0。日志先记录可见 props 的 slope/step entities 与中心位置，随后输出 `FOOT-IK-STAGE` 的 slope、step、crouching、jump_air、landed、roll、mantle；每条含左右 target four spaces、hip/knee/ankle、errors、pole、weight。实际 verdict 列出 `foot_ik_slope`、`foot_ik_step`、`foot_ik_crouching`、`foot_ik_jump_air`、`foot_ik_landed`、`foot_ik_roll`、`foot_ik_mantle`、`foot_ik_terrain_cleaned` 全部 `EXECUTED`，并以 `scenario=FootIk real_entry_paths -> PASS` 收尾。
  - 全量回归（2026-08-31）：`cargo test -p als-ext --lib --jobs 2` 为 203 passed；`cargo test -p als-core --lib --jobs 2` 为 226 passed／1 ignored；`cargo test -p bevy_als --lib --jobs 2` 为 143 passed；`cargo check --all-targets --jobs 2` 通过（仅既有 examples warnings）。

- [ ] **BF106.Gate — Foot IK 空间、求解与调度全链路验收（P0）**
  - 依赖：BF106.01–BF106.04。
  - 验收：源事实、空间转换、trace、两骨 solver、权重／门控、系统顺序和真实入口日志构成闭环；四向旋转、坡面、台阶边缘、蹲伏与落地均无翻膝、扭曲、单帧跳变或跨帧发散，关闭 IK 后基础动画正常。

## BF107 — P1 状态机语义与持续真实入口行为回归（待 BF106.Gate）

目标：在三个 P0 闭环通过后，以真实运行日志逐项验证 Idle／Start／Cycle／Stop／Pivot、standing／crouching、walk／run／sprint、grounded／jump／fall／land、turn／rotate、roll／mantle／ragdoll／get-up、overlay、转场中断／惯性化、root motion、sync marker、Notify State 与多角色隔离。

- [ ] **BF107.01 — S01：建立按行为维度的源事实与真实入口回归矩阵（P1）**
  - 依赖：BF106.Gate。
  - 验收：当前项开始时重新提取本地 ALS-Refactored 的每个覆盖维度事实；为每个行为登记真实入口触发、可观测参数、预期状态路径、数值断言和日志路径。未覆盖项必须明确列为未通过，不能用图加载、组件存在或单帧截图代替。

- [ ] **BF107.Gate — P1 行为矩阵与持续回归验收（P1）**
  - 依赖：BF107.01 及其后续按矩阵展开的条目。
  - 验收：全部登记行为均有真实入口连续运行证据；若矩阵发现新增缺陷，先将其登记为按依赖顺序的 BF 条目并修复、重新过相关 P0／P1 Gate。

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
─► BF106 Foot IK 闭环 ─► BF107 状态机语义与持续回归
```

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
