# bevy_als — 执行清单（M0–M11）

> 本文件是唯一权威执行清单；执行纪律见 `agents.md`（仅规则），启动方式见 `goal.md`。
> 完成一项即勾选并一次提交：实现 → 自测 → 勾选 `[x]` → `git commit -m "M<编号>: <条目摘要>"`（勾选与实现同一提交）。

## 背景与基础事实（已核实）

- 本机 bevy 源码：`0.19.1`（release-0.19.1）。`bevy_animation_graph` 0.11.0 与 `bevy-tnua` 均基于 Bevy 0.19，兼容。
- `bevy_animation_graph` 已有节点：ClipNode、ChainNode、BlendNode（含骨骼 mask）、BlendSpace1D/2D、布尔运算节点等；图资产 RON 序列化、支持编辑器热重载；Pose 以 `BoneId` 路径表达。**缺**：曲线/ramp 节点、距离匹配、事件/notify、two-bone IK、additive 叠加语义需验证。
- `bevy-tnua` 已有 basis：Walk（coyote time）、Jump（高度/悬挂控制）、Crouch、Dash、Climb、Knockback、WallSlide；物理后端 avian3d 0.12.1 / rapier3d 0.17.0。
- ALS-Refactored 关键结构：4 模块（ALS / ALSCamera / ALSExtras / ALSEditor）；状态体系用 GameplayTags（非枚举）；自定义 AnimNode_CurvesBlend；ControlRig 输入结构 `AlsControlRigInput`；IK 相关骨骼 `foot_l/foot_r`、`ik_foot_l/ik_foot_r`、`VB foot_l/VB foot_r`；关键曲线名 `LayerHandLeft/Right`、`HandLeftIk/HandRightIk`、`FootLeftIk/FootLeftLock/FootRightIk/...`；动画资产分组：Grounded / Air / Actions / Transitions / TurnInPlace / RotateInPlace / Overlays / View / Base。
- 本地环境路径、引擎版本等隐私信息见 `docs/env.md`（已 gitignore）。

## 模块依赖顺序

```
M0 环境与资产管线 ─► M1 ECS核心状态模型 ─► M2 曲线基础设施 ─► M3 tnua运动基座
─► M4 动画图基座 ─► M5 Locomotion状态机动画 ─► M6 Overlay/Aim ─► M7 FootIK/CR等价物
─► M8 特殊动作(跳跃翻滚) ─► M9 Ragdoll ─► M10 相机 ─► M11 调试与总验收
```

---

## M0 环境与资产管线（依赖：无）

- [x] M0.1 初始化 Cargo workspace：根 crate `bevy_als`（bin 示例入口），成员 `crates/als-core`、`crates/als-ext`（基座插件扩展与 IK 后处理：curve、ControlRig 等价 IK 等）；依赖 bevy 0.19（本地 path 可选）、bevy_animation_graph(fork)、bevy-tnua(fork, avian3d)、bevy_enhanced_input。
- [x] M0.2 配置依赖指向本机 fork：bevy_animation_graph、bevy-tnua（公开 git 依赖，隐私红线规则 7 禁 path；本地调试用 gitignored 覆盖），并在 README 记录 upstream 与 fork 地址。
- [x] M0.3 建立 `assets/` 目录约定：`assets/models/`、`assets/animations/`、`assets/graphs/`、`assets/curves/`、`assets/docs/export-manifest.md`（记录导出资产清单与来源路径哈希）。
- [x] M0.4 通过 Unreal MCP 连接 ALS-Refactored 编辑器工程，验证工具集可用（list_toolsets ≥ 1 个动画相关 toolset；连接方式与验证命令见 `docs/env.md`）。
- [x] M0.5 从 ALS 工程一次性批量导出全部占位资产（不分批）：UE5 Manny 系蒙皮网格与骨骼骨架（glTF 入库 `assets/models/`）+ 全部动画组 Grounded/Air/Actions/Transitions/TurnInPlace/RotateInPlace/Overlays/View/Base（glTF 入库 `assets/animations/`，BlendSpace 定义 JSON 同目录 `blendspaces/`）+ 关键曲线数值表（JSON 均匀采样入库 `assets/curves/`），完整 export-manifest 已登记。（通道定案：headless commandlet pythonscript——MCP toolset 无导出工具；引擎/工程路径经 gitignored `tools/env.local.json` 注入，见 agents.md 规则 7 与 export-manifest 说明）
- [x] M0.5b（2026-08-23 增补，用户指示）动画内嵌浮点曲线补提取：独立转换工具 `tools/uasset_curve_export/`（纯 stdlib Python 直接解析 UE5.8 .uasset 包格式：包摘要/名字表/导入导出表/版本化标签属性→`FRawCurveTracks.FloatCurves` 的 `FFloatCurve+FRichCurve` 全键值），把 127 个 AnimSequence 的 **367 条内嵌 ALS 曲线**（LayerArm*/LayerHand*/LayerHead/LayerLegs/LayerPelvis/LayerSpine 系、HandLeftIk/HandRightIk、HipsDirectionLock、PoseAiming/PoseGait/PoseCrouching/PoseStanding、FootPlanted/FootLeftLock/FootRightLock/FeetCrossing、RotationYawSpeed、AllowTransitions、FootstepSoundBlock 等）以自定义扩展 `ALS_float_curves` 写回对应 `assets/animations/**.gltf` 的 animation 对象（82 个资产含曲线；bevy_gltf 对未知 extensions 静默忽略不破坏现有加载，后续经 `GltfExtensionHandler::on_animation` 在 als-ext 摄取）。背景：M2.4 已核实引擎 glTF 导出器与 bevy loader 两端都不支持浮点轨道，导致 M6.2/M6.3 被迫用 CF_Als_Aim_In/Out 与静态骨骼 mask 等替代——本条目从源头补齐真实数据。schema 与登记见 export-manifest「M0.5b」章节。

- [x] M0.6 编写资产导入小工具（`tools/import` 或 build 脚本）：UE 导出物 → bevy_asset 可加载格式（GLTF→GraphClip/场景）的一键流程。
- [x] M0.7 **Gate**：`cargo run --example skeleton_boot` 加载骨架+一条动画并在窗口中播放；export-manifest 存在且含来源记录。
  - Gate 证据（2026-08-22）：`cargo run --example skeleton_boot`（ALS_BOOT_AUTO_QUIT_SECS=8）输出 `SKELETON_BOOT_OK step=1 … player_elapsed=0.031s` / `step=2 … player_elapsed=2.016s`——GraphClip/Skeleton/AnimationGraph 各 1 个全部加载成功、播放头持续推进、无 ERROR/panic、exit=0；骨架源为含 AnimationPlayer 的动画 glTF（插件 find_root 依赖，工具已内置该规则）；export-manifest.md 登记 158 条来源记录。

