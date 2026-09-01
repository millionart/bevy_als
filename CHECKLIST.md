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
─► BF103 Notify 事件管线（已归档） ─► BF104 方向／步态闭环（已归档） ─► BF105 蹲起事务（已归档）
─► BF106 Foot IK 闭环（已归档） ─► BF107 状态机语义与持续回归（按用户指令归档，未完成）
─► BF108 BlendSpace 运行图等价（已归档）
```

## BF110 结构化动画上下文图资产（当前执行批次）

范围限定为本项目的动画图作者资产、离线展开／审计工具和 `manual_playground` 集成验证。目标是把同一动画分支的 pose 与已声明曲线 ABI 作为一个结构化作者连接表达，离线确定性展开为现有 `bevy_animation_graph` 可读取的 RON edge；运行时继续消费生成后的普通图资产。不得修改 `bevy_animation_graph`、`bevy_animation_graph_editor`、Cargo dependency revision 或 `docs/als-ref-animation-asset-map.md`；`NodeTime`、事件和独立控制输入不得被错误并入上下文。每次迁移必须逐字段证明结构化源与展开图的 pose、曲线、time 和控制 edge 等价，生成器必须可重复运行且不产生未追踪运行时变换 bridge。

- [ ] BF110.01 动画上下文作者合同：建立项目内结构化连接 schema、curve ABI 清单与静态解析／自测，明确 pose-context、time、event、控制输入的边界。验收：合同拒绝未声明曲线、重复字段、跨上下文混接和非法 time/event 打包；对当前 `AB_Als` 根图建立完整基线清单，且不修改生成图。
- [ ] BF110.02 离线展开器与审计：实现结构化源图到标准 `.animgraph.ron` 的确定性展开、只读等价审计和幂等性检查；生成物保持 editor 元数据、node position、input order 和现有 runtime ABI。验收：最小 fixture 与 malformed fixture 全部自测通过，展开图可通过既有 editor asset validator，审计逐字段报告零差异。
- [ ] BF110.03 `AB_Als` 根图试迁移：把 Standing、Crouching、Grounded、两个 Slot、Overlay/Inertialization 等同一分支的 pose/curve edge 迁入结构化作者源，生成正式根图；保留所有独立 pose 分支、`NodeTime`、event 与控制 edge。验收：展开结果与迁移前基线的 node、position、edge、pin、curve/time/control ABI 精确等价，根图静态合同与运行时 contract gate 通过。
- [ ] BF110.04 总 Gate：全量运行结构化展开／审计、graph asset fix/read-only validator、相关 Rust／Python 自测、`AB_Als` contract gate 和真实 `run_manual.bat` Playground 路径。验收：生成物干净、无依赖仓库改动、无未解释 ABI 差异，真实入口记录结构化来源已执行且最终 pose 链 PASS。

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
