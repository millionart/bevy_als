# bevy_als — 执行清单

> 本文件是唯一权威执行清单；执行纪律见 `agents.md`（仅规则），启动方式见 `goal.md`。
> 完成一项即勾选并一次提交：实现 → 自测 → 勾选 `[x]` → `git commit -m "<前缀><编号>: <条目摘要>"`（勾选与实现同一提交）。
> 基座模块提交前缀沿用 `M<编号>`；缺陷修复清单用 `BF<编号>`。
> 已归档清单：[archivedchecklist.md](docs/archive/archivedchecklist.md)

## BF139 全范围移植差异审计（只记录）

用户当前目标：找出所有 P2 及以上的移植差异，记录进本清单。审计基线为 `bb3dccf3066a217b750670918e30f248ef2c5b98`；本清单登记待修差异，[审计记录](docs/porting-audit-2026-10-11.md) 保存详细证据与覆盖边界。本轮授权 source/target 审阅、最小反例和文档记录，不修复生产实现、不重开历史完成项、不把 TODO 的待验收状态直接视为缺陷。

- [x] BF139.01 Character / Movement / Input：开始：2026-10-11 11:27 +08:00；预估结束：2026-10-11 12:12 +08:00；实际结束：2026-10-11 11:43:32 +08:00，偏差−28分28秒，审计Gate通过、6项差异未修复。依据本地 Character、CMC、Extras Input 的函数闭包及 BF138 已归档物理审计边界，逐项检查状态、输入、速度、时钟、碰撞、基座和旋转的真实消费者。Gate：源文件当次身份、可定位的算法/输入/生命周期差异、最小反例与漏检原因。后续动作/物理、动画/图/资产、相机/场景/UI 分批独立登记，各批不超过 60 分钟。
  - 初始范围：已确认本地源可用、工作区干净、182 个 Rust 源文件；ALS 插件含 54 个 C++ 与 96 个头文件（含 Editor、网络、调试和本地取证辅助，后者需按职责排除）。已阅读刻意差异与最近 BF135/BF138 结果；历史修复不作为当前缺陷，固定长帧腿平面症状尚未获得 source 根因，不预先定级。

  - source/static 闭环（2026-10-11 11:42 +08:00）：fresh 本地 Character/CMC/Input、UE Actor component tick/PhysFalling/StepUp、AnimationInstance world-space 输入已核对；原生 reader 导出 IA_Als_Sprint 与 CS_Als_Default，watchdog 自测全部通过。直接链接当前生产库的 9 个最小测试为 2 对照 PASS、7 语义断言 FAIL，归并为以下 6 项 P2；没有运行时实现修改。普通 actual rotation request 被覆盖、未接入当前生成图的旧 stop 输入、以及 default bIgnoreBaseRotation=true 下的速度旋转均不单独算缺陷。

- [x] BF139.02 Roll / Mantle / GetUp 与动作 RootMotion：开始：2026-10-11 11:43 +08:00；预估结束：2026-10-11 12:28 +08:00；实际结束：2026-10-11 11:56:37 +08:00，偏差−31分23秒，审计Gate通过、新增3项P2未修复。依据当次 `AlsCharacter_Actions`、Mantle RootMotionSource、蒙太奇消费与 BF137 已修 owner-clock/parent-space 边界，检查启动/抢占/退出、设置、目标基座、slot/notify/root 的实际消费者。Ragdoll solver/PA 独立下一批。
  - source/static 闭环：fresh native 导出 Roll、GetUp Front、Mantle High 的 RateScale/Blend/Trigger/Slot；UE `UpdateMontage→UpdateWeight→Advance→Stop` 与 `AlphaBlend::Update` 时序对照完毕。完整加载角色后的5个动作反例全部语义 FAIL；累计14测试为2对照PASS/12预期FAIL，归并9项P2。Mantle不扫掠与源 `PhysCustom(...,false)` 一致，不登记；GetUp BlendIn native字段为0，初始weight1不是缺陷。当前目标丢失、抢占、montage tail、基座相对空间和已修动作时钟未发现另一项已证实P2。
