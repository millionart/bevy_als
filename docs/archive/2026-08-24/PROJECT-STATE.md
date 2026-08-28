# bevy_als 项目状态归档（2026-08-24）

> 新会话恢复上下文入口：读完本文件即可继续工作，无需重读全文清单。
> M0–M11 已完成并整体归档于本目录：完整逐字快照 `CHECKLIST-M0-M11.md`、门禁日志 `logs/`、差异对照 `parity-report.md`；
> 权威执行清单为根目录 `CHECKLIST.md`（现只承载进行中的工作，如 BUGFIX 清单）；执行纪律见根 `agents.md`。

## 1. 项目目标（一句话）

以 Bevy ECS 复刻 [ALS-Refactored](https://github.com/Sixze/ALS-Refactored)（UE 5.8, v4.18）高级运动系统：
基座 bevy_animation_graph 0.11.0（替代 AnimBP）+ bevy-tnua 0.32.0（替代 CharacterMovement/KCC），
插件能力扩展一律经公开扩展点落在本仓库 `crates/als-ext`；不做网络同步、不做 AI、音频特效只留挂钩点。
刻意不移植 UE 技术债：状态用纯 ECS 枚举组件，AnimBP 状态机改为显式 system 路由 + 动画图 RON。

## 2. 当前状态：M0–M11 全部完成（2026-08-24）

各模块 Gate 一行结论（详细证据正文在快照 `CHECKLIST-M0-M11.md`；运行日志在本目录 `logs/`）：

| Gate | 日期 | 结论 | 日志 |
|---|---|---|---|
| M0.7 skeleton_boot | 08-22 | 骨架+动画加载播放 OK exit=0；manifest 登记 158 条来源记录 | 无独立日志（正文在快照） |
| M1.7 state_switch | 08-22 | 九步序列全符合预期（蹲切/Air 排队生效/Ragdoll 拒绝 Air）exit=0 | 无独立日志（正文在快照） |
| M2.6 curve_gate | 08-22 | 曲线驱动数值逐点吻合；als-ext 9 / workspace 35 测试绿 | 无独立日志（正文在快照） |
| M3.8 motion_gate | 08-22 | 19 阶段三档速度精确达标、蹲走钳制、坡道台阶无回归 exit=0 | 无独立日志（快照文末注记区） |
| M4.6 热重载+timescale | 08-22 | HOTRELOAD 实测生效；0.5x/2x 帧率无关成立；crouch_walk 分支就绪 | 同上 |
| M5.7 locomotion_gate | 08-23 | 七阶段起跑→急停→原地转身→蹲走钳制 1.50 全程无脚滑跳变 PASS | logs/m5-gate-run.log |
| M6.5 overlay_gate | 08-23 | Torch 渐入渐出与腿部解耦 + aim 四象限扫掠平滑 PASS | logs/m6-gate-run.log |
| M7.7 foot_ik_gate | 08-23 | 三 VERDICT 全 PASS（贴地/坡站 pelvis/转身钉地零漂移）；workspace 146 绿 | logs/m7-gate-run.log |
| M7.8 增补复跑 | 08-23 | ALS_float_curves 导入通道重做动态 mask；overlay_gate 复跑 PASS；154 绿 | 无独立日志（正文在快照） |
| M8.1 air_gate | 08-23 | 轻/重落地分段出现 VERDICT PASS；161 绿 | logs/m8-air-gate-run.log |
| M8.3 stop_gate | 08-23 | plant_on_stop=true slide_in_band=true PASS；172 绿 | logs/m8-stop-gate-run.log |
| M8.4 mantle_gate | 08-23 | 低台/高台均登顶 Grounded PASS；176 绿 | logs/m8-mantle-gate-run.log |
| M8.5 actions_gate | 08-23 | 冲刺前滚/助跑跳轻落地/停步匹配综合 PASS | logs/m8-actions-gate-run.log |
| M8.10 root motion 验收 | 08-24 | validator PASS + workspace 全绿 + 三门禁复跑 PASS | logs/m8-root-motion-acceptance-run.log |
| M8.8 补遗终态复跑 | 08-24 | roll 2.70/2.72 零根骨警告、mantle 1.72/2.17、actions 综合 PASS | logs/m8-root-motion-{roll,mantle,actions}-gate-run.log |
| M9.3 ragdoll_gate | 08-24 | 运动中倒地自然静止/恢复 y=0.90/nan_bodies=0 PASS；178 绿 | logs/m9-ragdoll-gate-run.log |
| M10.3 camera_gate | 08-24 | probe_min=1.550 不穿模/frame_jump≤0.352/linkage_ok PASS | logs/m10-camera-gate-run.log |
| M10.4 playground_parity_gate | 08-24 | landscape/static/text/moving 全通过 VERDICT PASS | logs/m10-playground-parity-run.log |
| M11.2 perf_gate | 08-24 | release avg=1.286ms p95=1.521 ≤ 放宽预算 1.5ms PASS | logs/m11-perf-run.log |
| M11.5 最终 Gate | 08-24 | cargo test --workspace 全绿 + 全模块 Gate 勾选 + parity-report 落地 + README 可复现 | 提交 cdab951 |

测试计数备注：最近一次全量记录 workspace 全绿 187（提交 b8a9150：als-core 129 + als-ext 51 + 其他 7）；
M11.5 时点为 179。差异来自后续增补单测与临时 example 清理，非回归。

## 3. 架构速查

Workspace 成员（根 Cargo.toml）：`crates/als-core`、`crates/als-ext`、`crates/als-camera` + 根 crate `bevy_als`（bin 入口 src/main.rs，编排层 src/lib.rs）。
基座依赖均为公开 git fork：millionart/bevy_animation_graph（master）、millionart/bevy-tnua（main）；bevy 0.19 + file_watcher；bevy_enhanced_input 0.26；avian3d。

### als-core（crates/als-core）— ALS 等价 ECS 核心
模块（src/lib.rs）：aim, air, bones, character, control_scheme, debug_hud, foot_ik, graph_router,
input, mantle, motion, overlay_blend, params, pivot, start_stop, state, roll, rotate_in_place,
transition, transitions, turn_in_place, view。
re-export 五大状态枚举：Gait / LocomotionState / OverlayState / RotationMode / Stance。
插件：AlsInputPlugin、AlsStateTransitionPlugin、AlsCharacterPlugin、AlsMotionPlugin、
AlsStateHudPlugin、AlsAimPlugin、AlsAirPlugin、AlsFootProbePlugin。
职责要点：状态组件+Verdict(Accept/Reject/Queue) 合法性转移；enhanced_input→AlsInputCommand 桥接；
tnua 参数映射与视角旋转；图输入路由纯函数 compute_graph_inputs；起跑/急停/pivot/TurnInPlace/
RotateInPlace/DynamicTransitions 检测器；foot 探测与 pin 请求；air 三相/roll/mantle 分类纯函数；
aim 边沿联动与 AimStateChanged 广播；ALS 数值参数资产 AlsMovementParams。

### als-ext（crates/als-ext）— 基座插件扩展层（不改 fork）
模块：als_curves, curve, distance_match, ik, loader, nodes, overlay, ragdoll_config_node,
playground, root_motion_defaults, time_position。
公开项：CurveAsset/CurveKey/CurveSampleMode/sample_curve、CurveAssetLoader(.curve.ron)、
CurveSampleNode、CurveDrivenValueNode、DistanceMatchNode(+default_stop_distance_curve/
invert_distance_curve/sample_distance_curve)、RagdollConfigNode、TimePositionNode(+resolve_position)、
ik::solve_two_bone_ik/IkHandTarget/spawn_ik_hand_gun、overlay::build_bone_mask/build_grouped_bone_mask/
mask_coverage(TORCH_* LOOK_SPINE_BONES 等常量)、als_curves::AlsAnimCurves/AlsCurvesHandler
(glTF 扩展 `ALS_float_curves` 导入，标注子资产 `#AlsCurves`)、playground::* 装配族(PlaygroundPlugin)、
root_motion_defaults::derive_default_root_bone/select_shallowest_animated_path(RootMotionDefaultsPlugin)。
插件：AlsExtPlugin（注册全部节点类型+曲线 loader+RootMotionDefaults）、PlaygroundPlugin、RootMotionDefaultsPlugin。
自定义图节点族：CurveSampleNode / CurveDrivenValueNode / DistanceMatchNode / RagdollConfigNode / TimePositionNode（全部经 NodeLike 公开扩展点注册，fork 零改动）。

### als-camera（crates/als-camera）— ALSCamera 等价物
公开项：AlsCameraParams(+AlsCameraParamsLoader，扩展名 .camera.ron)、AlsCameraMode
(ThirdPerson/AimingShoulder/FirstPersonPlaceholder)、AlsCameraRig、AlsCameraTarget、
mode_for_rotation、spring_arm_length、blended_shoulder、blended_arm_length。
插件：AlsCameraPlugin（Update 轮询参数加载 + camera_rig_system 射线收缩臂长/肩偏移/aim blend）。

### bevy_als 根（src/）— 编排层
模块：debug_draw, foot_ik_apply, mantle_drive, motion_graph_link, overlay_layer, ragdoll_link,
roll_drive, root_motion_link, stop_link, view_layer。
插件：AlsDebugDrawPlugin(Gizmos 速度球/IK 目标线/躯干连线)、FootIkApplyPlugin(IK apply+二次传播)、
AlsMantleDrivePlugin、OverlayLayerPlugin(CF 曲线→后改 ramp 驱动 overlay_weight)、
AlsRagdollLinkPlugin、AlsRollDrivePlugin、AlsStopLinkPlugin(AlsStopMatch 自持检测)、
ViewLayerPlugin(pitch 阻尼+look pins)。root_motion_link 为 RootMotionOutput→LinearVelocity 消费辅助；
motion_graph_link 提供 compute_motion_graph_inputs/write_air_pins 等 pin 写入助手。

### 关键资产路径约定
- `assets/graphs/*.animgraph.ron`：主图 locomotion_overlay.animgraph.ron（Base+overlay/look/air/land/roll/stop/mantle 全链），
  另有 locomotion_speed / crouch_walk / curve_gate / graph_node_walk_fwd / skeleton_boot。
- `assets/animated_scenes/`：motion_overlay.animscn.ron（角色动画场景入口）、als_ragdoll.animscn.ron（ragdoll 进出）、其余 gate 用。
- `assets/animations/**.gltf`：127 AnimSequence 含 `ALS_float_curves` 内嵌曲线扩展；Roll/Mantle×3 已回填真实 root motion 轨道。
- `assets/curves/`：UE 导出曲线数值表 JSON → tools/import/gen_curves.py 转 .curve.ron。
- `assets/ragdolls/als_manny.rag.ron`（16 刚体/15 SphericalJoint）+ `assets/ragdoll_bone_maps/als_manny.bm.ron`。
- `assets/params/als_camera.camera.ron`；`assets/models/playground/`（SM_Als_* glTF + playground.ron/playground.json/playground_parity.ron）。
- 登记簿：`assets/docs/export-manifest.md`（章节：约定/登记记录/M3.1 增补/M0.5b 增补/M9.1 Ragdoll/M8.6 Root Motion 回填；主登记 158 条来源记录 + M0.5b 补 367 条内嵌曲线/82 资产）。

## 4. 关键数值事实表（出处=CHECKLIST 条目号）

| 事实 | 值 | 出处条目 |
|---|---|---|
| walk/run/sprint 三档速度 | 1.75 / 3.75 / 6.50 m/s | M1.5（AlsMovementSettings.h:21/28/35）；M3.8 实测达标 |
| 蹲速 | 1.50 m/s | M1.5（AlsCrouchingSettings.h:13）；M5.7 实测钳制 |
| 加速/减速/摩擦 | 20.0 / 15.0 / 4.0 | M1.5（AMC.cpp:139-141） |
| 胶囊站半高/半径 | 0.90 / 0.30 m | M1.5（AlsCharacter.cpp:31） |
| 蹲姿胶囊半高 | 0.40 m | M1.5（引擎默认无覆写）；M5.7 实测 0.39/0.40 |
| 跳跃 Z 初速 | 4.20 m/s（重力 1.0） | M1.5（工程默认） |
| yaw 转速 VD / View / Aiming | 800 / 500 / 1000 deg/s | M1.5（AlsCharacter.cpp≈1574-1884） |
| TurnInPlace | 触发 45°、180 判定 130°、play_rate 1.2 | M5.3（AlsTurnInPlaceSettings） |
| RotateInPlace 映射 | 视角转速 180–460 °/s → play_rate 1.15–3.0 | M5.4（AlsRotateInPlaceSettings） |
| 最大可行走坡度 | 50° | M7.1（ALS max_slope 对齐） |
| Pivot | 反转>100°、门槛 2.0 m/s、冷却 0.3s | M5.2 |
| coyote / jump buffer | 0.10s / 0.15s（推断项，ALS 无此机制） | M1.5 |
| Stop distance match S 曲线 | 足触点 t_norm=0.85 | M8.3 |
| Mantle 分类线 | High≥1.25m / Low 0.45–1.25m | M8.9（MantlingHighHeightThreshold） |
| Roll clip 根位移/时长 | 2.75 m / 1.5 s | M8.6 回填数据；时长修正见 M8.4 附带修正 |
| 重落地判别阈值 | 撞击 vy≥8 m/s（EMA α=0.3 滤波） | M8.1 |
| 图求值性能预算 | release avg 1.286ms p95 1.521ms 达放宽线 1.5ms | M11.2 |

## 5. 输入模型现状

- 输入栈：bevy_enhanced_input 0.26，上下文组件 `OnFoot`，7 个 action（Move/Look/Jump/Crouch/Sprint/Aim/Roll）；
  `als_input_bridge_system` 在 PreUpdate 把 action 值写入角色 `AlsInputCommand` 组件（crates/als-core/src/input.rs）。
- AlsInputCommand 字段：move_dir(Vec2)、look_delta(Vec2)、jump_held(bool)、jump_pressed(bool)、
  crouch_pressed(bool)、roll_pressed(bool)、sprint_held(bool)、aim_held(bool)；reset() 全清零。
- 默认键位（AlsInputMap）：WASD 移动、鼠标移动视角、Space(手柄 South) 跳、CtrlLeft(手柄 RT) 蹲(Press)、
  ShiftLeft(手柄 LThumb) 冲刺、鼠标右键(手柄 LT2) 瞄准、AltLeft(手柄 East) 翻滚(Press)。
- 生产消费者现状：move_dir/jump_held ← AlsMotionPlugin（tnua 喂入+视角旋转）；aim_held ← AlsAimPlugin；
  roll_pressed ← roll/mantle 逻辑；look_delta ← motion 视角积分。
- **已知缺口 1**：crouch_pressed 与 sprint_held 在任何 crate 内都没有消费系统——只有 examples 消费
  （motion_gate.rs 读 crouch_pressed；各 gate 经 AlsInputOverride 注入脚本值）。对应 bug「Ctrl 蹲 / Shift 冲刺无效」。
- **已知缺口 2**：examples/manual_playground.rs（run_manual.bat 启动的即玩例程）只挂了
  DefaultPlugins/PhysicsPlugins(FixedPostUpdate)/TnuaAvian3d/TnuaController/EnhancedInput/AlsInput/
  AlsStateTransition/AlsCharacter/AlsMotion/AlsStateHud/AlsDebugDraw/Playground/AnimationGraph/AlsExt——
  未挂 air/roll/ragdoll/view/overlay/stop/mantle/foot_ik/camera 等插件；相机为例内固定跟随（camera_follow）而非 AlsCameraPlugin。
  对应 bug「Alt 翻滚无效」「跳跃无动画状态」（缺 AlsAirPlugin）等。

## 6. 已知偏离与后置项汇总

架构级刻意差异（CHECKLIST 附录 A，规范性）：枚举组件替代 GameplayTags；显式 system 路由替代 AnimBP；
分析式 IK 替代 ControlRig；Root Motion 用图内 RootMotionMode 提取+RootMotionOutput（M8.8，数据经 M8.6 回填）；
自研 uasset→glTF `ALS_float_curves` 通道补浮点轨道；CurveAsset 替代 UBT CurveFloat；独立 als-camera crate。

行为级低配/偏离（本目录 parity-report.md §2 十项要点）：
- 急停仅单边时间 warp，预测分母为恒减速解析值（实测滑移 0.205m vs 预测 0.469m，对齐性不受影响）。
- TurnInPlace 检测器与脚部钉地已生效，但 Turn 动画本体未接入图链（增强项）。
- AimOffset 的 yaw 由身体朝向承担（库内无 yaw 偏移资产），pitch 走 BlendSpace1D look 链。
- Rifle 双手 overlay 与 HandLeftIk/HandRightIk 消费未做（无持枪需求）。
- Foot IK 只锁垂直 offset，水平位移不锁定（急停/转身横向滑动依赖动画本身）。
- Ragdoll 退出直接对齐 pelvis 位姿，无 GetUp_Front/Back 动画混合（资产已在库，增强项）。
- Mantle 免按键自动触发（前向输入遇可翻台面），ALS 为专用输入——记录在案的输入偏离。
- FirstPerson 相机为占位（eye 高度+臂长 0.05）。
- 网络同步与 AI（ALSExtras）为 goal 明确排除项。
- 性能未达原生 0.5ms 目标（release 1.286ms 达放宽线），优化方向：节点合并/Context 分发降耗/mask 常量化。

其他记录在案：M10.4 text=0 查询待排查（标签已 spawn 未命中 query）、贴图像素未导出用参数色替代、
Landscape 先近似平面后经提交 cd32922 升级为程序化高度场；SpeedNode pose_ts 记账伪影不影响视觉相位；
ALS_TIMESCALE 基建可用（Time<Virtual>）。

## 7. 待办：用户报告的 6 个 bug（2026-08-24 原文照录）

1. WASD 方向相反
2. 双腿插地需对照胶囊与模型位置
3. 鼠标视角·Ctrl蹲·Shift冲刺·Alt翻滚无效
4. 跳跃无动画状态
5. 缺 ALS 对标按键无法攀爬无法 ragdoll
6. 缺 HUD 1:1 复刻

计划：按依赖序建立 BUGFIX 清单逐项修复（先补输入消费缺口与 manual_playground 插件编排，
再修视觉/物理表现，最后 HUD 1:1 复刻）；修复须在 CHECKLIST.md 增补明确条目后执行并各自留证据。

## 8. 快速上手

- 即玩：双击 `run_manual.bat`（启动 target/release/examples/manual_playground.exe，缺失时自动 release 构建）。
- Gate 证据复现件：examples/ 下 skeleton_boot、state_switch、curve_gate、motion_gate、locomotion_gate、
  overlay_gate、foot_ik_gate、air_gate、roll_gate、stop_gate、mantle_gate、actions_gate、ragdoll_gate、
  camera_gate、perf_gate、debug_draw_smoke、playground_parity_gate、playground、manual_playground。
- 自动化运行：设环境变量 `ALS_BOOT_AUTO_QUIT_SECS=<秒>` 脚本化退出；`ALS_TIMESCALE` 可变速回放。
- 测试：`cargo test --workspace`（最近全量 187 绿，见第 2 节备注）。
- 资源限流（agents.md 规则 9，严格执行）：跑 cargo build/test/check/clippy 前 jobs=min(逻辑核/4, 可用内存GB/3) 钳 [2,8]，
  写入用户级 ~/.cargo/config.toml 或以 --jobs 注入；未限流导致系统假死视同执行事故。
- 提交隐私自检：运行 agents.md 规则 7 的 git grep 模式检查命令，必须无命中（模式原文以 agents.md 为准，本文件不复制以免污染检查结果）。

## 9. 文档地图

agents.md=执行纪律（10 条规则+教训）；CHECKLIST.md=权威清单（进行中工作；已完成基座见本目录归档）；
goal.md=goal 启动方式；docs/archive/2026-08-24/parity-report.md=与 ALS 差异对照；docs/ik-scheduling.md=IK 调度链路；
assets/docs/export-manifest.md=资产登记簿；docs/env.md=本地隐私环境（gitignored，内容严禁入库或外传）；
docs/archive/2026-08-24/=M0–M11 完成态归档（快照+状态+日志+parity-report）。