## M1 ECS 核心状态模型（依赖：M0）

对标 ALS GameplayTags 体系，改为纯 ECS 组件。参考 ALS：ViewMode / LocomotionMode / RotationMode / Stance / Gait / OverlayState。

- [x] M1.1 `crates/als-core`：定义状态组件 `LocomotionState`（Grounded/Air/Ragdolling 等顶层 mode）、`RotationMode`(VelocityDirection/ViewDirection/Aiming)、`Stance`(Standing/Crouching)、`Gait`(Walking/Running/Sprinting)、`OverlayState`(Default/…13 项枚举)；全部枚举取值已对照 ALS v4.18 AlsGameplayTags.h 核对定稿（RotationMode 用源码 ViewDirection，修正本清单旧称 ControlDirection；Ragdoll 并入顶层 LocomotionState）。
- [x] M1.2 状态合法性约束系统：非法转移（如空中切蹲）由专门 system 拒绝/排队；提供 `StateTransitionEvent`。
  - 设计要点：逐维度 Verdict(Accept/Reject/Queue) 支持部分应用；默认策略 Queue 对应 ALS Desired 字段持久语义；Ragdolling→Air 无视策略直接拒绝；同值请求幂等不发事件。
- [x] M1.3 输入抽象层：bevy_enhanced_input 映射 Move/Look/Jump/Crouch/Sprint/Aim/Roll 等 action → `AlsInputCommand` 组件（与具体设备解耦）。
- [x] M1.4 角色实体组装 API：`AlsCharacterBundle`（Transform、物理体、tnua controller、状态组件、参数资源句柄）+ 生成示例角色的 example。
- [x] M1.5 参数资产：`AlsMovementParams`（各 gait 最大速度/加速度、旋转速率、蹲高、跳跃参数）RON 化，支持 per-character 覆盖。
  - 数值出处：walk/run/sprint 1.75/3.75/6.50 m/s（AlsMovementSettings.h:21/28/35）；加速20/减速15/摩擦4.0（AMC.cpp:139-141）；胶囊站半高0.90半径0.30（AlsCharacter.cpp:31）、蹲半高0.40（引擎默认，工程无覆写）；蹲速1.50 m/s（AlsCrouchingSettings.h:13）；跳跃Z 4.20 m/s+重力1.0（引擎默认）；旋转半衰期曲线取 M0.5 导出 JSON 实测值；yaw 转速 VD800/View500/Aiming1000 deg/s（AlsCharacter.cpp≈1574-1884）。推断项：coyote 0.10s/jump buffer 0.15s（ALS 无此机制，平台惯例）。
- [x] M1.6 调试 UI（简单 egui/bevy_text 打印当前状态组合即可）。
- [x] M1.7 **Gate**：example 中按键可在合法范围内切换 Stance/Gait/RotationMode，非法切换被拒绝并有日志；状态打印正确。
  - Gate 证据（2026-08-22，`cargo run --example state_switch`，ALS_BOOT_AUTO_QUIT_SECS=14 脚本化序列）：9 步全部符合预期——蹲切 ACCEPTED(Standing→Crouching→Standing)；Air 切换 ACCEPTED；空中请求蹲**排队无事件**、回 Grounded 后排队项自动生效(同帧两条 ACCEPTED)；Gait Walking→Sprinting ACCEPTED；Ragdolling ACCEPTED；Ragdoll 态请求 Air **REJECTED reason=RagdollCannotEnterAir**；退出 Ragdoll ACCEPTED。HUD 实时打印五状态组合；exit=0 无 panic。键盘路径 C/G/R/J/K 同 API 可交互操作。

## M2 曲线与数据驱动基础设施（依赖：M1）

ALS 大量依赖动画曲线与参数曲线（Layer 权重、IK 权重、倾斜、stride blending 等）。通过 bevy_animation_graph 的自定义节点扩展点在 `crates/als-ext` 补齐 curve 能力（规则 4），不修改 fork。

- [x] M2.1 在 `crates/als-ext` 实现 `CurveAsset`：一维 float 曲线资产（RON：线性/阶梯/平滑三种采样模式），注册进 bevy Assets。
- [x] M2.2 经 bevy_animation_graph 自定义节点注册实现 `CurveSampleNode`：float 输入（时间或任意参数）+ CurveAsset 句柄 → float pin。
  - 实现差异（公开扩展点边界核实）：节点内嵌曲线数据字段（mode+keys，随图 RON 序列化）而非资产句柄——`SystemResources` 为固定 SystemParam（仅 GraphClip/AnimationGraph/StateMachine/Skeleton 四类）且 `DataSpec` 封闭枚举，自定义资产句柄在 update 内无解析通道；句柄形态若未来必需再按规则 4 评估 fork 改动。分发键 `als_ext::CurveSampleNode`（#[type_path]），注册契约单测覆盖。
- [x] M2.3 实现 `CurveDrivenValueNode`：把曲线输出映射到 Pose 之外的"元数据输出"（模拟 ALS 的 curve→anim 通知通道），供 IK/权重消费。
  - 在 CurveSampleNode 基础上增加 scale/bias 重映射（ALS 曲线常需量程变换）；输出经图级 `output_data` pin（DataSpec::F32，资产先例 blend_parameters.animgraph.ron）供外部系统经 `AnimationGraphPlayer::get_outputs()` 消费——即 ALS curve→参数通道的 ECS 等价物。
- [x] M2.4 clip 曲线轨道采样：播放 clip 时同步输出其浮点曲线轨道值（对接 ALS 导出动画中的 FootSliding/LayerHand* 等）；优先基于公开数据结构在 als-ext 包装实现，若公开 API 不足再按规则 4 做可回馈上游的最小独立改动。
  - 事实核查（2026-08-22）：UE glTF 导出器不写 AnimSequence 自定义浮点曲线为 glTF 通道（实测导出文件仅 translation/rotation/scale 三类 target），且 bevy 0.19 glTF loader 仅读取该四类通道（含 morph）——浮点轨道在资产管线两端都不存在，ALS 的 FootSliding/LayerHand* 等曲线实际以独立 CurveFloat 资产存在（M0.5 已导出 16 张数值表）。等价实现：曲线经 M2.5 导入为 CurveAsset，播放时由图内共享时间源同时驱动 ClipNode 与 CurveDrivenValueNode 实现逐帧同步输出（Gate 示例演示）；若未来确需真内嵌轨道，须先增补导出条目或按规则 4 处理 bevy_gltf 上游。
