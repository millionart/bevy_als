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

## 附录 B：风险与后置项

- **资产格式决策（2026-08-21）**：Bevy 官方推荐 glTF；M0.5/M0.6 管线定为 UE → glTF（Interchange 导出）→ bevy_animation_graph 自带的 glTF 导入器（GraphClip/Skeleton）。**不引入 bevy_ufbx**：其 v0.19.0（bevy 0.19 兼容）虽支持蒙皮数据，但动画曲线未接入 Bevy 动画系统、无社区验证，且 FBX 中转对 ALS 曲线/虚骨/单位换算风险更大；仅当未来需要直接加载 FBX 时再评估。

- bevy_animation_graph additive/mask 语义若与 ALS layering 不完全一致，在 M6 前验证并决定扩展方式（优先 als-ext 扩展点）。（已于 M6.1 验证通过，记录备查）
- Distance matching / motion matching 类高级特性仅实现 Stop 所需子集（M8.3）。
- ALSExtras（AI 控制器示例）不在范围内。
- 音频（Metasound）、Niagara 特效仅保留挂钩点。ALSCamera 完整移植已纳入 BF116.01。

## BF134 — 全范围 P2 及以上移植差异审计与修复

用户于 2026-10-05 将全部移植范围选为当前任务。按本地 ALS/UE source 与真实 target 消费链逐模块核对；排除附录 A 的刻意差异、网络同步、AI 示例和仅编辑器功能。P0 为核心入口不可用/数据损坏，P1 为常用核心行为错误，P2 为受支持输入、配置或生命周期下的明确行为错误。未经 source 证实的观察不定级、不修复。

- [x] BF134.00 建立 fresh source/target 覆盖清单与差异台账。开始：2026-10-05 06:12 +08:00；预估结束：2026-10-05 06:42 +08:00；实际结束：2026-10-05 06:17 +08:00，偏差 -25 分钟，结果：完成覆盖登记。依据：本地插件 146 个 C++/header、目标 181 个 Rust 文件、已安装 15 AnimBP/递归资产与 BF131–BF133 的真实记录；先确认实际源位置、模块入口、既有 Gate 与刻意差异，不重新运行已归档任务。Gate：覆盖清单包含全部移植模块和未审计边界，台账记录每个确认差异的源函数/资产、触发条件、target consumer、严重级、反例与修复验证。
- [ ] BF134.01 角色状态、输入、运动与动作逐链审计及修复；按发现项拆为独立 Gate/提交，完成 native 算法、参数、坐标与生命周期闭包。
- [ ] BF134.02 动画图、播放器、曲线、同步、通知、根运动与 Control Rig 逐链审计及修复；遵循 animation-graph-equivalence，转换规则修改后全量重转并核对完整 diff。
- [ ] BF134.03 PhysicsAsset/物理时钟、相机、Overlay、关卡、材质与 UI 逐链审计及修复；只有限复开有 fresh source/症状/consumer 证据的旧项。
- [ ] BF134.04 全范围覆盖核销与真实 Playground 最终 Gate；所有确认的 P2+ 项通过自身 Gate 后才可核销总目标。

活动摘要（06:12 +08:00）：仓库起始 HEAD `9105cdf`，工作区干净；实际可读源在插件源码 checkout，工程插件是打包目录的符号链接。BF132 的 49 项与 BF133 的两项已完成，作为漏检边界参考，不凭标题复开。当前只有清单/覆盖登记，尚未宣称任何模块审计通过；下一步从角色和 CMC 的 native 入口建立 source-target 清单。

覆盖 Gate（06:17 +08:00）：当次 inventory 列出全部 146 个源文件（ALS 112/Camera 12/Editor 16/Extras 6）及 SHA-256、181 个 target Rust、15 个 installed root/359 个生成依赖文件，缺失文件 0，连同 15 份 manifest 为 374 个完整生成产物。源码 revision ALS `b754d6f`、UE `396c9f0`；B_Als_Character 原生 reader 39.050 秒退出 0，watchdog 的停滞/子进程/正常完成/构建时效 self-test 全 PASS。台账覆盖八个运行时闭包且明确未核销边界；只完成 inventory，不宣称功能审计完成。下一步 BF134.01 的源刹车归零反例。