- [ ] BF139.03 Ragdoll / PA / 物理适配：开始：2026-10-11 11:57 +08:00；预估结束：2026-10-11 12:47 +08:00。依据本地 Ragdoll Start/Refresh/Stop、PA的body/constraint闭包、现有source joint/CCD/sleep实现及 BF135 已归档反例，检查源参数、碰撞/约束/驱动、重建、坐标和最终姿态消费者。Gate：fresh native PA与源算法对照，候选问题以最小反例确认并登记；不修改solver或重跑无关全构建。

### BF139 已确认待修差异（只登记，未授权实现）

以下条目修复均未开始；本轮审计时点及 Gate 属 BF139.01，修复被选为当前任务后再按 fresh source 依赖拆分估时。详细反例与漏检原因见审计记录的同号条目。

- [ ] **BF139-P2-01 Sprint 事件消费差异**：源 `ALSExtras/Private/AlsCharacterExample.cpp:110,161` 在 Started 切换 Sprint；目标 `src/als_extras/core/input.rs:433` 与 `input_request.rs:66` 按按住/释放设置 Sprinting/Running。Running→Shift 按住→释放后源仍 Sprinting，目标 Running，实际输入插件反例已 FAIL。验收应覆盖 Started、短按、保持、释放、再次按下及键盘/手柄，不能仅核对 IA 的 Hold/Released 触发器。
- [ ] **BF139-P2-02 DesiredRotationMode=Aiming 无效**：源 `ALS/Private/AlsCharacter.cpp:675` 将 desired Aiming 与 Aim 按钮独立合并；目标 `src/als/state/aim.rs:55–60` 强制把存储 Aiming 改为 ViewDirection。Grounded/Standing、desired=Aiming、按钮未按下时实际反例 FAIL。验收覆盖 desired Aiming、普通自由模式、按键覆盖/释放、空中和第一人称，并修正旧测试中“stored mode never Aiming”的源合同。
- [ ] **BF139-P2-03 普通运动忽略角色 CustomTimeDilation**：UE `Actor.h:4877` 的组件 tick 使用 owner 倍率；目标 `dynamics.rs:441`/`motion.rs:3447` 的 Walking/Falling 事务使用全局 Delta。3.75 m/s、50 ms 帧、倍率0.5时源水平位移0.09375 m，目标0.1875 m，实际物理反例 FAIL。验收应覆盖 Grounded/Air、0/0.5/1/2 倍率、起跳/落地、基座和动作移交；已完成 Roll/Mantle/GetUp 不因名称相似而复开。
- [ ] **BF139-P2-04 落地丢失当帧剩余行走时间**：UE CMC `PhysFalling:5050→ProcessLanded:6373→StartNewPhysics` 传递 remainingTime；目标 `motion.rs:2697` 发现地面后直接 break。200 ms帧/50 ms子步、3.75 m/s水平速度、从1 m以3 m/s下落时源约0.75 m，目标0.10516852 m且已Grounded；无落地对照0.75 m PASS。验收覆盖子步内落地后的Walking预算、速度/制动/输入及落地动作仲裁，不以“最终Grounded”代替位移证明。
- [ ] **BF139-P2-05 普通 locomotion/provider 混用局部与世界旋转**：源 Character/AnimationInstance 使用 actor world transform；目标 `motion.rs:415,577–579`、`actual_motion.rs:90`、`ab_als_provider.rs:319`、`locomotion_layer.rs:209` 以局部旋转计算世界朝向/局部速度并写物理 Rotation。90°父级下前进2 m/s被识别为左移；真实移动管线中渲染world yaw约0°而物理yaw−90°，两个反例 FAIL，无父级对照 PASS。验收覆盖父实体当帧平移/旋转/缩放及普通 Grounded/Air 的最终朝向和动画输入，不复开已修动作/相机项。
- [ ] **BF139-P2-06 普通 StepUp 忽略组件跨越许可**：UE CMC `CanStepUp:7525` 必须检查 `CanCharacterStepUp`；目标 `motion.rs:3365` 的台阶事务不消费该许可，现 `AlsMantleTargetPermission.allows_step_up` 仅在 `mantle.rs:569` 使用。20 cm台阶许可true/false两次都走到z=−3.4291668，false反例 FAIL。验收覆盖普通低台阶、允许/禁止、actor/child collider许可所有权及Mantle原有分支。
- [ ] **BF139-P2-07 Mantle 用局部位置计算世界 root 速度**：源 `AlsRootMotionSource_Mantling.cpp::PrepareRootMotion` 用目标world减actor world；目标 `mantle_drive.rs:727` 用目标world减局部Transform，`:729,795` 也直接写world到local。完整加载角色仅增加x=5父级并保持世界起点不变，横向速度从约0变成299.99997 m/s，实际反例 FAIL。验收应包括父级平移/旋转/缩放、移动目标基座、最终world姿态/速度及退出惯性；BF137的owner-clock已通过边界不重复修复。
- [ ] **BF139-P2-08 动作方向绕开源已消费输入状态**：源 Roll/Mantle 使用 `LocomotionState.bHasInput/InputYawAngle`（安全归一化阈值平方1e−8）；目标 `roll.rs:57`/`roll_drive.rs:1931` 再用1e−6拒绝有效小输入，Mantle的 `motion.rs→mantle.rs:469` 则先归一化原始输入，使无效微量输入变为有效方向。原生反例：0.0002向右输入已has_input=true，Roll实际yaw0而源−90°；0.00001输入已has_input=false，Mantle trace却向右50°。普通0.01输入对照PASS。验收应从实际消费后的输入状态推导动作方向，覆盖阈值两侧、无输入、真实player-view及已有速度分支。
- [ ] **BF139-P2-09 Roll/GetUp 自然混出时序及剩余预算不同**：UE `AnimInstance.cpp:573–578` 先UpdateWeight后Advance；`AnimMontage.cpp:2626–2636` 首次进入自然混出窗口才Stop，并用当前剩余播放时间作为混出时长。目标 `roll.rs:48` 与 `get_up.rs:112–130` 从连续elapsed直接计算默认长度的fade。100 ms帧下，Roll到0.9 s的首个Stop帧源weight仍1、实际0.93627644；GetUp到1.0 s源仍1、实际0.92592555，完整角色slot两个反例FAIL，进入窗口前weight1对照PASS。验收应覆盖窗口跨越、不同帧长/RateScale、自然与中断、slot pose/曲线/notify权重，并核对下一帧使用剩余预算。

