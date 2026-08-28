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

## BF101 — 2026-08-29 有限源码取证修复批次

来源是经 SHA-256 核对的 2026-08-29 重建取证包。该包不是全量审计：仅 F001、F002 是可直接修复的局部错误；F003 是函数级不等价但调用影响未确认；R001–R003 是必须先获取运行证据的风险。未验证领域不得列为缺陷或据此实现功能。

```
BF101.1 F001 规范世界移动方向 ─► BF101.3 R003 台阶辅助物理集成验证 ─┐
BF101.2 F002 挤压恢复垂直重叠保护 ───────────────────────────────────┤
BF101.4 R001 站起胶囊体积安全验证／修复 ──────────────────────────────┤──► BF101.Gate
BF101.5 R002 方向相关速度数据合同追踪 ─────────────────────────────────┤
BF101.6 F003 播放率消费者与连续权重追踪／修复 ─────────────────────────┘
```

- [x] **BF101.1 — F001：让台阶辅助消费规范化的世界移动方向**
  - 依赖：无。
  - 目标：移除台阶辅助对视角相对输入的重复解释，使其与 locomotion 使用同一方向合同。
  - 验收：0°、90°、180°和 -90°视角 yaw 下，前进输入的辅助探测方向与普通移动方向数值一致；保留无输入、低速和冷却守卫。

- [x] **BF101.2 — F002：挤压恢复只处理真实垂直重叠**
  - 依赖：无。
  - 目标：在选择恢复地板前同时验证角色胶囊与标记地板的垂直区间相交。
  - 验收：上方、X/Z 投影重合但垂直分离的标记地板不会改写角色位置或 recovery 计数；真实穿入仍被安全地推出，Position、Transform 和速度保持同步。

- [ ] **BF101.3 — R003：验证台阶辅助的物理集成与状态同步**
  - 依赖：BF101.1。
  - 目标：以真实 Avian/Tnua 运行日志验证视角相对台阶探测、Transform/Position 同步和目标胶囊安全性；若失败，仅修复实证的缺口。
  - 验收：四向视角台阶探针记录一致的世界方向、无非有限值、无残留穿透，并输出可复查 verdict。

- [ ] **BF101.4 — R001：验证站起时目标胶囊的体积安全性**
  - 依赖：无。
  - 目标：在偏离中心线但进入站姿胶囊半径的障碍物下运行实际物理探针；只有实证显示中心射线不足时才替换为体积检查。
  - 验收：探针日志明确记录站姿是否被拒绝、最终 collider、Position/Transform 和穿透状态；若修改，偏心横梁不能允许角色进入重叠状态。

- [ ] **BF101.5 — R002：追踪方向相关速度的数据合同**
  - 依赖：无。
  - 目标：在开始任何数据模型改动前，从 ALS-Refactored 实时重新提取开关、默认值和消费链，并确认 Bevy 当前是否已有等价实现。
  - 验收：默认关闭时现有运动合同保持不变；只有确定缺少可达消费链时才新增可关闭的适配与数值 Gate。

- [ ] **BF101.6 — F003：追踪播放率消费者并恢复连续 gait 权重语义**
  - 依赖：无。
  - 目标：先追踪 `als_play_rate` 是否到达完整角色动画图；若可达，重新从 ALS-Refactored 提取当次函数事实，再把离散 gait 信号替换为等价的连续权重数据流。
  - 验收：运行探针证明消费者与输入来源；仅在可达时验证中间权重与播放率连续、有限且与固定 ALS 参考样本一致。

- [ ] **BF101.Gate — 有限取证批次总验收**
  - 依赖：BF101.1–BF101.6。
  - 验收：每项都有真实运行日志 verdict；相关单元／集成 Gate 与 workspace 测试通过；隐私扫描无命中；未验证领域仍明确保持未定性。

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
