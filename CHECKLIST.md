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

## BF102 — 有限取证包的逐域验证批次

本批次登记核实包 U01–U16 的全部“未完成验证”领域。它们是怀疑对象与审计任务，不是已确认缺陷；创建本段不授权提前提取源码、查询资产、实现或验收。每项成为当前依赖顺序中的执行项时，必须重新从 ALS-Refactored 提取当次事实并取得 Bevy 运行证据。结论只能是“确认缺陷并新增修复项”“已有等价实现／不适用并归档证据”或“证据不足，保留待验证”。

```
BF101.Gate ─► BF102.1 U15 基座能力 ─► BF102.2 U14 资产事实
                                      ├► BF102.3 U01 状态 ─► BF102.4 U02 调度
                                      ├► BF102.5 U10 曲线 ─► BF102.6 U11 动画底座
                                      │                         ├► BF102.7 U05 转身
                                      │                         ├► BF102.8 U03 Foot Lock
                                      │                         ├► BF102.9 U04 Ground Prediction
                                      │                         ├► BF102.10 U06 Dynamic Transition
                                      │                         ├► BF102.11 U07 Mantle
                                      │                         ├► BF102.12 U08 Roll
                                      │                         ├► BF102.13 U09 Ragdoll
                                      │                         └► BF102.14 U12 相机／BF102.15 U13 效果
                                      └► BF102.16 U16 网络范围 ─► BF102.Gate
```

- [ ] **BF102.1 — U15：核验两个底座仓库的公开能力与扩展边界**
  - 依赖：BF101.Gate。
  - 验收：固定实际锁定版本，逐项记录公开扩展点、已有能力与缺口；确认缺口后才可在 `als-ext` 创建修复项。

- [ ] **BF102.2 — U14：核验 uasset、AnimBP 与 PhysicsAsset 的导出事实**
  - 依赖：BF102.1。
  - 验收：从当前 ALS-Refactored 工程提取当前执行领域所需的资产事实并登记 export manifest；确认是否可由已导出资产表达。

- [ ] **BF102.3 — U01：核验完整状态与动作转换合同**
  - 依赖：BF102.1、BF102.2。
  - 验收：给出状态、动作、输入、守卫和输出的双端表；每个差异都有源码与运行探针，确认缺陷另建修复项。

- [ ] **BF102.4 — U02：核验全局 ECS 调度与跨帧依赖**
  - 依赖：BF102.3。
  - 验收：记录输入、状态、物理、动画和 IK 的实际执行顺序；探针证明或排除一帧延迟与竞争。

- [ ] **BF102.5 — U10：核验曲线键值与混合语义**
  - 依赖：BF102.1、BF102.2。
  - 验收：逐个当前领域曲线建立名称、键值、单位、采样和消费链证据；缺失曲线仅在确认可达影响后修复。

- [ ] **BF102.6 — U11：核验动画同步组、slot、notify 与 root motion 底座**
  - 依赖：BF102.1、BF102.2、BF102.5。
  - 验收：对每项能力取得底座 API、图资产和运行输出证据；确认能力缺口时优先登记 `als-ext` 适配项。

- [ ] **BF102.7 — U05：核验 Turn/Rotate in Place 的最终图与角色旋转**
  - 依赖：BF102.3、BF102.4、BF102.6。
  - 验收：同一输入轨迹下记录转身触发、动画相位、角色 yaw 和结束恢复；确认偏差后新增修复项。

- [ ] **BF102.8 — U03：核验 Foot Lock、pelvis 与 ControlRig 等价闭环**
  - 依赖：BF102.4、BF102.5、BF102.6。
  - 验收：记录锁点空间、骨盆补偿、角色移动补偿和运行骨骼探针；确认缺失后新增 `als-ext` 或本体修复项。

- [ ] **BF102.9 — U04：核验 Ground Prediction 完整闭环**
  - 依赖：BF102.4、BF102.5、BF102.6。
  - 验收：记录垂直速度、碰撞距离、曲线输入和预测输出；以运行日志确认或排除偏差。

- [ ] **BF102.10 — U06：核验 Dynamic Transition 的完整消费链**
  - 依赖：BF102.3、BF102.5、BF102.6、BF102.8。
  - 验收：记录触发误差、资源选择、重入守卫和最终姿态；确认缺陷后新增修复项。

- [ ] **BF102.11 — U07：核验 Mantle 的探测、移动基座、动画、位移与恢复**
  - 依赖：BF102.3、BF102.4、BF102.6。
  - 验收：逐阶段运行探针记录目标、动作、root motion、碰撞与恢复状态；确认缺口后新增闭环修复项。

- [ ] **BF102.12 — U08：核验 Roll 的 root motion 与结束恢复**
  - 依赖：BF102.3、BF102.4、BF102.6。
  - 验收：运行探针记录触发、位移、碰撞、姿态与结束状态；确认偏差后新增修复项。

- [ ] **BF102.13 — U09：核验 Ragdoll 的进入、物理、起身与速度继承**
  - 依赖：BF102.3、BF102.4、BF102.6。
  - 验收：运行探针记录进入条件、骨盆跟踪、朝向、起身资源、控制器重新接管与速度；确认偏差后新增修复项。

- [ ] **BF102.14 — U12：核验相机完整行为及其 locomotion 输入合同**
  - 依赖：BF102.3、BF102.4、BF102.5。
  - 验收：记录视角模式、yaw、lag、遮挡、FOV 曲线与状态输入；确认差异后新增修复项。

- [ ] **BF102.15 — U13：核验足音与表面效果的动画事件合同**
  - 依赖：BF102.2、BF102.6。
  - 验收：记录 notify、脚选择、表面、声音和效果挂钩；项目范围外项目标为不适用并归档依据。

- [ ] **BF102.16 — U16：确认网络语义的项目范围与确定性边界**
  - 依赖：BF102.1。
  - 验收：明确单机范围外的网络功能不会被误记为缺陷；仍记录会影响单机确定性的本地行为合同。

- [ ] **BF102.Gate — 逐域验证批次总验收**
  - 依赖：BF102.1–BF102.16。
  - 验收：每个 U 项均有可复查的三态结论与真实运行日志；确认缺陷均有独立修复项和 Gate；未确认领域不被宣称已完成。

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
