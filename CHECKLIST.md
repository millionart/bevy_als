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
```

## BF102 — 2026-08-29 默认生产链审计重组

来源是已校验的 2026-08-29 58 项静态审计包（`findings.json` SHA-256 `40be20020dc11ad7b39a145aee3d8a948b703ea0ddd1dac56b34fe3b1327ff1d`）。其 Bevy 基线为 `ea05bbe`，早于已归档的 [BF101](docs/archive/2026-08-29/BF101-2026-08-29.md)；因此此处的 M02、M03、M09 只能作为**当前状态回归核验**，不得直接重开或宣称已解决。审计为固定源码与公式／控制流取证，不是完整 UE／Bevy 实机回放；条件性资产/底座项不得冒充默认场景故障。

本段仅登记范围、依赖和验收条件，不授权源码提取、资产查询、实现或验收。每项成为当前执行项时必须重新从 ALS-Refactored 提取当次事实，并在确认缺陷后创建独立修复子项；修复 Gate 通过前不得推进其依赖项。旧的 U01–U16 泛化清单已归档至 [BF102-pre-reconciliation-2026-08-29](docs/archive/2026-08-29/BF102-pre-reconciliation-2026-08-29.md)。

```
BF102.0 BF101 基线回归（M02/M03/M09）
 ├► BF102.1 默认执行链（A01/A02/A03/Q01） ─► BF102.2 槽位与 overlay 曲线（A05/A06）
 ├► BF102.3 坐标/状态/投影（M01/M04/M05/M08/C01）
 │    └► BF102.4 实例所有权与移动基座（A04/F08/M06/M07/I04/X01/X02/C06/F09）
 └► BF102.5 动画底座语义（F01–F07）
      ├► BF102.6 空中与动作时间（L01–L09） ─► BF102.9 相机时间与行为（C02–C05/C07/C08）
      ├► BF102.7 Foot IK 闭环（I01–I03/I05–I07）
      └► BF102.8 特殊动作闭环（X03–X08）