## 背景与基础事实（已核实，持续有效）

- 本机 bevy 源码：`0.19.1`（release-0.19.1）。`bevy_animation_graph` 0.11.0 与 `bevy-tnua` 均基于 Bevy 0.19，兼容。
- bevy_animation_graph 已有节点：ClipNode、ChainNode、BlendNode（含骨骼 mask/Difference/Additive）、BlendSpace1D/2D、布尔运算节点等；图资产 RON 序列化、支持编辑器热重载；Pose 以 `BoneId` 路径表达；RootMotionMode 提取已可用（M8.8）。曲线/ramp/距离匹配等由 als-ext 经公开扩展点补齐。
- bevy-tnua 已有 basis：Walk（coyote time）、Jump（高度/悬挂控制）、Crouch、Dash、Climb、Knockback、WallSlide；物理后端 avian3d 0.12.1 / rapier3d 0.17.0。
- ALS-Refactored 关键结构：4 模块（ALS / ALSCamera / ALSExtras / ALSEditor）；状态体系用 GameplayTags（非枚举）；自定义 AnimNode_CurvesBlend；ControlRig 输入结构 `AlsControlRigInput`；IK 相关骨骼 `foot_l/foot_r`、`ik_foot_l/ik_foot_r`、`VB foot_l/VB foot_r`；关键曲线名 `LayerHandLeft/Right`、`HandLeftIk/HandRightIk`、`FootLeftIk/FootLeftLock/FootRightIk/...`；动画资产分组：Grounded / Air / Actions / Transitions / TurnInPlace / RotateInPlace / Overlays / View / Base。
- 本地环境路径、引擎版本等隐私信息见 `docs/env.md`（已 gitignore）。

