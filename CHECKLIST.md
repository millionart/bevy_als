# bevy_als — 执行清单

> 本文件是唯一权威执行清单；执行纪律见 `agents.md`（仅规则），启动方式见 `goal.md`。
> 完成一项即勾选并一次提交：实现 → 自测 → 勾选 `[x]` → `git commit -m "<前缀><编号>: <条目摘要>"`（勾选与实现同一提交）。
> 基座模块提交前缀沿用 `M<编号>`；缺陷修复清单用 `BF<编号>`。

## 归档注记（2026-08-24）

- M0–M11 全部清单项与 Gate 已完成，整体归档至 [`archive/2026-08-24/`](archive/2026-08-24/)：
  - `CHECKLIST-M0-M11.md` —— 原 M0–M11 清单逐字快照（全部条目实现细节与 Gate 证据正文）；
  - `PROJECT-STATE.md` —— 状态浓缩（新会话恢复上下文的入口，先读它）；
  - `logs/` —— 全部门禁运行日志；`parity-report.md` —— 与 ALS 差异对照。
- 本文件此后只登记进行中的工作；已完成基座仅保留下方索引。附录 A/B 为规范性内容，持续有效。

- 2026-08-29：下列所有子项均已勾选的完整批次已归档，主清单不再重复展开其事实与 Gate 记录：
  - [x] BF100-2026-08-27（25/25）——[`archive/2026-08-29/BF100-2026-08-27.md`](archive/2026-08-29/BF100-2026-08-27.md)
  - [x] SYNC-PARITY-2026-08-28（10/10）——[`archive/2026-08-29/SYNC-PARITY-2026-08-28.md`](archive/2026-08-29/SYNC-PARITY-2026-08-28.md)
  - [x] BUGFIX-2026-08-24（14/14）——[`archive/2026-08-29/BUGFIX-2026-08-24.md`](archive/2026-08-29/BUGFIX-2026-08-24.md)
  - [x] BF-Scene-2026-08-27（6/6）——[`archive/2026-08-29/BF-Scene-2026-08-27.md`](archive/2026-08-29/BF-Scene-2026-08-27.md)
  - [x] ANIM-PARITY-2026-08-25（10/10）——[`archive/2026-08-29/ANIM-PARITY-2026-08-25.md`](archive/2026-08-29/ANIM-PARITY-2026-08-25.md)

- 2026-08-29：所有含未勾选项的历史批次已整体废止并移入 [`archive/2026-08-29/`](archive/2026-08-29/)；它们只保留历史事实和证据，**不构成待执行清单**：
  - BF-Scene-2026-08-27b——[`obsolete-BF-Scene-2026-08-27b.md`](archive/2026-08-29/obsolete-BF-Scene-2026-08-27b.md)
  - BF-Character-2026-08-28——[`obsolete-BF-Character-2026-08-28.md`](archive/2026-08-29/obsolete-BF-Character-2026-08-28.md)
  - REFACTOR-2026-08-25——[`obsolete-REFACTOR-2026-08-25.md`](archive/2026-08-29/obsolete-REFACTOR-2026-08-25.md)

## P0-2026-08-29（manual 冲刺崩溃）

> 用户报告：从 `run_manual.bat` 启动后，持续按住 Shift 冲刺数秒会使游戏崩溃报错。此处只登记待办；尚未提取原因事实，也未将任何推测视为结论。