BF102.10 网络范围（Q02） ─────────────────────────────────────────────────────────────► BF102.Gate
```

覆盖核对：3 + 4 + 2 + 5 + 9 + 7 + 9 + 6 + 6 + 6 + 1 = 58 个审计 ID；每个 ID 只归入一个当前 BF102 项。

- [ ] **BF102.0 — BF101 基线重合项的当前状态回归核验（M02、M03、M09）**
  - 依赖：已通过 BF101.Gate。
  - 目标：区分 `ea05bbe` 审计快照与 BF101 后实现：M02 对应视角相对台阶方向、M03 对应逻辑 stance 与物理胶囊同步、M09 对应方向相关速度。
  - 验收：默认生产入口的探针给出三项当前状态；仅将仍可复现的部分创建修复子项，不能用旧快照重做 BF101。

- [ ] **BF102.1 — 默认执行链与实际 FSM 消费（A01、A02、A03、Q01）**
  - 依赖：BF102.0。
  - 目标：从默认 avatar 入口追踪动作请求、状态转换、头部曲线和最终 FSM／骨骼输出，建立“请求 → 消费 → 最终姿态”证据链。
  - 验收：翻滚槽位、一次性转换、ViewBlock/PoseAiming 与 FSM state id 在 30/60/144/240 FPS × fixed dt 探针中可追踪；确认断点逐项完成修复子项与 Gate。

- [ ] **BF102.2 — 槽位/overlay 姿态与曲线传递（A05、A06）**
  - 依赖：BF102.1。
  - 目标：核验 overlay 输入不再硬编码，并使 slot 注入姿态、曲线、权重和动作许可使用同一角色、同一时间域。
  - 验收：动作槽位的 FootLock、ViewBlock、SprintAcceleration、AllowTransitions 在最终消费者处按权重可复查；条件性曲线不存在时保持明确默认值。

- [ ] **BF102.3 — 坐标、实际状态、旋转律与投影合同（M01、M04、M05、M08、C01）**
  - 依赖：BF102.0、BF102.1。
  - 目标：统一 yaw 正负号、速度/输入方向来源、aim override、实际旋转曲线与 UE 水平 FOV 到 Bevy 垂直 FOV 的换算。
  - 验收：横向翻滚、刹车反向输入、aim override、运动/瞄准/原地转向及多宽高比相机均有数值不变量和最终 Transform/Projection 证据。

- [ ] **BF102.4 — 多角色所有权、移动基座与临时物理状态（A04、F08、M06、M07、I04、X01、X02、C06、F09）**
  - 依赖：BF102.1、BF102.3。
  - 目标：建立每角色 avatar/player/root-motion/临时碰撞/移动基座归属，消除 `single()`、未归属 `next()` 与跨动作恢复冲突。
  - 验收：两个 ALS 角色、无关动画实例、Enabled/Disabled Tnua 实体、移动/旋转平台和非默认 GravityScale 均互不干扰；临时碰撞层与基座相对状态能恢复原值。

- [ ] **BF102.5 — 动画底座与条件资源语义（F01–F07）**
  - 依赖：BF102.1、BF102.2。
  - 目标：逐项核验惯性化、链接层复位、曲线外推/cubic、根旋转加权和 morph 插值的当前底座语义及资产触发条件。
  - 验收：每项给出底座 API、图/资产条件和运行数值；只对当前资产可达的缺陷建立 `als-ext` 或上游隔离修复子项，保留条件项的触发边界。

- [ ] **BF102.6 — 空中、落地与动作时间闭环（L01–L09）**
  - 依赖：BF102.1、BF102.3、BF102.5。
  - 目标：统一预碰撞速度、地面预测形状、成功起跳事件、播放率锁存、曲线积分时间、撞击速度、lean 和缩放来源。
  - 验收：同一输入轨迹的跳跃/落地探针记录预/后碰撞速度、姿态时间、曲线时间、重落地权重与最终骨骼；变速、不同 FPS 和缩放条件都有明确结论。

- [ ] **BF102.7 — Foot IK、骨盆与动态过渡闭环（I01、I02、I03、I05、I06、I07）**
  - 依赖：BF102.4、BF102.5、BF102.6。
  - 目标：按正确父子变换、骨盆顺序、地面法线、锁定重初始化、水平漂移和弹簧响应重建每角色脚锁链。
  - 验收：斜坡、平台、传送、骨盆下移、原地转身和不同帧率下均记录最终足端世界误差、骨盆偏移、锁点与动态过渡选择。

- [ ] **BF102.8 — 攀爬、翻滚与布娃娃的动作闭环（X03–X08）**
  - 依赖：BF102.2、BF102.4、BF102.5、BF102.6。
  - 目标：在正确槽位、所有权、根运动和物理状态基础上核验攀爬轨迹/收尾、根运动分量、权限/缩放、布娃娃退出分流与速度相关驱动。
  - 验收：对移动/旋转/销毁目标、空中布娃娃恢复、非纯前向根运动和条件权限分别记录动作请求、动画相位、位移、碰撞和恢复状态。

- [ ] **BF102.9 — 相机 lag、时间、模式与碰撞恢复（C02、C03、C04、C05、C07、C08）**
  - 依赖：BF102.3、BF102.4、BF102.6。
  - 目标：核验坐标转换后的轴独立 lag、动画/世界时间、time dilation、desired rotation mode、起点多重 MTD 和第一人称头部去缩放时间。
  - 验收：慢动作、暂停、模式切换、墙角、移动基座和变播放率冲刺下的相机位置/旋转/FOV/曲线输入均有可复查数值轨迹。

- [ ] **BF102.10 — 网络范围与本地确定性边界（Q02）**
  - 依赖：BF102.4、BF102.8。
  - 目标：保留项目的单机范围，同时将会影响本地多角色确定性的契约与真正的网络角色/预测语义分开登记。
  - 验收：不将网络缺失伪装成单机画面缺陷，也不将 A04/F08 等本地多实体错误推迟到网络阶段；若用户扩展范围，再独立创建网络清单。

- [ ] **BF102.Gate — 58 项审计重组总验收**
  - 依赖：BF102.0–BF102.10。
  - 验收：58 个 ID 均有“当前已修复／确认待修复／条件性不适用或待证”三态记录；每个确认缺陷均有独立修复提交和真实运行 verdict；未执行的 BF102 项不得被宣称完成。

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