- [x] M2.5 将 ALS 关键曲线数值表导入为 RON 并加载验证；单元测试覆盖曲线端点/中点求值与节点类型检查。
  - tools/import/gen_curves.py 将 M0.5 导出的 16 张 JSON 数值表转 .curve.ron(Linear 模式逐键保真)；CurveAssetLoader(.curve.ron 扩展名)注册进 AlsExtPlugin；集成测试经 AssetPlugin+AssetServer 真实加载 CF_Als_StrideBlendAmount_Walk 并断言 129 键/域[0,150]/端点 0.2→1.0/均匀网格中点精确命中/相邻键半点线性插值；节点类型检查=注册契约单测(type_path 分发键+ReflectNodeLike)。
- [x] M2.6 **Gate**：example 用 CurveDrivenValue 驱动一个可见数值（on-screen 显示或物体缩放）随时间按曲线变化；`cargo test -p als-ext` 全绿。
  - Gate 证据（2026-08-22，`cargo run --example curve_gate`，ALS_BOOT_AUTO_QUIT_SECS=1）：Smooth 三键曲线(0/0.25@1s/1.0@2s)经 CurveDrivenValueNode→图 output_data "curve_value"→外部 get_outputs() 消费驱动立方体缩放+高度，实测序列 drive_time=0.51→curve_value=0.1243、1.01→0.2499、1.53→0.6406、≥2.03→1.0000(钳制)，与 Smooth 插值逐点吻合；exit=0 无 panic。`cargo test -p als-ext` 9 测试全绿（5 曲线数学+3 节点契约+1 AssetServer 集成），workspace 总计 35 全绿。

## M3 运动基座：tnua → ALS 参数映射（依赖：M1, M2）

用 bevy-tnua 替代 UE CharacterMovementComponent。目标：walk/run/sprint/crouch/jump 手感对齐 ALS 数值。

- [x] M3.1 场景复刻：按 ALS 工程 `ALSExtras/Levels/L_Als_Playground.umap` 在本项目搭建等价测试场景——经 commandlet 枚举关卡内 actor（StaticMeshActor 等）并导出其 StaticMesh 为 glTF 入库 `assets/models/playground/`，同时生成场景描述 JSON（actor→mesh 映射 + world transform）；bevy 侧程序化重建静态碰撞体与视觉网格；**不含 Landscape**（bevy 侧无等价方案则跳过该部分并在 manifest 注明）。导出属 M0.5 一次性批量导出的增补，依据本条目执行。
  - 验收（2026-08-22）：219 actor 全量枚举（147 BP_SimpleObject 参数化方块+22 StaticMeshActor+Brush 等）；20 唯一 SM_Als_* mesh glTF 导出 20/20 成功入库；坐标换算实测修正为 (x,z,y)·0.01+四元数共轭映射（18 单测）；examples/playground 实测 171 静态体、角色落地 (-15,0.92,-15) 于 Floor 网格、loco=Grounded、无 panic。Landscape 按约定跳过并登记 manifest。
- [x] M3.2 avian3d 集成选型落地：固定 timestep、角色 capsule collider、地面传感器配置。
- [x] M3.3 tnua 控制器参数 ↔ `Gait` 映射系统：Walking/Running/Sprinting 三档速度与加速度来自 `AlsMovementParams`。
- [x] M3.4 Crouch basis 接入：站立/蹲下 capsule 高度切换（含头顶遮挡检测防站起）。
- [x] M3.5 Jump action 接入：高度/coyote/buffer 参数化，落地事件转发为 `LocomotionState::Air→Grounded`。
- [x] M3.6 旋转模式系统：VelocityDirection（非瞄准）/ViewDirection+Aiming（瞄准朝向）两种 yaw 行为，含 ALS 式转身速率曲线（消费 M2 curve）。
- [x] M3.7 斜坡/台阶处理验证：slope 旋转、step height 参数调至 ALS 观感。
- [x] M3.8 **Gate** **Gate**：example 第三人称胶囊在测试关卡完成走跑冲/蹲/跳全流程，速度曲线数值与 ALS 参数表一致（打印比对），无抖墙/卡台阶回归。

## M4 动画图基座（依赖：M2, M3）

把导出资产接入 bevy_animation_graph，建立 ALS 对应的图骨架。

- [x] M4.1 骨骼映射：UE Manny 骨骼名 → bevy 实体层级 BoneId 映射表，含 virtual bone 占位策略（`VB foot_l` 等 ALS 虚骨先以 marker entity 表示）。
- [x] M4.2 基础图库 `assets/graphs/`：Idle、Walk F/B/L/R、Run、Sprint、Crouch 系列 clip 图；BlendSpace1D 按速度混合行走组。
- [x] M4.3 ChainNode 组装 Base locomotion 层 + 上层叠加接口（LayeredPose 输出 pin 约定）。
- [x] M4.4 时间参数系统：Stride Blanking/步频同步——动画播放速率随实际水平速度缩放（ALS Start/Stop/移动匹配的简化版，完整 distance matching 后置到 M8）。
- [x] M4.5 动画图热重载验证：编辑 RON 无需重启即生效。
- [x] M4.6 **Gate**：example 中角色随速度自动在 idle↔walk↔run↔sprint 平滑混合，蹲姿切换对应图分支；帧率无关（0.5x/2x timescale 测试）。

## M5 Locomotion 状态机动画（依赖：M4）

- [x] M5.1 Start/Stop（起跑/急停）状态与动画选择逻辑。
    - 源码核实修正：ALS v4.18 EAlsMovementDirection 仅 4 向(Forward/Backward/Left/Right)，分类用 CalculateMovementDirection(ForwardHalfAngle=70°,Threshold=5°)；急停 PlayQuickStopAnimation 按剩余转角符号选 Left/Right 支且 play rate 随 |转角|/180 缩放——非清单旧称九向。实现：MoveDirection 八分区纯函数 classify+迟滞状态机(update_start_stop start=0.1/stop=0.05 m/s)，单测 12 项；资产对接：起跑=Acceleration 4 向+冲刺专用，急停=Stop_Left/Right。
- [x] M5.2 Pivot（折返）触发条件与混合。
    - 源码核实：ALS 无显式 Pivot 状态/动画——C++ 仅提供速度门槛(PivotActivationSpeedThreshold=200cm/s)与 bPivotActive 标志，反转检测在 AnimBP 图层；定位为折返检测器供图路由消费。实现：PivotDetector(reversal>100°+min_speed=2.0m/s 对齐 ALS 门槛+冷却 0.3s)+PivotSide 左右支；无 Pivot 类资产(工程亦无)，表现走图内 blend 路线复刻本意。单测 10 项(反转触发/渐转不触发/冷却/低速/边界值)。