## 附录 A：与 ALS 的刻意差异（非缺陷）

| ALS | bevy_als | 理由 |
|---|---|---|
| AnimBP 状态机 | 源合同生成 RON/FSM，由公开节点与 ECS 输入接线执行 | 保留源状态、转换及求值生命周期 |
| GameplayTags | `bevy_gameplay_tag` 的真实层级 Tag | 保留 UE Tag identity 与 exact/hierarchical query 语义 |
| ControlRig | 离线编译源 RigVM 程序，在源 ControlRig 节点中逐实例执行 | 保留 Construction/Forward Solve、当前 Pose/Curve 及图内求值位置 |
| Root Motion | 图内 `RootMotionMode` 提取 + `RootMotionOutput` 驱动位移（M8.8；数据经 M8.6 回填） | fork 已支持提取，M0.5 导出缺根轨道由 M8.6 离线回填，不再用曲线等价物 |
| Curves（UBT 资产） | als-ext CurveAsset + 采样节点 | 见 M2 |
| w_als_hud Overlay 菜单世界锚定头顶骨骼 | 菜单停靠 HUD 面板左侧（BF6b 用户决策 2026-08-25） | 解除 UI 对骨骼/相机投影的耦合 |
| W_Als_Hud 使用 Roboto 字体 | Bevy 默认字体（BF6b 用户决策 2026-08-25） | 字体非功能项，不移植 |
| L_Als_Playground TextRender 使用 RobotoDistanceField 与描边材质 | Bevy 默认字体布局/atlas 生成的 `Mesh3d` 世界字形四边形（BF-Scene 用户决策 2026-08-26） | 不带入字体资产；保留文字内容、颜色、世界锚点与固定朝向，不做相机 billboard |
| S_Als_UI_Click 菜单点击音 | 不复刻（BF6b 用户决策 2026-08-25） | WAV 解码需 bevy `wav` feature（hound 依赖），用户否决新增依赖 |
| UE Shift+数字仍触发玩法Action | Shift+数字用于Debug时仅抑制对应数字binding的玩法动作 | BF9用户决策（2026-08-25；4018824），手柄和非数字binding保持源行为 |

## 附录 B：风险与后置项

- **资产格式决策（2026-08-21）**：Bevy 官方推荐 glTF；M0.5/M0.6 管线定为 UE → glTF（Interchange 导出）→ bevy_animation_graph 自带的 glTF 导入器（GraphClip/Skeleton）。**不引入 bevy_ufbx**：其 v0.19.0（bevy 0.19 兼容）虽支持蒙皮数据，但动画曲线未接入 Bevy 动画系统、无社区验证，且 FBX 中转对 ALS 曲线/虚骨/单位换算风险更大；仅当未来需要直接加载 FBX 时再评估。

- bevy_animation_graph additive/mask 语义若与 ALS layering 不完全一致，在 M6 前验证并决定扩展方式（优先 als-ext 扩展点）。（已于 M6.1 验证通过，记录备查）
- Distance matching / motion matching 类高级特性仅实现 Stop 所需子集（M8.3）。
- ALSExtras（AI 控制器示例）不在范围内。
- 音频（Metasound）、Niagara 特效仅保留挂钩点。ALSCamera 完整移植已纳入 BF116.01。
