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

- [ ] M3.1 avian3d 集成选型落地：固定 timestep、角色 capsule collider、地面传感器配置。
- [ ] M3.2 tnua 控制器参数 ↔ `Gait` 映射系统：Walking/Running/Sprinting 三档速度与加速度来自 `AlsMovementParams`。
- [ ] M3.3 Crouch basis 接入：站立/蹲下 capsule 高度切换（含头顶遮挡检测防站起）。
- [ ] M3.4 Jump action 接入：高度/coyote/buffer 参数化，落地事件转发为 `LocomotionState::Air→Grounded`。
- [ ] M3.5 旋转模式系统：VelocityDirection（非瞄准）/ControlDirection+Aiming（瞄准朝向）两种 yaw 行为，含 ALS 式转身速率曲线（消费 M2 curve）。
- [ ] M3.6 斜坡/台阶处理验证：slope 旋转、step height 参数调至 ALS 观感。
- [ ] M3.7 **Gate**：example 第三人称胶囊在测试关卡完成走跑冲/蹲/跳全流程，速度曲线数值与 ALS 参数表一致（打印比对），无抖墙/卡台阶回归。

## M4 动画图基座（依赖：M2, M3）

把导出资产接入 bevy_animation_graph，建立 ALS 对应的图骨架。

- [ ] M4.1 骨骼映射：UE Manny 骨骼名 → bevy 实体层级 BoneId 映射表，含 virtual bone 占位策略（`VB foot_l` 等 ALS 虚骨先以 marker entity 表示）。
- [ ] M4.2 基础图库 `assets/graphs/`：Idle、Walk F/B/L/R、Run、Sprint、Crouch 系列 clip 图；BlendSpace1D 按速度混合行走组。
- [ ] M4.3 ChainNode 组装 Base locomotion 层 + 上层叠加接口（LayeredPose 输出 pin 约定）。
- [ ] M4.4 时间参数系统：Stride Blanking/步频同步——动画播放速率随实际水平速度缩放（ALS Start/Stop/移动匹配的简化版，完整 distance matching 后置到 M8）。
- [ ] M4.5 动画图热重载验证：编辑 RON 无需重启即生效。
- [ ] M4.6 **Gate**：example 中角色随速度自动在 idle↔walk↔run↔sprint 平滑混合，蹲姿切换对应图分支；帧率无关（0.5x/2x timescale 测试）。

## M5 Locomotion 状态机动画（依赖：M4）

- [ ] M5.1 Start/Stop（起跑/急停）状态与动画选择逻辑（方向分类：前/后/左/左前/…九向）。
- [ ] M5.2 Pivot（折返）触发条件与混合。
- [ ] M5.3 TurnInPlace（原地转身）：角度阈值 + 90/180 选择 + 步点对齐（foot lock 简化为曲线锁相，完整 IK 在 M7）。
- [ ] M5.4 RotateInPlace（瞄准态原地转向）。
- [ ] M5.5 Hard Stop/Gait 过渡细节：ALS Transition 组动画接入（stance/gait 间过渡）。
- [ ] M5.6 状态→图路由器：集中 system 把 `LocomotionState` 组合翻译成图输入 pins（替代 AnimBP 的状态机图）。
- [ ] M5.7 **Gate**：演示路径覆盖：静止→起跑→全速→急停→原地转身→蹲走→起立，全程无明显脚滑与跳变；录制视频/截图存入 `docs/` 并在提交说明中引用。

## M6 Overlay 与 Aim/View 系统（依赖：M5）

- [ ] M6.1 Overlay 分层架构：Base 图之上叠加 Overlay 图（rifle/pistol/… 起步仅 Default + Torch 类单手），骨骼 mask 用 BlendNode mask 能力。
- [ ] M6.2 Layer 权重曲线接入：LayerHandLeft/Right 等曲线经 M2 管线驱动叠加强度。
- [ ] M6.3 AimOffset 等价物：2D BlendSpace(yaw,pitch) + View 组件（视角偏航俯仰分离，视线跟随插值）。
- [ ] M6.4 Aiming 模式联动：Aim 输入 → RotationMode::Aiming + camera 相关状态广播（相机本体在 M10）。
- [ ] M6.5 **Gate**：切换 Overlay 时上半身换装动画正确叠加且不影响腿部移动混合；鼠标控制 aim offset 四象限平滑。

## M7 Foot IK 与 ControlRig 等价物（依赖：M6）

ALS 用 ControlRig 做 Foot IK + Pelvis 位移 + 手部 IK。Bevy 侧 IK 能力统一落在 `crates/als-ext` 的 ik 模块（规则 4），以分析式 IK systems 实现；bevy_als 提供状态、射线与曲线权重等输入并接线。

- [ ] M7.1 地面探测：双脚 raycast（含斜面法线），输出 FootOffset 目标量。
- [ ] M7.2 Two-bone IK 求解器在 `crates/als-ext`（ik 模块）实现：解析法髋-膝-踝求解，作用于骨骼实体 Transform。
- [ ] M7.3 Pelvis 垂直补偿与双脚插值锁定（消费 FootLeftLock/FootRightIk 曲线权重的等价输入）。
- [ ] M7.4 Foot lock 与步点：TurnInPlace/Stop 状态下脚钉地，配合 M5 曲线锁相升级为真实 transform 锁定。
- [ ] M7.5 Hand IK 占位：`ik_hand_gun` 虚拟骨等价的挂点 entity + 手部两骨 IK 到挂点（rifle overlay 用）。
- [ ] M7.6 IK 系统调度顺序固定：pose apply → ground probe → IK solve → 渲染，写入文档。
- [ ] M7.7 **Gate**：斜坡站立双脚贴地、pelvis 高度正确；行走上下坡脚不穿地；原地转身脚钉地无滑动。

## M8 特殊动作（依赖：M7）

- [ ] M8.1 Jump/Fall/Land 动画链：Air 组动画 + 落地选择（轻/重）+ Land recovery 混合。
- [ ] M8.2 Roll（翻滚/闪避）：Rolling 状态、方向选择、root motion 等价位移方案（曲线驱动位移而非 root motion 骨骼）。
- [ ] M8.3 Distance matching 基座（在 `crates/als-ext` 经动画图扩展点实现，公开 API 不足时按规则 4 处理）：Stop 动画按剩余距离归一化播放（ALS Stop 动画观感核心）。
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