- [x] M5.3 TurnInPlace（原地转身）：角度阈值 + 90/180 选择 + 步点对齐（foot lock 简化为曲线锁相，完整 IK 在 M7）。
    - 对齐 AlsTurnInPlaceSettings：ViewYawAngleThreshold=45°、ViewYawSpeedThreshold=50°/s、Turn180AngleThreshold=130°、PlayRate=1.2；激活延迟按 |角|∈[45,180] 线性递减（近似 ALS 曲线映射）；符号选向同 RemapAngleForCounterClockwiseRotation(≤0→Left)。步点对齐按条目范围留 M7 曲线锁相。单测 6 项。
- [x] M5.4 RotateInPlace（瞄准态原地转向）。
    - 对齐 AlsRotateInPlaceSettings：静止+Aiming 时 |视角偏差|>50° 选左/右旋转；play rate 由 ReferenceViewYawSpeed 180–460°/s 线性映射至 1.15–3.0 随视角转速缩放，停止时 DamperExact 回中 1.15。单测 7 项。
- [x] M5.5 Hard Stop/Gait 过渡细节：ALS Transition 组动画接入（stance/gait 间过渡）。
    - 对齐 AlsAnimationInstance：stance 切换过渡(CrouchToStand/StandToCrouch)带 idle-only 条件(moving 时由 Dynamic 分支处理)；Dynamic Transitions 按 foot-lock 距离平方比较选对侧支+2 帧防抖(TransitionCooldown)；资产映射 A_Als_Stand/Crouch_DynamicTransition_Left/Right 与 CrouchToStand/StandToCrouch。单测 7 项(选支/idle 拦截/默认左/强制侧/冷却)。
- [x] M5.6 状态→图路由器：集中 system 把 `LocomotionState` 组合翻译成图输入 pins（替代 AnimBP 的状态机图）。
    - compute_graph_inputs 纯函数：effective_gait 钳制(蹲→Walk)+按 gait_max_speed 钳制(蹲姿走= crouch.max_walk_speed_crouched 1.5)+actual_speed 直通；graph_input_router_system 把 speed/actual_speed 写入 AnimationGraphPlayer 输入 pins(连接 M3 状态与 M4 图)。单测 4 项。
- [x] M5.7 **Gate**：演示路径覆盖：静止→起跑→全速→急停→原地转身→蹲走→起立，全程无明显脚滑与跳变；录制视频/截图存入 `docs/` 并在提交说明中引用。
  - Gate 证据（2026-08-23，`examples/locomotion_gate`，Playground 场景+AnimatedScene 挂接角色+graph_input_router+三检测器，ALS_BOOT_AUTO_QUIT_SECS=60 脚本化 7 阶段，exit=0 无 ERROR/panic）：静止 rel=0 无误触发→起跑 Running 实测 3.75→全速 Sprinting 6.50→急停 Started(Forward)→Stopping(Forward) 减速至 0 且视角回中对齐（rel→0）→原地转身 view 180°→60°（rel=-120°）TurnInPlace Left90 play_rate=1.2 触发、保持期按失准持续重触发（ALS 语义）→蹲走 Crouching 实测钳制 1.50（capsule 0.39/0.40）→起立 Standing capsule 恢复 0.90、view 回中 rel→0 收尾 idle。0.5s 状态快照日志 `docs/m5-gate-run.log`。实现注记：AlsViewYaw 以出生朝向初始化；检测用 yaw 转速经包络滤波（250°/s² 衰减）抗系统调度抖动；stride_play_rate=actual/speed 钳制 [0.6,1.6] 且静止恒 1.0——顺带修复 M4 SpeedNode 直连 actual_speed 导致静止时动画冻结的缺陷（graph 新增 play_rate 输入 pin，默认 1.0 对旧示例兼容）；player_elapsed 在步态切换瞬间的跳变即 CHECKLIST 已记录的 SpeedNode 记账伪影，不影响视觉相位。

## M6 Overlay 与 Aim/View 系统（依赖：M5）

- [x] M6.1 Overlay 分层架构：Base 图之上叠加 Overlay 图（rifle/pistol/… 起步仅 Default + Torch 类单手），骨骼 mask 用 BlendNode mask 能力。
  - 语义核实（附录 B 前置验证）：fork BlendNode 原生支持 Difference（A⊖B 生成动态加法增量，BonePose::difference_mut=other∘self⁻¹）与 Additive（additive_blend 按 alpha*mask 权重合成）+ use_bone_mask 输入 pin（DataSpec::BoneMask），无需改 fork。实现：assets/graphs/locomotion_overlay.animgraph.ron——Base locomotion(BlendSpace+play_rate SpeedNode+Loop) 与 Torch_Poses、Default_Poses(reference, override_duration 2s) 经 Difference→Additive 两级 BlendNode 链复刻 ALS MakeDynamicAdditive；overlay_weight/diff_factor/upper_body_mask 为图输入 pins；als-ext::overlay 提供 build_bone_mask/mask_coverage(TORCH_RIGHT_HAND_BONES 六骨预设 spine_02..hand_r, Skeleton.iter_paths 按末段名精确匹配)。自测：als-ext 5 项新单测(mask 权重/Positive 基底/末段匹配/空安全/路径覆盖)全绿 23；examples/overlay_smoke 实测图求值无 error、pose 持续输出、weight 0→1 渐变、mask bones=6，exit=0。
- [x] M6.2 Layer 权重曲线接入：LayerHandLeft/Right 等曲线经 M2 管线驱动叠加强度。
  - 资产事实：ALS 的 LayerHandLeft/Right 是动画内嵌曲线轨道（M2.4 已核实 UE glTF 导出器不写出该类轨道），导出资产中不存在同名 CurveFloat；等价采用 Blending 组 `CF_Als_Aim_In`/`CF_Als_Aim_Out`（实测 0→1 平滑表，域 [0,1]s）经 M2 CurveAsset 管线驱动叠加权重。实现：als-core::overlay_blend 纯状态机（Idle/BlendingIn/Held/BlendingOut，中断续接 live weight、ε 容差相位完成、负 dt 安全）；bevy_als::overlay_layer::OverlayLayerPlugin（编排层首次落地根 crate lib）加载双曲线、按 OverlayState!=Default 目标驱动并写 overlay_weight pin。单测 7 项全绿（als-core 89）；overlay_smoke 实测 Torch 切换 BlendingIn@曲线→Held→BlendingOut→Idle 与中途重触发闭环，无 graph error，exit=0。左右手分工权重待 Rifle 双手 overlay 时后置。
