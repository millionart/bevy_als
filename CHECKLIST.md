# bevy_als — 执行清单

> 本文件是唯一权威执行清单；执行纪律见 `agents.md`（仅规则），启动方式见 `goal.md`。
> 完成一项即勾选并一次提交：实现 → 自测 → 勾选 `[x]` → `git commit -m "<前缀><编号>: <条目摘要>"`（勾选与实现同一提交）。
> 基座模块提交前缀沿用 `M<编号>`；缺陷修复清单用 `BF<编号>`。
> 已归档清单：[archivedchecklist.md](docs/archive/archivedchecklist.md)

## BF103 — ALS Notify / Notify State / Marker 事件管线

目标：以 `bevy_animation_graph` 的 `EventTrack`／`EventQueue` 为实际播放时钟，重建当前范围内 ALS 的动画 Notify、Notify State 与标记触发；不得再以角色状态、输入边沿或全局时间模拟动画事件。所有项目语义、源参数映射和消费者保留在主项目／`als-core`，可复用的事件轨道桥接与生命周期机制优先归属 `als-ext`。

- [x] **BF103.01 — N01：原版 Notify／Notify State／Marker 事实清点与导出合同（P1）**
  - 依赖：BF102.Gate 已通过。
  - 执行前：仅在本项成为当前项后，从本地 ALS-Refactored 的实际动画资产、通知类源码和消费者源码重新提取本项事实；不得复用历史记录作为实现依据。
  - 验收：记录当前已导出动画集合中每个实际 Notify、Notify State 与相关 Marker 的资产路径、类型、名称、时间区间、参数、权重／过滤条件和消费者；定义可复现、无 UE 原始资产的项目侧导出格式，并以解析器验证全部记录的时间与引用。
  - 当次 ALS 事实（2026-08-30，本地 ALS Content uasset 与 ALS／ALSCamera C++ 源码实时提取）：137 个动画资产=128 个 AnimSequence+9 个 AnimMontage；177 条通知=161 个 point+16 个 state，另有 60 个 AuthoredSyncMarker 和 9 个 Montage segment。point 类为 `AlsAnimNotify_CameraShake`×6、`AlsAnimNotify_FootstepEffects`×154、`AlsAnimNotify_SetGroundedEntryMode`×1；state 类为 `AlsAnimNotifyState_SetLocomotionAction`×9、`AlsAnimNotifyState_EarlyBlendOut`×6、`AlsAnimNotifyState_SetRootMotionScale`×1。128 个已导出序列全部有目标 glTF；15 个 state 位于 9 个 Montage 容器，不能从现有 glTF 直接取得。CameraShake 仅向本地 player camera manager 发 class+scale；SetGroundedEntryMode 写 animation instance tag；LocomotionAction 在 Begin 写入、End 仅在值仍匹配时清除；RootMotionScale Begin 写 scale、End 仅在未被覆盖时复位 1；EarlyBlendOut 在 branching tick 的源条件成立时按 asset BlendOutDuration 停止 montage。AuthoredSyncMarker 是同步组相位数据，UE 只在组内共有 marker 时使用它，不能默认当作 gameplay Notify。
  - 合同／归属：`docs/bf103-notify-source-audit.{json,md}` 为无 uasset 的事实记录；`docs/bf103-notify-contract.md` 定义 BF103.02 的 JSON sidecar、GraphClip EventTrack 编译和 point 跨越规则。通用桥接／生命周期归属 `als-ext`，ALS 类语义和消费者留在项目；现有 `assets/sync/als_sync_metadata.ron` 继续承载同步 marker，不复制为全局计时器。
  - 证据：`python tools/import/audit_als_animation_events.py --content-root <ALS Content>` 输出 `animation_count=137`、`event_count=177`、`marker_count=60`、`montage_segment_count=9`、`failure_count=0`，且 event／marker time 与引用验证均为 `true`。