- [x] P0-01 `run_manual.bat` Shift 冲刺崩溃：先以当前完整角色 Actor 入口复现并保留崩溃日志/调用栈，再按涉及的 ALS-Refactored 行为和当前实现实时取证定位根因；修复不得为 manual 场景添加特例，须落在完整角色系统。依赖：本轮归档完成后立即执行，不等待其他普通待办。Gate：相同启动方式下连续冲刺输入及可复查的自动输入序列均无 panic、崩溃或 ERROR，进程以受控退出码结束，运行日志证明完整 Actor 和 linked `AB_Als` 根图仍在用。
  - 实时事实（2026-08-29，ALS-Refactored `AlsAnimationInstance.cpp:1025-1064`、`AlsCharacter.cpp:675-747`）：原版在持续 Sprinting 时每帧只更新 `SprintTime`，0.5 s 后将 `SprintAccelerationAmount` 归零；播放速率以 `Speed / AnimatedSprintSpeed / StrideBlendAmount` 计算并钳制到 `(0, 3]`。角色以当前最大允许 Gait 决定冲刺，并只在设置允许时切换到 VelocityDirection；该持续状态没有按时间累积创建角色或相机对象的路径。
  - 当前实现事实（2026-08-29，`crates/als-core/src/input.rs:121-128, 310-361`、`motion.rs:337-370`、`gait.rs:262-303`）：Shift 持有超过 0.1 s 后由 `AlsSprintInputLatch` 维持 `AlsInputCommand.sprint_held`；运动、Gait、Aim 等完整角色系统统一从可选的 `AlsInputOverride` 或该命令读取输入。因此 P0 Gate 可以只向完整 Actor 的已有 override 写入持续前进/冲刺，不改变 manual 或图合同。
  - 复现证据（2026-08-29，`RUST_LOG=info RUST_BACKTRACE=1 cargo run --example sprint_stress_gate --jobs 2`）：完整 Actor 已生成、相机存在，`t=1.10 s` 和 `t=2.04 s` 均为 `Gait=Sprinting`、速度 `6.500 m/s`；此时 Graph Player 已报告 error。随后 Avian 在 `assert_components_finite` 报告 entity `763v0` 的 `LinearVelocity` 为 NaN/∞，并在 collider AABB 更新中触发 `b.min.cmple(b.max).all()`；这已实证为 native 崩溃的直接前因，下一步必须记录 Graph error 文本和该实体与角色/ragdoll 的归属。
  - 分离实验（2026-08-29）：先以 Bevy `AssetServer::is_loaded_with_dependencies` 等待根 `AB_Als` 的递归依赖全部加载，再生成同一完整 Actor；此前的 `FsmAssetMissing` 消失，但 `t≈2.0 s` 仍有一个 16-body ragdoll 的 Avian `LinearVelocity` 变为 NaN。故资产就绪错误是独立缺陷，不是本 P0 的充分原因；P0 根因应继续沿 ragdoll body 的速度写入链定位。
  - 根因实证（2026-08-29，Bevy live probe 与 `bevy_animation_graph` Avian adapter 源码实时核对）：出错实体是 `BodyLabel("calf.R")` 的 kinematic ragdoll body；其有限 `relative_target` 与有限 pose 相对比，body `Position` 已发散到约 `10^37`，随后 `LinearVelocity` 出现 `-inf`。基座 `update_relative_kinematic_position_based_body_velocities()` 每物理帧以 `(relative_target - current_relative_position) / delta_seconds` 写入该速度，且正常态也对所有 ragdoll body 执行该路径。原版 `StartRagdollingImplementation()` 先将 mesh 从角色分离、再开启 `SetSimulatePhysics(true)`；`StopRagdollingImplementation()` 则显式 `SetSimulatePhysics(false)` 和 `QueryOnly`（`AlsCharacter_Actions.cpp:784-807,1081-1089`）。因此 Bevy 正常 locomotion 必须取消 ragdoll body 的物理速度积分；进入 Ragdoll 的单次激活再以当前动画目标设置 world pose 与角色速度，不能让正常态 kinematic body持续被物理积分。
  - Gate 记录（2026-08-29）：`RUST_LOG=info cargo run --example sprint_stress_gate --jobs 2` 在完整 Actor、linked `AB_Als` 根图、相机、物理与 16 个 ragdoll body 同时存在的条件下，连续 `7.924 s` 的前进冲刺保持 `Gait=Sprinting`、`speed=6.500 m/s`、finite pose、`animation_error=false`，最终 `actor=true input=true camera=true pose=true animation_error=false -> PASS`，进程退出码为 0。随后用原始入口执行 `ALS_BOOT_AUTO_QUIT_SECS=12 RUST_LOG=info cmd /c run_manual.bat`，PlayerStart→完整 Actor admission、相机、碰撞、材质和 grounded probes 均 PASS，自动退出码为 0，运行中无 panic 或 ERROR。实现位于通用 `ragdoll_link`：正常 locomotion 在物理步前归零 kinematic ragdoll body 的线/角速度；仅在真实 Ragdoll 激活边沿将 body teleport 到当前动画目标的 world pose、继承角色线速度并切至 Dynamic。

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
```

## 已完成基座索引（详情一律见 `archive/2026-08-24/CHECKLIST-M0-M11.md`）

- [x] M0 环境与资产管线（Gate：skeleton_boot 骨架+动画加载播放 OK）
- [x] M1 ECS 核心状态模型（Gate：state_switch 九步序列合法转移全符合预期）
- [x] M2 曲线与数据驱动基础设施（Gate：curve_gate 曲线驱动数值逐点吻合）
- [x] M3 tnua 运动基座（Gate：motion_gate 三档速度精确达标无回归）
- [x] M4 动画图基座（Gate：速度自动混合+热重载+timescale 帧率无关）
- [x] M5 Locomotion 状态机动画（Gate：locomotion_gate 七阶段全程无脚滑跳变）
- [x] M6 Overlay 与 Aim/View 系统（Gate：overlay_gate 叠加解耦+aim 四象限平滑）
- [x] M7 Foot IK 与 ControlRig 等价物（Gate：foot_ik_gate 三 VERDICT 全 PASS）
- [x] M8 特殊动作（Gate：actions_gate 滚/跳/停步综合 PASS + root motion 链路验收）
- [x] M9 Ragdoll（Gate：ragdoll_gate 自然倒地恢复 PASS）
- [x] M10 相机（Gate：camera_gate 不穿模/aim 切换平滑/数据联动 PASS）
- [x] M11 调试工具与总验收（Gate：workspace 测试全绿 + parity-report 落地）

---

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