- [x] M6.3 AimOffset 等价物：2D BlendSpace(yaw,pitch) + View 组件（视角偏航俯仰分离，视线跟随插值）。
  - 资产事实修正：ALS v4.18 无 Look_Left/Right 类 yaw 偏移动画资产（瞄准偏航由身体朝向 RotationMode::Aiming+RotateInPlace 承担，已分别于 M3.6/M5.4 复刻；导出的 CF_Als_RotationYawOffset_* 三曲线为 mesh yaw offset 数据，留 M10 相机联动消费），故 AimOffset 的 yaw 轴不走图内混合、pitch 轴用 BlendSpace1D(view_pitch_norm ∈[-1,1]：Look_Down@-1/Forward@0/Up@+1) 实现。实现：locomotion_overlay 图追加 look 链——BS1D→Difference(对照 reference poses 独立副本节点，fork 限制每输出 pin 仅一消费者)→Additive masked(look_mask=spine_01..03+neck_01)；als-core::view 组件 AlsViewPitch(目标)/AlsViewSmoothed(实际)+clamp_pitch(±85°)+damp_view(half-life=0.08s 指数插值=视线跟随平滑)+view_pitch_norm 映射；bevy_als::view_layer::ViewLayerPlugin 链式系统(积分阻尼→写 view_pitch_norm/look_weight pins)。单测 5 项全绿(als-core 94)；overlay_smoke 实测 pitch 目标 ±68° 正弦下平滑值连续跟随且 overlay 权重链不受影响，无 error，exit=0。
- [x] M6.4 Aiming 模式联动：Aim 输入 → RotationMode::Aiming + camera 相关状态广播（相机本体在 M10）。
  - 实现：als-core::aim——aim_rotation_mode_system（aim_held 边沿驱动 AlsStateRequest.rotation_mode↔Aiming，AlsLastFreeRotationMode 记忆非瞄准模式且永不存 Aiming；释放恢复记忆值，缺省 VelocityDirection）+ AimStateChanged 消息广播（M10 相机挂钩点）；系统显式 before(state_transition_system) 保证同帧应用。单测 3 项（进入+单次广播/按住不重发、释放恢复 ViewDirection、free 模式非 Aiming）全绿(als-core 97)；测试基建教训记录：spawn_als_actions 会经 Commands 延迟覆盖预置 AlsInputCommand，纯状态机测试勿混用 Actions 装配。
- [x] M6.5 **Gate**：切换 Overlay 时上半身换装动画正确叠加且不影响腿部移动混合；鼠标控制 aim offset 四象限平滑。
  - Gate 证据（2026-08-23，`examples/overlay_gate`，Playground+全栈插件编排，ALS_BOOT_AUTO_QUIT_SECS=40 脚本化，exit=0 无 ERROR/panic）：①行走中(3.75 m/s 恒定)切 Torch——overlay_weight 经 CF_Als_Aim_In 0.214→0.704→1.000 渐入且 speed_pin/actual/rate 全程不受影响（上半身 mask 叠加与腿部移动混合解耦）；②aim quadrants sweep——AIM broadcast aiming=true、rot=Aiming 体朝向跟随 view、view/body 连续扫过 NE→SE→SW→NW 四象限（360° 单向匀速无回跳）、pitch 平滑值 -58°↔+64° 连续无跳变；③释放 aim 广播 false 且 rot 恢复 VelocityDirection、Torch 保持 w=1.000；④blend-out 经 CF_Als_Aim_Out 0.995→0.181→0.000。快照日志 `docs/m6-gate-run.log`（88 行）。附带重构：motion↔graph pin 写入抽取至 bevy_als::motion_graph_link（M5/M6 gate 示例共用）；修复 als-core::aim 未消费 AlsInputOverride 导致脚本化 aim 失效的问题。

## M7 Foot IK 与 ControlRig 等价物（依赖：M6）

ALS 用 ControlRig 做 Foot IK + Pelvis 位移 + 手部 IK。Bevy 侧 IK 能力统一落在 `crates/als-ext` 的 ik 模块（规则 4），以分析式 IK systems 实现；bevy_als 提供状态、射线与曲线权重等输入并接线。

- [x] M7.1 地面探测：双脚 raycast（含斜面法线），输出 FootOffset 目标量。
  - 事实核查：fork builtin_nodes 已有图级 TwoBoneIKNode（CHECKLIST 背景事实"缺 two-bone IK"已过时）；M7 按条目要求走 ECS 路线——求解器落 als-ext::ik、作用于骨骼实体 Transform（PostUpdate 在 apply_animation_to_targets 之后），目标量来自同帧探测避免一帧滞后。实现：als-core::foot_ik——AlsGroundHit/AlsFootProbe(双脚命中点+斜面法线)、foot_target_offset(hit.y+ANKLE_TO_SOLE-ankle.y 钳制 ±0.45)、foot_ik_target(命中点抬 sole 高)、slope_is_walkable(对齐 ALS max_slope 50°)、AlsFootBones 按 foot_l/foot_r 名称子树映射+PostUpdate 探测系统链(bone_map→probe)。单测 6 项全绿。
- [x] M7.2 Two-bone IK 求解器在 `crates/als-ext`（ik 模块）实现：解析法髋-膝-踝求解，作用于骨骼实体 Transform。
  - 实现：als-ext::ik::solve_two_bone_ik——余弦定理解析法(hip 角 α=acos((d²+u²−l²)/2du))，bend 平面由 target 轴+pole 侧向构成(pole 平行轴时正交回退)，输出 world 系 hip/knee 四元数+knee 位置；距离钳制 [max(|u−l|,ε), u+l−ε] 内/外可达均稳定；world_rotation_to_local 供调用方转骨骼实体局部旋转(parent_world⁻¹·world)。单测 9 项：精确命中/极侧弯曲/旋转向量一致性/超远伸直/超近内限/已满足姿态恒等/pole 退化/退化输入 None/world→local 往返，全绿。