- [x] **BF103.02 — N02：Notify 资产侧数据生成与完整性校验（P1）**
  - 依赖：BF103.01。
  - 执行前：重新从本地 ALS-Refactored 提取本项所覆盖资产的通知事实与导出边界。
  - 验收：每个已审计且在范围内的动画都有与其真实播放片段对应的项目侧事件数据；批量校验检测缺失、重复、越界时间、非法区间和不匹配片段。数据不得依赖角色全局时间或手写的按状态定时表。
  - 当次 ALS 事实（2026-08-30，重新运行本地 uasset 审计）：结果仍为 137 个动画资产、177 条通知、60 个 marker、9 个 Montage segment，所有时间和引用有效。源 event GUID 在复制的动画记录间不是全局唯一，故运行时 event ID 必须以 source asset+GUID 共同确定；9 个 Montage segment 均引用当前已导出的序列 GraphClip，slot 与 source segment 时间／rate／loop 均需保留。
  - 实现：`tools/import/generate_als_notify_manifest.py` 从审计 JSON 确定性生成 `assets/animation_events/als_notify_manifest.json`（v1），保留 point／state 原始区间、class／parameters、weight／filter、同步 marker 和 Montage→GraphClip 映射；生成时验证缺失 GraphClip、未映射 Montage、重复 ID、越界／倒置时间与非法 marker。`assets/docs/export-manifest.md` 已登记此项目侧导出物。
  - 证据：生成器输出 `clips=137 events=177 markers=60`；复查 event ID 唯一数=177、9 个 Montage 均有存在的 runtime segment GraphClip。`python tools/import/test_generate_als_notify_manifest.py` 7/7 通过，覆盖精确 point 时间、重复 ID、越界、倒置区间、缺失 GraphClip、Montage 映射及虚拟源路径解析；`python tools/import/generate_als_notify_manifest.py --check` 通过。

- [x] **BF103.03 — N03：`bevy_animation_graph` EventTrack／EventQueue 通用桥接（P1）**
  - 依赖：BF103.02。
  - 执行前：重新提取本项涉及 ALS 动画时间、混合权重和事件消费事实，并核对当前锁定的 `bevy_animation_graph` 公共接口。
  - 验收：项目侧事件数据进入相应 GraphClip 的 `EventTrack`，由实际 `ClipNode` 时间采样并经 `EventQueue` 输出；实例化、循环、seek、混合权重和多个角色之间互不串扰。通用桥接归属 `crates/als-ext`，不得修改依赖 fork。
  - 当次事实（2026-08-30，重新运行本地 ALS 审计并读取锁定 `bevy_animation_graph` 源码）：源数据仍为 177 条 event／60 个 marker；`GraphClipSerial.event_tracks` 直接载入 GraphClip，`ClipNode` 以 `partial_update_with_tracks()` 处理 event seek、以 `sample_tracks()` 输出 `EventQueue`，区间采样为 `[start_time,end_time)`。内建 `LoopNode` 和 `BlendNode` 不将 EventQueue 作为输出传播，故若直接依赖它们会丢失事件／权重；这属于公开扩展点能力缺口，不修改 fork。
  - 实现／归属：`tools/import/compile_als_event_tracks.py` 将 manifest 177 条记录编译为 65 个 GraphClip 的 98 条 EventTrack，source asset+GUID 生成稳定事件 ID，Montage 时间经 segment 映射到真实 sequence GraphClip。`crates/als-ext` 新增通用 `EventQueueBlendNode`、`EventLoopNode`，并令 `RootMotionClipNode` 与 `TimePositionNode`传递 EventQueue／PercentOfEvent；`roll_slot` 和 `state_roll` 图暴露实际 `events` 输出。所有实现使用 `bevy_animation_graph` 公共接口，无依赖 fork 改动。
  - 证据：`python tools/import/compile_als_event_tracks.py --check` 输出 `graph_clips=65 tracks=98 events=177`；`cargo test -p als-ext --test notify_event_tracks --jobs 2` 9/9 通过（真实 `.anim.ron` 反序列化、全部编译记录、ClipNode 输出、seek、独立上下文、根运动 clip）；`cargo test -p als-ext --lib --jobs 2` 184/184 通过（含 queue weight 与 loop forwarding）；`BEVY_ASSET_ROOT=<repo-root> target/debug/examples/ab_als_contract_gate.exe` exit=0。

