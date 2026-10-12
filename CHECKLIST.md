# bevy_als — 执行清单

> 本文件是唯一权威执行清单；执行纪律见 `agents.md`（仅规则），启动方式见 `goal.md`。
> 完成一项即勾选并一次提交：实现 → 自测 → 勾选 `[x]` → `git commit -m "<前缀><编号>: <条目摘要>"`（勾选与实现同一提交）。
> 基座模块提交前缀沿用 `M<编号>`；缺陷修复清单用 `BF<编号>`。
> 已归档清单：[archivedchecklist.md](docs/archive/archivedchecklist.md)

## BF140 ALS 运动系统第二轮只读审计

用户范围（2026-10-12）：重新审计 ALS 运动系统本身；周边场景、材质、UI、相机和示例物件排除，截图关闭、性能验收继续暂停。只登记有当次本地 source、实际 target 消费者和可重复反例支撑的差异，不实施修复或重开历史实现。报告见 `docs/locomotion-audit-2026-10-12.md`。

- [x] BF140.01 Character / CMC / 状态：输入、步态、旋转、蹲起、跳跃、地面/空中事务、移动基座与时钟。开始：2026-10-12 15:00 +08:00；预估结束：2026-10-12 15:50 +08:00；实际结束：2026-10-12 15:12:25 +08:00；偏差 −37分35秒。估时依据：BF139.01 的同类 C++/ECS 消费链及已有独立生产库反例入口；本轮重用工具，只补遗漏边界。结果：审计 Gate 完成，确认 4 项 P2，未实施修复；无需拆分。重用生产库避免了完整构建，同类只读复审后续估时可缩短。
  - source/static 摘要：原始 Character/CMC、UE Character/CMC/Capsule/Actor 与 3 个 fresh 原生资产闭环；15 个实际消费者反例/对照为 9 PASS、6 预期语义 FAIL，归并为跳跃按住时钟、父级缩放地面尺寸、静止速度朝向旋转、方向配置 ActualGait 四项。已排除输入归一化、加速度 owner delta、基座升降起跳与默认 InAir 枚举差异；watchdog 自测通过。下一步只查动作/Root Motion/Ragdoll 闭包；生产实现及 BF139 已完成项保持不变。
- [x] BF140.02 动作 / Root Motion / Ragdoll：Roll、Mantle、GetUp、动作抢占与物理移交。开始：2026-10-12 15:14 +08:00；预估结束：2026-10-12 15:49 +08:00；实际结束：2026-10-12 15:26:28 +08:00；偏差 −22分32秒。估时依据：BF139.02–03 源闭包、本轮 .01 的生产库重用耗时与必要原生动作/PA 读取。结果：审计 Gate 完成，新增 2 项 P2，未实施修复；无需拆分。
  - source/static 摘要：fresh Actions/Mantling RMS/UE RootMotionSource、PA 与 4 个动作蒙太奇闭环；PA 21/20/49 与 Ragdoll/GetUp 设置重建一致。新增 8 个完整角色测试，累计 23 项为 12 PASS、11 预期语义 FAIL；新增 Root Motion 发布速度时钟和 Ragdoll 恢复 Pitch/Roll 两项。下一步仅沿 Character 动画输入到运动图/Rig 消费闭包审计；截图与性能暂停边界不变。
- [ ] BF140.03 运动动画：AnimationInstance 输入、Grounded/Standing/Crouching/Air、转身/停止、图求值/曲线/同步及角色 Rig 消费边界。依赖 BF140.02 审计记录完成；开始/预估结束在启动时登记。估时依据：BF139.04–05 的已知完整生成闭包与当前源合同；必要时按消费者拆分。Gate：当次 source 与当前产物/实际消费核对、最小反例、报告汇总；不启动性能或截图验收。

活动摘要：已确认目标基线 `5952afcd` 工作区干净，原始 ALS 插件 `b754d6f0` 的 ALS 源码无修改；工程插件链接指向 Build 副本，C++ 取证固定读取原始插件根。BF139 的 18 项已修问题只作去重。未修改生产代码、资产或转换器，无新增设计需要消融。

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