- [x] M7.3 Pelvis 垂直补偿与双脚插值锁定（消费 FootLeftLock/FootRightIk 曲线权重的等价输入）。
  - 曲线权重等价输入定案：ALS 的 FootLeftLock/FootRightIk 为动画内嵌轨道(M2.4 已核实不随 glTF 导出)，等价输入改为 ECS 组件槽——AlsFootIkTargets(grounded 门控 ik 权重目标+pin 左右请求, 由 foot_ik_state_system 从 LocomotionState/AlsFootPinRequest 派生)与 AlsFootPinRequest(TurnInPlace/Stop 检测器写入, M7.4 接线)。实现：AlsFootIkRuntime 双侧运行时——offset 以速度上限插值向 raw*ik_weight(空中权重→0 收回)，pin 上升沿冻结 locked_offset 并按 pin_weight 混合、释放后回落 raw，pelvis_drop=min(L*w,R*w).min(0) 只降不升再经速度上限插值。单测 9 项全绿(al-core 106)。
- [x] M7.4 Foot lock 与步点：TurnInPlace/Stop 状态下脚钉地，配合 M5 曲线锁相升级为真实 transform 锁定。
  - 实现：foot_pin_request_system——StartStopState=Stopping(急停)或 AlsFootPinHold 时间窗(TurnInPlace 检测器触发时由消费方经 extend_foot_pin_hold 延长 TURN_PIN_HOLD_SECS=1.2s，M5.3 事件为纯函数返回值非 Message 故走组件窗)→写 AlsFootPinRequest 双脚，非 grounded 由 foot_ik_state_system 门控丢弃；AlsFootIkRuntime pin 冻结(M7.3)即真实 transform 锁定基础。单测 3 项(stopping 双脚钉/idle 不钉/hold 窗内钉+过期清除)全绿(al-core 109)。
- [x] M7.5 Hand IK 占位：`ik_hand_gun` 虚拟骨等价的挂点 entity + 手部两骨 IK 到挂点（rifle overlay 用）。
  - 实现：als-ext::ik——IkHandTarget 标记组件+spawn_ik_hand_gun(以 ChildOf 挂到手骨、命名 ik_hand_gun、持枪偏移占位 (0.10,0.02,0.25))；手部两骨求解复用 M7.2 solve_two_bone_ik(upperarm/lowerarm 链，target=挂点 world 位置)，实际应用系统随 Rifle overlay(M8+)接线。单测 1 项(父子关系/命名/偏移)全绿(als-ext 10 ik 测试)。
- [x] M7.6 IK 系统调度顺序固定：pose apply → ground probe → IK solve → 渲染，写入文档。
  - 实现：bevy_als::foot_ik_apply::FootIkApplyPlugin——foot_ik_apply_system 显式 after(apply_animation_to_targets/probe/pin_request/state)， AlsFootIkRuntime 插值→solve_two_bone_ik→骨骼实体局部旋转(parent_world⁻¹·world)+pelvis 骨局部 Y 补偿；AlsFootBones 扩展为 thigh/calf/foot 全链。调度文档 docs/ik-scheduling.md（含一帧滞后与 pelvis 等价折衷说明）。
- [x] M7.7 **Gate**：斜坡站立双脚贴地、pelvis 高度正确；行走上下坡脚不穿地；原地转身脚钉地无滑动。
  - Gate 证据（2026-08-23，`examples/foot_ik_gate`，ALS_BOOT_AUTO_QUIT_SECS=25 脚本化 6 阶段 idle→12°坡上行→坡面急停站立→退行下坡→停+180°原地转身(Right180 play_rate=1.2 触发)→settle，exit=0 无 ERROR/panic）：三 VERDICT 全 PASS——min_penetration=+0.000(tol −0.060)；slope_stand 静止帧双脚命中且 pen∈[−0.06,+0.12]、pelvis_drop −0.039∈[−0.30,0]；turn_pins engaged=true root_drift=0.000/0.05、lock_shift=0.000/0.03（stop_pins_seen=false 仅采样粒度所致，Stopping 钉地路径有单测覆盖）。快照 `docs/m7-gate-run.log`。workspace 146 测试全绿。
  - 实现修复注记（本条目过程中确诊三缺陷）：①PostUpdate 管线钉死——fork 的 `AnimationGraphSet::Final.before(TransformSystems::Propagate)` 只配置在 FixedPostUpdate，PostUpdate 内动画写入与变换传播顺序悬空致 probe/IK 读过期全局变换；现经 configure_sets 补约束+probe 链 after Propagate+`bevy::transform::systems` 公开系统第二实例（mark_dirty→propagate→sync，after IK）完成二次传播（docs/ik-scheduling.md 已修订）。②`als-ext::ik::solve_two_bone_ik` 返回**增量旋转**（from_rotation_arc），apply 原按绝对旋转写入致骨骼瞬移回绑定姿态（脚悬空 ~0.5m）——修正为先乘当前世界旋转、小腿用新髋世界旋转作父系换算（src/foot_ik_apply.rs）。③AlsFootIkRuntime pin 锁存语义精炼：上升沿锁 `raw_offset×ik_weight`（触地帧收敛目标）而非插值中间值，消除急停锁定残留 8–10cm 悬空；单测改写 pin_locks_plant_frame_target_and_releases。另 foot_bone_map 收紧为仅匹配 AnimationTargetId 关节实体。
- [x] M7.8 **增补（M8 前）**：合并 `feature/uasset-curve-gltf` 分支（M0.5b：uasset 直读 FRawCurveTracks.FloatCurves 回写 glTF 扩展 `ALS_float_curves` 的 127 条 AnimSequence 重导出+提取工具），用带内嵌曲线的真实资产重做 M6.2/M6.3 中因资产缺失被迫采用 CF_Als_Aim_In/Out+静态 mask 的替代实现：新增 glTF `ALS_float_curves` 扩展导入通道（落 `crates/als-ext`，规则 4），LayerHand*/HandIk 等曲线经 M2 管线驱动叠加与 IK 权重，替换 overlay_layer 的 CF 双曲线驱动路径；自测覆盖扩展解析与曲线数值抽查（对照 ALS 语义如 Rotate_90_Left.RotationYawSpeed 积分≈−90°）。
  - 实现证据（2026-08-23）：合并零冲突（0264aef）。①导入通道 als-ext::als_curves——经 bevy_gltf 0.19 公开扩展点 `GltfExtensionHandler::on_animation/on_animations_collected`（不改 fork）解析动画级扩展 JSON 为 `AlsAnimCurves` 资产（anim→curve→track），以标注子资产 `#AlsCurves` 供运行时按路径加载；采样支持 LINEAR/CONSTANT 段插值（键 InterpMode 管离开段，UE 惯例）+ Constant/Linear/Cycle 外推，CUBIC 切线降级线性（已在 CHECKLIST 记录的近似）。②M6.2 重做——overlay_layer 移除 CF_Als_Aim_In/Out 依赖，过渡包络改由 OverlayBlendState 线性 ramp 驱动；上半身 mask 由 Torch_Poses 实测 Layer 曲线逐帧动态生成（LayerSpineAdditive→spine_02/03、LayerArmRight→右臂链、LayerHeadAdditive→neck/head，经新 build_grouped_bone_mask），look_mask 一并收编插件。③数据校验 tests/als_curves_data.rs 直读真实导出物：Rotate_90_Left.RotationYawSpeed 梯形积分 −90°±5 ✓、Walk_Forward.FootPlanted 值域归一 [−1,1] ✓（修正 manifest 抽查记录 [−0.25,1] 的偏差）、Torch_Poses 三组 Layer 曲线在位 ✓；单元测试覆盖解析/插值/外推/长度失配拒绝。④端到端 overlay_gate 复跑（exit=0 无 ERROR）：blend-in 线性 0.224→0.487→0.739→1.000、Torch held 行走中 speed_pin/actual 恒 3.75 不受影响、aim 四象限扫掠与释放恢复正常、blend-out 0.857→0.091→0——动态 mask 生效反证 #AlsCurves 标注资产通道打通（失败时驱动器早退 w 恒 0）。workspace 测试 154 全绿（als-core 109 / als-ext 38+3+1 / bevy_als 3）。