- [x] **BF103.04 — N04：Notify State 生命周期与 Marker 触发适配（P1）**
  - 依赖：BF103.03。
  - 执行前：重新提取原版通知状态和标记在开始、持续、结束、循环／跳转及混合时的实际语义。
  - 验收：在项目运行时为同一角色、同一轨道可靠地产生一次 Begin、连续 Active 和一次 End，且点式 Marker 仅在真实跨越时间点时触发一次；对循环、跳帧、反向／重置和低权重过滤的处理有可复查的数值 Gate。可复用机制归属 `crates/als-ext`。
  - 当次 ALS／UE 事实（2026-08-30，重新读取本地 ALS NotifyState 类、AnimSequenceBase／AnimMontage／AnimInstance／AnimNotifyQueue 源码并重跑资产审计）：队列先以 leader／follower、dedicated-server、weight threshold、LOD 与 chance 过滤；Notify State 在同一 source instance 内去重，旧状态 End 先于新状态 Begin。标准 sequence 正放依据 `start<=current && end>previous`，反放依据 `start<previous && end>=current`，整段跳过仍作为 Begin+End；原版 native branching state 的活动区间为 `(start,end]`。当前 ALS 177 条记录均 chance=1、均不在 follower 触发、160 条禁 dedicated server、无 converted branching point；`SetLocomotionAction`／`EarlyBlendOut` 的类构造启用 native branching，RootMotionScale 使用常规状态语义。Engine SyncMarker 只在有效同步组记录本帧 crossed marker，不能按角色全局时钟模拟。
  - 实现／归属：`crates/als-ext::EventLifecycle` 以每实例 active state 维护 Begin／Active／End，按 source policy 去重和过滤；支持标准 `[start,end)` 与 native `(start,end]` 两种边界、完整跳帧脉冲、正反向 wrap、精确 point／marker crossing 和 reset End。其输入仅为 EventQueue、事件定义和实际 clip clock，不含 ALS tag／类名或角色规则；ALS 的 class 映射留给 BF103.05。
  - 证据：`cargo test -p als-ext --lib event_lifecycle --jobs 2` 8/8 通过，覆盖 lifecycle、threshold、point／marker、正反 loop、jump、native boundary、follower／dedicated、reset 与隔离；`cargo test -p als-ext --lib --jobs 2` 192/192 通过。

- [ ] **BF103.05 — N05：ALS Notify 消费者接线并移除临时代码路径（P1）**
  - 依赖：BF103.04。
  - 执行前：重新从本地 ALS-Refactored 提取每个本项消费的 Notify／Notify State 类、参数与角色／相机行为，明确范围内外差异。
  - 验收：范围内 ALS 专用消费者只消费实际 GraphClip 事件，不从 gait、输入或全局时间推断；现有 `AlsCameraAnimationNotifyDriver` 临时定时路径被真实事件路径取代或删除。范围外通知显式登记原因，不得静默丢弃。

- [ ] **BF103.06 — N06：真实入口场景与可复查遥测（P1）**
  - 依赖：BF103.05。
  - 执行前：重新提取本项实际演示场景涉及的 ALS 触发事实与预期时序。
  - 验收：`run_manual.bat` 的 `manual_playground` 在可见原始 Playground 地形、正常角色和相机中可重复触发点式 Notify 与 Notify State；HUD／probe 记录 clip、轨道、事件、时间、权重、生命周期和消费者结果。自动场景不离开 Playground，运行日志给出逐项 PASS。

- [ ] **BF103.Gate — Notify／Notify State／Marker 全链路验收（P1）**
  - 依赖：BF103.01–BF103.06。
  - 验收：从项目侧导出数据到实际 GraphClip、EventQueue、生命周期、ALS 消费者和 `manual_playground` 的完整链路都有真实运行日志；覆盖至少一个点式 Notify、一个 Notify State、一个 Marker、循环与多角色隔离，确认不存在临时状态／全局时间替代路径。

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
