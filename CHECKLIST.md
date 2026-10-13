# bevy_als — 执行清单

> 本文件是唯一权威执行清单；执行纪律见 `agents.md`（仅规则），启动方式见 `goal.md`。
> 完成一项即勾选并一次提交：实现 → 自测 → 勾选 `[x]` → `git commit -m "<前缀><编号>: <条目摘要>"`（勾选与实现同一提交）。
> 基座模块提交前缀沿用 `M<编号>`；缺陷修复清单用 `BF<编号>`。
> 已归档清单：[archivedchecklist.md](docs/archive/archivedchecklist.md)

## BF144 — ALS 运动系统持续 source-first 差异审计

用户授权（2026-10-13）：持续审计 ALS 运动系统本身，及时登记遗漏移植差异；周边内容暂不展开，截图关闭，性能验收继续暂停。本轮审计不修改生产实现或原始资产。未覆盖边界、未解释差异和确认缺陷均不得计作“无差异”；不以历史通过或有限测试宣称绝对等价。

范围按本地 ALS 源闭包推进：Character 状态/输入/旋转 → CMC 地面/空中/碰撞/基座/蹲跳 → Roll/Mantle → Ragdoll/GetUp → 运动动画输入/生命周期/通知 → 核心 AnimBP/Rig 与实际消费。排除 Camera、场景、材质、UI、Overlay 道具表现、音效/特效和网络同步。发现差异先追到实际消费者并保留最小反例，登记不代表授权修复或复开相邻已完成项。

- [x] BF144.01 本轮基线、源码闭包和覆盖入口：核对原始 ALS 插件/本地 UE/当前生产代码的身份，重新提取角色和运动设置，列出按源责任分组的审计入口与既有 Gate 漏检边界。Gate：fresh source 身份、native reader/watchdog 报告、范围表和后续独立审计项可核查。开始：2026-10-13 18:49 +08:00；预估结束：2026-10-13 19:13 +08:00；实际结束：2026-10-13 18:54:50 +08:00；偏差：−18分10秒；结果：基线 Gate PASS，语义审计按下表分项继续。估时依据：BF140 的 19 个 native 资产及 C++ 快照、BF140.03 的 20 分钟闭包审计记录；本项仅取角色/配置及清点源码，不运行完整图审计或构建。
  - 18:53 活动摘要：生产基线 `5554874b`，工作树干净；已读当前执行规则、源归属、BF140/BF141 复审记录和原生 reader 的 watchdog 入口。当前没有已确认新增差异；下一步核对原始插件身份、源函数责任与现有消费者，按规模拆分后续审计。截图/性能均未启动。
  - 18:54 source/static 闭环：原始插件 `b754d6f0`；112 个 ALS C++/header 与4个 UE Character/CMC/Actor/Capsule 文件已快照/hash，160 个 target Rust 文件仅做身份登记，不能视为已审计。fresh B_Als_Character、CS_Als_Default、MS_Als_Normal 均与 reader 实际加载的 Build 资产逐字节一致，3个 reader report PASS；`pwsh tools/run_ue_source_reader.ps1 -SelfTest` 的正常完成、停滞终止/子进程清理及机器报告全部 PASS。证据：`target/bf144/inventory.json`、`target/bf144/source/`。原始插件另有 Bow 资产工作区改动，属于本轮排除的道具，不据此扩展范围。复用既有 watchdog，未新增生产抽象/状态/分支，设计消融不适用。

| 顺序 / 源责任 | 本轮 target 核对入口 | 覆盖状态与旧 Gate 边界 |
|---|---|---|
| Character 输入、Gait、RotationMode、Grounded/InAir rotation | `state/{aim,gait,transition}`、`character/movement/motion`、`actual_motion` | 待逐项审计；BF140/141 回归不覆盖所有动作×状态×旋转意图组合 |
| CMC 时钟、速度、地面/空中、碰撞、蹲跳、基座 | `character/movement/{motion,dynamics,control_scheme,source_physics_timing}`、`tnua_ext` | 待逐项审计；按函数和有效分支核对，不能以既有场景通过代替 |
| Roll、Mantle、RMS、动作切换 | `character/actions/{roll,roll_drive,mantle}`、`root_motion_sources` | 待逐项审计；角色蓝图 SelectMantlingSettings 属此闭包 |
| Ragdoll、GetUp、PA 运动消费者 | `character/{pa_als,actions/ragdoll_link,actions/get_up}`、`avian_ext` 的实际调用 | 待逐项审计；不开展无源链路的通用物理审查 |
| AnimationInstance 输入、生命周期、Feet/Turn/Transition、通知 | `character/animation_instances`、`state`、`notifies` | 待逐项审计；实际动画选择包含角色 Overlay 标签，但排除道具表现 |
| 核心 AnimBP/FSM、曲线、同步、Root Motion、Rig | 已生成核心图、实际节点与 `control_rig` | 待重新取证及核对；历史155子图合同只作起点，不能自证算法等价 |
| 剩余源函数与跨阶段边界 | 以上 source-target 链路的覆盖缺口 | 前述审计后再独立收口；未解释项保留，截图与性能继续暂停 |

- [ ] BF144.02 Character 输入、状态与旋转语义：以 fresh Character/CMC、角色/运动配置核对初始化、输入、步态、旋转模式与地空旋转的有效分支；沿实际消费者确认候选，并用独立最小反例与对照记录遗漏。Gate：逐源分支覆盖记录、生产库身份与 focused 结果、确认/排除候选登记。开始：2026-10-13 18:55 +08:00；预估结束：2026-10-13 19:40 +08:00。估时依据：Character 约1960行与既有 motion/state 消费者、BF140 的15项基础运动反例和 BF141.03/.04 focused 回归；将 CMC 积分/碰撞另列后续项以控制本项规模。
  - 18:55 活动摘要：基础配置/输入/状态函数已定位；候选是离开 Grounded 后保留的 MaxAllowedGait 与 RotationMode 消费条件，尚未确认缺陷。下一步建立 source 分支对照与真实角色反例，不修改生产代码。

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