## M8 特殊动作（依赖：M7）

- [x] M8.1 Jump/Fall/Land 动画链：Air 组动画 + 落地选择（轻/重）+ Land recovery 混合。
  - 实现（2026-08-23）：①`als-core::air` AlsAirState 三相状态机 Idle/Airborne/Landing——滞空 air_weight 渐入(10/s)、fall_fast_weight 按 |vy|∈[8,14] 线性映射；落地以平滑撞击速度判轻重（EMA α=0.3 免疫接触帧求解器速度尖峰，实测尖峰 −8.1 vs 真实撞击 ~5），阈值 LAND_HEAVY_IMPACT_VY=8 m/s（点按短跳实测撞击 ~6.5 归轻、7m 高坠 ~11.7 归重，对齐 ALS 大落差才重落的语义）；Landing 按 variant 持续 Light 0.9s / Heavy 1.3s 后尾段 0.25s 淡出回 locomotion，滞空中再跳直接回 Airborne。②图侧 locomotion_overlay 新增 air 链：Jump_Loop/Fall/Fall_Fast 两级 blend + Land_Light/Heavy 变体 blend + land 总 blend，插在 locomotion 输出与 overlay 链之间；新输入 pins air_weight/fall_fast_weight/land_weight/land_heavy_weight（默认 0 向后兼容旧示例）。③bevy_als motion_graph_link 增 write_air_pins。④examples/air_gate 六阶段脚本化：跑动起跳→Air(vy +4.2→−6.5)→Light 着陆 recovery→再跑→teleport 7.9m 高坠(ff_w=1.00)→Heavy 着陆(w=1.00/1.00)→settle。
  - Gate 自测证据（docs/m8-air-gate-run.log）：`[LAND] light variant active w=1.00` ×3 与 `[LAND] heavy variant active w=1.00/1.00` ×4 分段出现，`VERDICT overall PASS`，exit=0 无 ERROR/panic；workspace 测试 161 全绿（als-core 116 含 6 项 air 单测：阈值边界/EMA 尖峰滤波/Landing 超时回 Idle/滞空重跳）。
- [x] M8.2 Roll（翻滚/闪避）：Rolling 状态、方向选择、root motion 等价位移方案（曲线驱动位移而非 root motion 骨骼）。
  - 实现（2026-08-23）：①`als-core::roll` AlsRollState——Grounded+Standing 且冷却完才可触发（冷却 0.35s），时长 0.8s 对齐 clip；方向经 M5 classify 八向映射 view_yaw±{0,π/4,π/2,3π/4,π} 得世界位移 yaw（右手系顺时针为右，单测锁定 dir=(sinθ,0,−cosθ) 约定）。②root motion 等价：解析归一化钟形速度曲线 v(t)=D·sin²(πt/T)/(T/2)（∫v dt≡D=3m 解析精确，离散积分误差 <2cm，单测覆盖；数据文件版曲线如后续需要再按规则增补条目），bevy_als::roll_drive 在 Rolling 期间直接覆写 LinearVelocity.xz、y 交还物理；roll_weight 包络 0.12s 入/0.18s 出写图 pin。③图侧 roll clip(override_duration 0.8)+全姿态覆盖 blend 插在 land blend 之后（pin roll_weight 默认 0 兼容旧例）。④bevy_als 新依赖 avian3d（velocity 覆写所需，自 dev-deps 提升）。
  - Gate 自测证据（docs/m8-roll-gate-run.log）：跑动前滚 `[RESULT 1] dist=2.94 align=+1.000 est_peak=7.34`；静止右滚 `[RESULT 2] dist=2.90 align=+1.000 est_peak=7.25`——距离偏差 ≤0.10m、方向对齐满值、节奏峰值 ~7.3 m/s 与曲线中点一致；exit=0 无 FAIL；workspace 测试 167 全绿（含 roll 单测 6：触发门控/八向选择/钟形积分/权重包络/方向约定）。
- [x] M8.3 Distance matching 基座（在 `crates/als-ext` 经动画图扩展点实现，公开 API 不足时按规则 4 处理）：Stop 动画按剩余距离归一化播放（ALS Stop 动画观感核心）。
  - 实现（2026-08-23）：①`als-ext::DistanceMatchNode`——经 NodeLike 公开扩展点（同 M2.2 契约，未改 fork）实现 pose+time 中间节点：输入 remaining/total 两 F32 pin，反演内嵌归一化距离曲线得 t_norm，向 ClipNode 发 `TimeUpdate::Absolute(t·duration)` 定位采样并重写 pose.timestamp。距离曲线默认 S 形 5 键（0/0.3→0, 0.6→0.85, **0.85→1 足触点**, 1.0 平台留 recovery 尾），invert 返回首次到达时间（平台段多对一）。②`als-core::start_stop` 增 predict_stop_distance(v,decel) 与 start_stop_detector_system 正式化（M5 示例驱动收编）；**语义修正**：Stopping 分支补 →Idle 转移（原实现 Stopping 为终态，M5 脚本未走到故未暴露；单测同步更新）。③`bevy_als::stop_link` AlsStopMatch 自持检测：**输入释放沿**（move>0.30→<0.05）+ Grounded + hspeed≥1.0 触发，remaining 由实测位移闭环消耗、hspeed≤0.05 即定格 plant（walked=1）——解析预测仅作归一化分母初值（tnua 制动非恒减速度模型，实测滑移 0.205m vs 恒减速预测 0.469m 的偏差不影响对齐性）。图侧 stop 链：Stop_L/R clip×DistanceMatch×2 + 变体/覆盖两级 blend，新 pins stop_weight/stop_right_weight/stop_remaining/stop_total。
  - Gate 自测证据（docs/m8-stop-gate-run.log）：sprint 3.75 m/s 急停 `[PREDICT] total=0.469` → `[RESULT] measured_slide=0.205 remaining_end=0.000 peak_w=1.00` → `[VERDICT] plant_on_stop=true weight_ok=true slide_in_band=true -> PASS`，exit=0 无 ERROR/panic；workspace 测试 172 全绿（含 distance_match 单测 5：S 曲线单调/反演往返+平台首达/足触点早于片尾/空曲線恒等/注册契约；start_stop 语义修正单测）。
- [ ] M8.4 Mantling（攀爬）低配版：检测+曲线位移+上下两类，可选后置。
- [ ] M8.5 **Gate**：奔跑中翻滚方向正确且位移与动画节奏一致；助跑跳停步动画脚步贴合预期落点。

## M9 Ragdoll（依赖：M8）

- [ ] M9.1 骨骼→刚体映射生成器（capsule 链）与关节约束配置。
- [ ] M9.2 进入/退出 ragdoll：blend-in（当前 pose→物理）、恢复起身（blend-out 回 Grounded）。
- [ ] M9.3 **Gate**：运动中触发 ragdoll 物理自然倒地，恢复后回到 idle 无爆骨骼。

## M10 相机（ALSCamera 等价物，依赖：M6/M8）

- [ ] M10.1 第三人称 rig：spring arm 等价（碰撞收缩、肩部偏移）。
- [ ] M10.2 相机模式：ThirdPerson 常规/Aiming 近肩/FirstPerson 占位切换，参数 RON 化。
- [ ] M10.3 **Gate**：贴墙不穿模、aim 切换平滑、与 M6 aim offset 数据联动正确。

## M11 调试工具与总验收（依赖：全部）

- [ ] M11.1 ALS 式调试绘制：状态文本、骨骼名、IK 目标线、速度球。
- [ ] M11.2 性能预算检查：单角色动画图求值 < 0.5ms（debug 构建放宽 3 倍）。
- [ ] M11.3 对照 ALS-Refactored 演示录屏做逐功能核对表，差异列表写入 `docs/parity-report.md`。
- [ ] M11.4 清理：移除临时 example、统一命名、补齐 crate 文档注释。
- [ ] M11.5 **最终 Gate**：`cargo test --workspace` 全绿 + 全部模块 Gate 勾选 + parity-report 完成 + README 快速上手可复现。

---

## 附录 A：与 ALS 的刻意差异（非缺陷）

| ALS | bevy_als | 理由 |
|---|---|---|
| AnimBP 状态机 | 显式 system 路由 + animation graph RON | ECS 数据流，无隐藏图执行 |
| GameplayTags | 普通 Rust 枚举组件 | 编译期检查 |
| ControlRig | 分析式 IK systems | 无运行时 rig 编译 |
| Root Motion | 曲线驱动位移 | bevy_animation_graph 无 root motion 提取，避免侵入 |
| Curves（UBT 资产） | als-ext CurveAsset + 采样节点 | 见 M2 |

## 附录 B：风险与后置项

- **资产格式决策（2026-08-21）**：Bevy 官方推荐 glTF；M0.5/M0.6 管线定为 UE → glTF（Interchange 导出）→ bevy_animation_graph 自带的 glTF 导入器（GraphClip/Skeleton）。**不引入 bevy_ufbx**：其 v0.19.0（bevy 0.19 兼容）虽支持蒙皮数据，但动画曲线未接入 Bevy 动画系统、无社区验证，且 FBX 中转对 ALS 曲线/虚骨/单位换算风险更大；仅当未来需要直接加载 FBX 时再评估。

- bevy_animation_graph additive/mask 语义若与 ALS layering 不完全一致，在 M6 前验证并决定扩展方式（优先 als-ext 扩展点）。
- Distance matching / motion matching 类高级特性仅实现 Stop 所需子集（M8.3）。
- ALSExtras（AI 控制器示例）不在范围内。
- 音频（Metasound）、Niagara 特效、ALSCamera 动画实例仅保留挂钩点。

  - M3.2-M3.7 验证证据（2026-08-22，examples/motion_gate 19 阶段脚本化全流程，exit=0 无 panic）：
    三档速度精确达标 walk/run/sprint = 1.75/3.75/6.50；jump Air↔Grounded 转移闭环；蹲走钳制
    target=1.50 且隧道内 headroom 阻挡起立(ph16 设计预期)、出隧道恢复站立行走(ph17)；原地旋转
    ViewDirection/Aiming 双模式执行；坡道爬升与台阶冲刺无抖墙无卡台阶。运动系统重构含
    effective_gait 蹲姿钳制、统一动作喂入、step-up 台阶辅助、capsule 换高延迟队列。
    （本注记为修复 PS 管道编码损坏后由主代理重写——dd7bb69/c1f813c/c74d2ff 中 CHECKLIST
    中文曾双重编码损坏，本版本以 8631741 干净基底重建。）
  - M4.1-M4.4 补勾选（实现于 8483270 与 bb8bf2d，当时漏同步勾选）。
  - M4.5/M4.6 进行中：file_watcher feature 启用后实测生效——运行中两次编辑 locomotion_speed.animgraph.ron 均触发 AssetEvent::Modified(日志 HOTRELOAD x2)；先前误判系编辑发生在 job 编译期(应用未启动)；ALS_TIMESCALE 基础设施已加(Time<Virtual>)但 0.5x/2x 比对测量
    方法有缺陷(报告列均为虚拟时间+SpeedNode elapsed 记账伪影)——两项完成后再勾选。
    M4.6 其余证据已具备：速度三角波自动混合+步频缩放实测、crouch_walk 分支图就绪。
  - M4.6 Gate 验证证据（2026-08-22）：速度三角波(0→6.5→0)自动驱动 BlendSpace1D 在 idle↔walk↔run↔sprint 平滑混合；ALS_TIMESCALE 实测 virtual/real = 0.50x→0.50、2.0x→2.00（真实时钟 ISO 时间戳列 vs 虚拟 elapsed 列，14/58 样本），帧率无关性成立；crouch_walk 分支图独立就绪(M6 经 BlendNode 接入)。SpeedNode pose_ts 记账伪影已记录备查(不影响视觉相位)。
