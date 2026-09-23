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

## BF124 — v10 移植差异反馈逐项核查与修复

来源：[复核移植差异：v10 全部 55 项正文](https://chatgpt.com/g/g-p-6a91ef049cac81919416c69792b9eb10/c/6ab0f900-8b94-83ea-9704-87bc63201172)；报告对照目标 `61d4dbe2472245735be83f4f683b142fffc285f7`、源端 `b754d6f0f2bb03741d301f8fb88077ebfe561e17`。以下为反馈索引，不把历史模型反例当作当前检出版本的实机复现；报告给出的 53 项 open、A26/A33 代码关闭及 A34 部分关闭均照录。按编号逐项激活执行条目；每项先重新读取本地源与当前目标，核对触发和影响，再留下最小失败验证、修复根因、做消融及对应 Gate，单项通过后单独提交并更新索引。涉及转换规则时执行全部已生成 AnimBP 及递归依赖的全量重转换和新旧产物 diff；涉及图或姿态时执行公共视觉 Gate。

| 编号 | 级别 | 报告状态 | 反馈 |
|---|---|---|---|
| A01 | P1 | open | 角色运动完整帧时间与封顶物理时间不一致 |
| A02 | P2 | open | 旋转响应角速度使用弧度每秒却除以角度每秒阈值 |
| A03 | P2 | open | 第一人称非瞄准遗漏身体瞄准旋转和限角分支 |
| A04 | P2 | open | 空中旋转沿用地面目标选择而非源端独立策略 |
| A05 | P2 | open | 连续 GaitAmount 旋转曲线被离散 Gait 半衰期取代 |
| A06 | P2 | open | 落地旋转阻止标志被残余 moving 状态解除 |
| A07 | P2 | open | VelocityDirection 缺少碰撞调整前期望速度朝向 |
| A08 | P2 | open | 动作期间遗漏目标 yaw 和瞄准约束的生命周期维护 |
| A09 | P2 | open | Roll 回退阻尼遗漏接近 180 度的源端转向偏好 |
| A10 | P2 | open | 蹲起中心位移和站起查询未使用有效缩放 |
| A11 | P2 | open | Ragdoll 找地查询未保留角色原始碰撞响应 |
| A12 | P2 | open | Ragdoll 找地和胶囊跟随使用未缩放尺寸 |
| A13 | P2 | open | Avatar 或骨骼替换后脚骨缓存不重建 |
| A14 | P2 | open | Turn 播放栈跨动画播放器替换残留 |
| A15 | P2 | open | 晚加载的贴花排除标记未传播到已处理子网格 |
| A16 | P2 | open | PlayerStart 占用查询排除角色且同批出生不预留空间 |
| A17 | P2 | open | 相机把基座存在直接当作相对旋转开启 |
| A18 | P2 | open | 相机切换 Avatar 时保留仍然存活的旧 root 骨骼缓存 |
| A19 | P2 | open | Ragdoll 按角色 ID 选择首个 Avatar 而非当前 Assembly Avatar |
| A20 | P2 | open | 运行时选骨设置更新 Control Rig 却不更新脚锁选择器 |
| A21 | P2 | open | Head 去时间缩放的输入未包含角色自定义倍率 |
| A22 | P2 | open | Turn 结束的惯性混合请求没有接入实际生成图 |
| A23 | P2 | open | Ragdoll 开始没有停止全部 Turn／Transition 播放栈 |
| A24 | P2 | open | Grounded Slot Group 没有对 Turn 与 Transition 实施跨栈互斥 |
| A25 | P2 | open | 脚步事件仅用取模前后时间的大小关系判断回环，丢失整圈信息 |
| A26 | P2 | closed | Mantle 起点穿透命中可把身后台阶选为前向攀爬目标 |
| A27 | P2 | open | 惯性混合曲线导数重复扣除新目标值 |
| A28 | P2 | open | 已播放 Turn／Transition 的实际 Slot 采样持续落后播放时钟一帧 |
| A29 | P2 | open | 动画加速度差分遗漏相对运动基座的上一帧速度旋转补偿 |
| A30 | P2 | open | 同步动画节点使用全局帧增量，绕过播放器 Pause 的零增量 |
| A31 | P2 | open | 退瞄脊柱平滑没有按运动基座旋转更新历史世界 yaw |
| A32 | P2 | open | Sprint 相机震动仍用手算原始权重/时钟并默认每条记录都是 leader |
| A33 | P2 | closed | 调试文本变化时在可见性传播之后重建整棵 UI，存在连续不可见的合法调度 |
| A34 | P2 | 部分关闭 | 足部 IK 与脚印查询把忽略当前角色扩大成忽略全部角色及其布娃娃 |
| A35 | P2 | open | 布娃娃物理装配未保留角色的正均匀缩放：碰撞几何、局部偏移与关节锚点仍为单位尺寸 |
| A36 | P2 | open | 真实 Playground 启动把场景夹具假设作为运行门槛：多个 PlayerStart 或非零地面高度可阻止角色启用 |
| A37 | P2 | open | 播放器组件同实体替换后遗留 PhysicsProfileApplied，令新布娃娃跳过 ALS 物理配置 |
| A38 | P2 | open | 跳跃速度按固定 9.81 换算高度，却按实际重力反算起跳速度 |
| A39 | P2 | open | 同图、同节点热替换 RigDefinition 时沿用旧 VM 内存和层级，导致旧常量生效或寄存器布局报错 |
| A40 | P2 | open | 贴花接收器同帧换材质并退出接收时，新材质被旧源材质覆盖 |
| A41 | P2 | open | Transition 表达式生成器保留 Reroute 的 InputPin，运行时 passthrough 却读取 A 并静默回落为零/假 |
| A42 | P2 | open | Transition 比较阈值的单位转换只检查直接上游，经过 Absolute 后丢失速度单位 |
| A43 | P2 | open | 布娃娃播放器脱离 Avatar 父链后，渲染记录找不到角色，导致可见布娃娃退出调试轮选候选集 |
| A44 | P2 | open | FSM 将 Machine Weight 恒写为 1，改变实际 Standing 起跑迁移分支 |
| A45 | P2 | open | Sequence Player 接受 bIgnoreForRelevancyTest 却丢弃标志，使被排除动画仍主导自动迁移计时 |
| A46 | P2 | open | Sequence Player 的入组 leader 位置覆盖标志止于 IR，marker 同步仍强制继承上一组相位 |
| A47 | P2 | open | ExclusiveAlwaysLeader 落选后未退出同步组，而被当作普通 follower 覆盖播放相位 |
| A48 | P2 | open | TwoBoneIKSimplePerItem 的 Direction pole 被当作绝对位置，破坏骨链在 Rig 全局空间的平移不变性 |
| A49 | P2 | open | 同步计划把 Sequence 的秒数起点误限制为 0～1，拒绝合法的长序列起播位置 |
| A50 | P2 | open | Two Way Blend 的激活重置只接受直接 Sequence Player，使合法子树重激活报错 |
| A51 | P2 | open | Curves Blend 按曲线模式选择相关动画，丢失真实更新权重并改变自动迁移 |
| A52 | P2 | open | BlendSpace Player 未发布相关动画计时，依赖它的 FSM 自动迁移持续拿到空值 |
| A53 | P2 | open | 分层骨骼混合的 mesh-space 路径把有效基础根运动清零，未按 root 骨权重合成 |
| A54 | P2 | open | 分层骨骼混合不合并叠加分支的惯性请求，导致下游请求消失或时长错误 |
| A55 | P2 | open | Sequence Evaluator 属性绑定绕过实例属性与 source_frame_input，采到旧值或错误回退 |

- [x] **BF124.A01-S1 — 物理时间合同与最小失败复现**：开始：2026-09-23 03:44:11 +08:00；预估结束：2026-09-23 04:29:11 +08:00；实际结束：2026-09-23 04:02:22 +08:00；偏差：-26m49s；结果：source/static 与反例 Gate 通过。估时依据：一个源端运动函数、一个目标时钟入口、四个直接调用模块与归档物理 fixture 的静态闭环记录。拆分结果：角色/刚体时间消费修复及真实入口 Gate 分别留待后续条目。

  | 时间消费者 | 本地 UE 5.8.1 源合同 | 当前目标 |
  |---|---|---|
  | 角色运动 | `ControlledCharacterMove` 将整帧 `DeltaTime` 交给 `PhysWalking`；`GetSimulationTimeStep` 分步，最后一步消费剩余时间 | Tnua 在 Update 计算运动，Dynamic 角色在 PostUpdate 经 Avian 积分时只收到封顶时间 |
  | 普通刚体 | 项目未覆写 `bSubstepping`，Chaos `SetUpForFrame` 将单帧物理时间封顶于 `1/30` | `SourcePhysicsTimingPlugin` 将传给 Avian 的 `Time` 封顶于 `1/30` |
  | 动画/帧时钟 | 角色更新仍使用整帧时间 | PostUpdate 物理之后恢复原始 `Time` |

  目标 `AlsCharacterBundle` 是 `RigidBody::Dynamic`；Avian `run_physics_schedule` 直接消费当次封顶 Delta，无积累补跑。100 ms、3.75 m/s 的隔离角色体测试先稳定失败：位移 `0.124999985 m`，源合同期望 `0.375 m`；只从夹具移除封顶插件后通过，证明封顶是该反例的必要原因。旧测试仅断言封顶数值，未覆盖角色位移。反例留为带原因的 ignored 测试，普通 focused 测试 1 passed/1 ignored；尚未运行真实 Playground。

- [x] **BF124.A01-S2a — 角色整帧积分的最小公开扩展点**：开始：2026-09-23 04:03:34 +08:00；预估结束：2026-09-23 04:43:34 +08:00；实际结束：2026-09-23 04:18:30 +08:00；偏差：-25m04s；结果：公开扩展点及碰撞边界 Gate 通过。估时依据：S1 已闭合源时钟，一个 Tnua 马达入口和 Avian 积分/接触阶段，复用隔离夹具。拆分结果：生产接线、模式边界及真实入口 Gate 留给后续条目。

  锁定 Avian 的 `CustomPositionIntegration` 可跳过默认角色位置积分，公开 `SubstepSchedule`/`SolverBody` 可按整帧 Delta 积分，普通刚体仍按 `1/30` 封顶。只替换积分的消融中，自由角色 `0.375 m`、普通刚体 `0.125 m`，但隔离墙面穿透到 `x=0.375 m`、地面落到 `y=0.825 m`；实际 ALS 地面马达的现有前扫挡住墙面，空中落地仍到 `y=0.805925 m`。再用公开 `SpatialQuery` 对角色每个子步的候选位移扫掠，四个 focused 夹具通过：自由/普通刚体时间、墙面/地面、实际地面马达与斜向沿墙移动、空中落地后持续五帧收敛。去掉扫掠即重现穿透；测试原型已消融删除，未改生产时钟。下一段需核对 Grounded/Air/Custom/Ragdoll 的积分所有权、动态接触与 root motion，再接入最小生产实现。

- [x] **BF124.A01-S2b1 — Grounded/Air 整帧角色积分与接触**：开始：2026-09-23 04:20:48 +08:00；预估结束：2026-09-23 05:10:48 +08:00；实际结束：2026-09-23 05:22:25 +08:00；偏差：+11m37s；结果：focused Gate 通过。估时依据：S2a 的两个已通过原型及一个角色束/一个物理调度点。拆分结果：Custom/Ragdoll、动态接触组合及真实 Playground 继续独立验收。

  本地 UE `PhysFalling` 命中后按剩余时间沿表面继续移动，`PrimitiveComponent::PullBackHit` 留出扫掠余量；复用既有 `als-core` 扫掠公式，未新增另一套阈值。长帧仅为尚未被别的系统接管的 Grounded/Air 角色临时持有 `CustomPositionIntegration`，逐子步按完整帧位移扫掠，物理结束归还；短帧和普通刚体保持原 Avian 积分与 `1/30` 封顶。最小反例解除 ignored 后先以 `0.124999985 m` 失败；修复后 120/60/30/20/15/10 FPS 与 200 ms hitch 的自由位移及普通刚体封顶通过。实际 ALS 马达夹具覆盖地面墙阻/沿墙移动、空中沿墙移动、落地后五帧收敛；Playground 楼梯/移动平台 3 项、Mantle 时钟/临时位置所有权 1 项、运动基座 1 项均通过（Cargo `--jobs 2`）。消融确认：去掉扫掠会穿墙/穿地；去掉源端余量会出现落地微穿透；永久接管破坏 Mantle，保留默认积分再补余量仍会穿地。尚未运行真实 Playground；动态接触、动作模式和可见角色时钟留给后续 Gate。

- [x] **BF124.A01-S2b2 — Custom/Ragdoll 时间与积分所有权**：开始：2026-09-23 05:23:22 +08:00；预估结束：2026-09-23 06:03:22 +08:00；实际结束：2026-09-23 05:34:29 +08:00；偏差：-28m53s；结果：模式所有权 Gate 通过。估时依据：S2b1 已限制临时所有权，复用 Mantle 多配置夹具和 Ragdoll 生命周期测试。拆分结果：运动基座上角色的相对速度长帧补偿、动态接触与真实 Playground 独立验收。

  | 模式 | 本地源端 | 当前目标所有权 |
  |---|---|---|
  | Grounded/Air 普通运动 | CMC 使用完整帧并扫掠 | 仅 Dynamic、无 Ragdolling 动作及既有位置接管时临时使用整帧积分 |
  | Mantle/Custom | `PhysCustom` 完整 Delta 且不扫掠；动作自身驱动根位移 | `mantle_drive_system` 逐帧写位置并持有 `CustomPositionIntegration`，时钟适配器不重复接管 |
  | Ragdoll | 胶囊禁用碰撞，物理 mesh 使用封顶 Chaos 时间，胶囊跟随 pelvis | 主胶囊切 Kinematic、Tnua 禁用；即便状态仍是 Grounded，Ragdolling 动作也禁止整帧积分 |

  Ragdoll 入口最小反例先以 `0.18749999 m` 失败（应为封顶 `0.12499999 m`），按 body/action 双条件修复后，退出并恢复 Dynamic/None 的下一长帧又回到 `0.375 m`。Mantle 结束时的移动基座回归揭示 support velocity 不可按角色整帧重复积分：无基座区分会产生 `0.05 m` 水平相对误差；临时跳过带非零 support velocity 的角色后恢复通过。此分支消融有明确失败边界，基座上的相对自驱运动仍待下一项补齐。聚焦时钟 3 项、Mantle 两项及 Ragdoll 退出一项通过，均使用 `--jobs 2`；未启动真实 Playground。

- [x] **BF124.A01-S2b3 — 移动基座相对运动与动态接触**：开始：2026-09-23 05:35:44 +08:00；预估结束：2026-09-23 06:25:44 +08:00；实际结束：2026-09-23 05:46:16 +08:00；偏差：-39m28s；结果：源/目标与 focused Gate 通过。估时依据：S2b2 的无输入携带反例与现有运动基座/Mantle 夹具。拆分结果：真实 Playground 的可见角色和时钟、动画/root-motion、接触事件仍待 runtime Gate。

  本地 UE `MaybeUpdateBasedMovement` 对模拟基座在物理后更新相对位置，`PhysWalking` 仍消费完整角色帧；目标 `support_velocity_from_base` 发布基座接触点速度，`locomotion_velocity` 将其从角色世界速度中分离，`follow_movement_base_system` 已处理封顶物理时间内的基座携带。移动基座上带输入的 100 ms 反例先仅移动相对 `0.1257577 m`；只对相对自驱速度补齐未由 Avian 积分的时间后，相对位移 `0.3772726 m`，与当帧马达/接触后的有效相对速度一致。无输入携带、带输入移动、停止、Kinematic→Dynamic 基座切换及动态接触夹具均通过；普通刚体仍封顶。消融去掉相对补偿复现原失败，改为补整份世界速度则重复计算基座移动。聚焦时钟 3 项及 Mantle 动作返回/移动基座回归通过（`--jobs 2`）；原 Playground 尚未启动。

- [x] **BF124.A01-S3a — 真实 Playground 启动与图姿态基线**：开始：2026-09-23 05:47:26 +08:00；预估结束：2026-09-23 06:32:26 +08:00；实际结束：2026-09-23 05:55:18 +08:00；偏差：-37m08s；结果：公共 startup 功能 Gate 通过，性能预算明确延期。估时依据：S2 的 source/static 与 focused 闭环、既有 startup runner/validator 及同目标目录构建。拆分结果：运动多帧率/卡顿与真实 root-motion、接触事件仍待后续独立 Gate。

  runtime 前摘要：本地 UE CMC/Chaos 时钟、Mantle/Ragdoll 和运动基座源闭包及目标三种模式/基座 focused Gate 已记录于 S1–S2b3；本轮未修改图、转换器或资产。`validate_runtime_graph_asset_boundary.py --check` 无 issue，公共视觉 validator `--self-test` 全项通过；与启动器同目录的 `target_game` 以 `cargo build --profile play --example manual_playground --jobs 2` 通过。当前未通过边界是原 Playground 的可见角色 startup 报告，先运行无驱动 `startup`，不先启动运动用例。

  首次 startup 后摘要：原入口进程退出 0、报告 ready，601 帧 root/姿态、物理单步、时序、渲染/贴花及 watchdog 均无功能 issue；完整验收唯一失败为动画求值 p95 `4.374 ms` 超过原 `4.0 ms` 预算。最近多个旧 startup 报告在本次代码之前已处于约 `4.2–4.6 ms` 同一带宽；归档记录用户明确暂停性能工作并继续使用 `FunctionalOnly`。保留首次完整失败报告和原阈值，不把它改称性能通过；下一次以独立输出目录运行公共 `-FunctionalOnly` startup，再进入运动场景。

  functional startup 后摘要：同一 `target_game/play` 二进制、原 Playground、`CaptureCount=0` 的第二次 runner 为 manual/validator 退出码 `0/0`、`validation_scope=functional`，render effects/decal 均 true；637 帧无图错误、fault、temporal anomaly 或 watchdog 触发，物理每 render 帧一阶。`report.pass=false` 与 `full_pass=false` 仍保留原性能 p95 `4.0843 ms > 4.0 ms`，公共 validator 只给出功能 `pass=true` 和 deferred performance issue；未修改门槛或生产动画图。下一步只沿 A01 时间链建立并运行可见运动场景。

- [x] **BF124.A01-S3b1 — 原 Playground 地面、空中与基座运动功能 Gate**：开始：2026-09-23 05:57:22 +08:00；预估结束：2026-09-23 06:47:22 +08:00；实际结束：2026-09-23 06:01:32 +08:00；偏差：-45m50s；结果：三场公共功能 Gate 通过。估时依据：已通过的 startup、既有公共场景时长与 S2b1–b3 静态/夹具闭包。拆分结果：受控低帧率与单帧 hitch 矩阵仍待独立 Gate，性能继续按用户要求暂停。

  runtime 前摘要：S1–S2b3 的 UE CMC/Chaos、Grounded/Air、Mantle/Ragdoll 与移动基座源/目标时钟及 focused Gate 已闭合；当前生产改动未触及图、资产和转换规则。公共 startup 功能已由同二进制通过，完整性能预算失败已保留且不调整阈值。先运行原 Playground 的现成移动基座场景，按其实际报告决定下一场，不用动作场景探索静态根因。

  `moving_base` 后摘要：同二进制原 Playground 的 public runner 为 manual/validator `0/0`，功能 `pass=true`；10 项实际覆盖全部完成（ride、jump、新基座、Mantle 起止与恢复），1073 条基座 trace、2 个有效动作窗口和 411 个姿态样本。图错误、fault、temporal anomaly 与 watchdog 均为 0，渲染/贴花检查通过。报告保留完整性能失败（eval p95 `7.35 ms`）且只按既定暂停范围给出功能结论；下一场为 `uphill_jump` 的空中接触与动画恢复。

  `uphill_jump` 后摘要：同二进制 public runner manual/validator `0/0`、功能 `pass=true`；三条上跳路线与全部落地动画一致性覆盖，18 个窗口、2002 条上跳 trace、1344 姿态样本，零 graph error/fault/temporal/watchdog，渲染/贴花检查通过。完整性能失败继续延期（eval p95 `8.8236 ms`），未放宽阈值；下一场为 `grounded` 的地面动作与 root-motion。

  `grounded` 后摘要：同二进制 public runner manual/validator `0/0`、功能 `pass=true`；15 项实际覆盖包含 Roll/Slot、起止回调、站蹲/瞄准/快速停止及 Roll 退出过渡，9 个窗口、1931 姿态样本。三场均无 graph error、fault、temporal anomaly 或 watchdog，渲染/贴花通过，截图为 0；完整性能失败按暂停范围保留（本场 eval p95 `8.4876 ms`）。现成场景证明普通帧功能与可见角色路径，尚未证明 A01 在受控 20/15/10 FPS 或 hitch 下的真实入口行为。

- [x] **BF124.A01-S3b2 — 真实入口受控帧时间与可核验配置**：开始：2026-09-23 06:04:00 +08:00；预估结束：2026-09-23 06:44:00 +08:00；实际结束：2026-09-23 06:09:23 +08:00；偏差：-34m37s；结果：受控时间入口和可核验证据 Gate 通过。估时依据：一个 Bevy 时间入口、既有 runner 参数/报告结构及同二进制场景基线。拆分结果：多帧率矩阵和单帧 hitch 仍待独立运行。

  runtime 前摘要：本地 Bevy 0.19.1 `TimeUpdateStrategy::ManualDuration` 推进 Real→Virtual，`Time<Virtual>` 默认最多接收 250 ms；只在 `ALS_MANUAL_INTEGRATION_GATE` 已设置时接受参数。example 聚焦自测合法 100 ms、零/负/NaN/inf/非数及 300 ms 均符合预期；runner 300 ms 在创建报告目录之前正确失败且没有启动游戏。旧未设参数的三场原 Playground 功能基线已通过；现在限流重建启动器同目录，随后用固定 60 FPS 的 `moving_base` 公共场景核对机器可读参数和实际时间间隔，未改图/资产/性能阈值。

  固定帧后摘要：同 `target_game/play` 预构建通过；公共 `moving_base` 在 `-FrameDeltaSeconds 0.016666666666666666 -CaptureCount 0 -FunctionalOnly` 下 manual/validator `0/0`、功能通过，668 条 trace 的 667 个相邻实际游戏时间差全部为 `0.016666667±1e-9 s`。runner 的 `frame_delta_seconds` 保存了请求值；未设置参数的先前同场 1073 条 trace 间隔为 `0.0100186–0.0244875 s`，没有被暗中固定。图/fault/temporal/watchdog 为零，渲染/贴花通过；性能仍按原暂停范围延期。配置异常在运行前失败且不创建产物，未改普通手动入口。

- [x] **BF124.A01-S3b3 — 真实 Playground 固定帧率运动矩阵**：开始：2026-09-23 06:11:07 +08:00；预估结束：2026-09-23 07:01:07 +08:00；实际结束：2026-09-23 08:16:01 +08:00；偏差：+74m54s；结果：全部固定帧率功能 Gate 通过，完整性能预算按用户暂停范围延期。沿同一 `target_game/play` 与原 `moving_base` 场景，逐次运行 120/30/20/15/10 FPS，核对 runner 设定、相邻实际游戏时间、基座/角色位移和现有动画/接触/图/时序 Gate；再用最低帧率的 `grounded`/`uphill_jump` 核查 Roll/root-motion 与落地。估时依据：已通过的普通/固定60场、现有公共 runner 和无需重编的同二进制。拆分结果：基座/Grounded、上跳节点归因、源 IK 校准分别按 S3b3a/b1/b2 验收；单帧 hitch 仍需独立 Gate。

  2026-09-23 07:13:38 +08:00 拆分检查点：原预计 07:01:07，当前偏差 +12m31s；固定帧率移动基座与 10 FPS Grounded 分支的功能 Gate 已通过，10 FPS 上跳的公共时序 Gate 仍未通过。报告显示其原动画图/惯性化链与本次物理整帧分支消融后均可产生腿部突变，不能把物理子项或整项直接勾选。按剩余源闭包重新拆成下述可独立验收的静态首失配定位与后续修复/runtime；现暂停新的 runtime，先读 fresh source 与目标中间节点。

  runtime 前摘要：本地 UE CMC/Chaos 完整帧/封顶时钟、目标空地/基座/动作隔离已在 S1–S2b3 静态与 focused 验证；startup、原 moving_base/uphill_jump/grounded 与固定 60 FPS moving_base 公共功能 Gate 通过。Bevy `ManualDuration` 与 250 ms 虚拟上限已用本地源码、聚焦自测和 667 个实际时间差确认；当前未通过边界是受控低帧率下的真实运动/动画采样。保留原图、Playground、性能阈值和 `CaptureCount 0`，先跑 120 FPS 再逐场检查。

  首次 120 FPS 后摘要：public runner 功能 `pass=true`，10 项覆盖、1420/1420 个相邻实际游戏时间差为 `1/120 s`，1371 条轨迹已有平台非零速度，零图错误/fault/temporal/watchdog。但早期辅助 `PLATFORM` 日志在场景就绪前把曲线起点的 `moving=0/2` 一次性判 FAIL，未进入公共报告，不能当作平台持续静止。focused 夹具先复现过早锁存失败，现复用 `ManualPlaygroundStartupReady` 延后判定并通过；保留原始 120 报告，限流重建后用新目录复验，随后才进入 30 FPS。

  120 FPS 修复后摘要：同 `target_game/play` 重建及新输出的 public runner manual/validator `0/0`、功能通过；10 项覆盖、1420/1420 个相邻实际游戏时间差为 `1/120 s`，无平台早期错误日志、graph error/fault/temporal/watchdog。辅助探针只在现成启动就绪后判定，未新增等待状态；原第一次误报报告保留供复查。下一场按相同源/目标闭包运行 30 FPS。

  30 FPS 后摘要：public runner manual/validator `0/0`、功能通过，10 项覆盖、345/345 个相邻实际游戏时间差为 `1/30 s`，零 graph error/fault/temporal/watchdog；当前未通过边界仍是低于 30 FPS 时物理封顶与角色整帧的分叉，下一场 20 FPS。

  20 FPS 后摘要：`moving_base` public runner manual/validator `0/0`、10 项功能通过；239/239 个相邻时间差为 `0.05 s`，无 graph error/fault/temporal/watchdog。已真实经过物理 `1/30` 封顶与角色整帧分离路径；下一场 15 FPS。

  首次 15 FPS 后摘要：原场景先完成 Ride/Jump，随后 `Flight` 超过固定 8 s 无阶段进展，manual 退出 91；第一个分叉在离台帧：15 FPS 的平台 `x≈18.28 m`、朝远离路径终点 `+X` 的 `9.81 m/s` 被角色继承，20 FPS 成功轨迹在 `x≈12.59 m`、`-X` 返回方向跳离；前者最终飞出原场景地面范围。新旧源场景记录该平台的 spline `start=(19,-28.75)`、`end=(12.5,-28.75)`；夹具旧 Gate 只要求走满周期，启动前固定帧数使不同 FPS 的离台相位漂移。新增方向最小反例先稳定 FAIL，按源 spline 向量仅在平台朝终点返回时允许完成 Ride，10 项同模块测试通过；没有修改平台曲线、角色运动、阈值或原场景。保留首份失败报告，限流重建后独立目录复验 15 FPS，再决定是否进入 10 FPS。

  15 FPS 修复后摘要：同二进制 `moving_base` public runner manual/validator `0/0`、10 项覆盖与功能通过，210/210 个相邻实际游戏时间差为 `1/15 s`；Ride 完成后等待源路径返回，离台速度改为 `-0.173 m/s` 并成功落到新支撑，再完成 Mantle 与动作返回。零 graph error/fault/temporal/watchdog；原失败报告保留。下一场 10 FPS，仍不更改物理/动画判据。

  首次 10 FPS 后摘要：原场景 Ride/Jump 与“任意非原基座”落地已执行，但 Flight 的反向平台速度使角色越过源落地区，落在远处 `x≈4.29 m` 地面；随后 `WaitMantle` 8.1 s 固定 watchdog 退出，Mantle 相关五项未覆盖。首个源链分叉仍是离台相位而非图/姿态错误：源 Playground 的 `MoveSpline` 终点 `x=12.5 m`、平台原网格沿路径半宽 `1 m`，旧仅要求速度指向终点，在 10 FPS 可能于起点 `x≈19 m` 附近提前离台。新增端点几何范围反例先 FAIL，只在源方向正确且进入平台自身半宽内允许结束 Ride；同模块 10 项 focused 与 watchdog 自测通过。保留失败报告，重建同入口后独立目录复验；未修改运动生产算法、场景或宽松验收阈值。

  10 FPS 修复后摘要：同 `target_game/play` 重建后 public runner manual/validator `0/0`、10 项覆盖与功能通过；164/164 个相邻实际游戏时间差为 `0.1 s`，离台位于源 spline 终点自身范围，后续新支撑、Mantle 和返回均成立。零 graph error/fault/temporal/watchdog，渲染/贴花通过；两份旧失败报告保留。下一步以同二进制运行 `grounded` 和 `uphill_jump` 的 10 FPS 姿态/root-motion/落地路径，性能仍只按暂停范围延期。

  首次 10 FPS `grounded` 后摘要：同入口场景仅 `grounded_roll_exit_transition`/`grounded_roll_completed` 两项覆盖未通过；图错误、fault、temporal 与 watchdog 均为零。报告中 Roll 退出函数确已调用一次，选中 Crouching 片段并在 Slot 有效播放，随后实际站起状态结束该片段。fresh 原生 `AB_Als_Grounded` 明示 Roll `StateExitFunction=PlayRollToGroundedTransitionAnimation` 与 Crouching→Standing `StateEntryFunction=StopTransitionAndTurnInPlaceAnimations`；fresh `CS_Als_Default` 为 `Rolling.bCrouchOnStart=True`，本地 `PlayTransitionRightAnimation` C++ 按当帧实际 Stance 选片段。旧公共判据不当地要求 Roll 已结束与过渡仍活跃在同一采样帧重叠；10 FPS 的源合法路径可能先贡献 Crouching Slot，再在站起入口停止。现保留片段身份/参数/Slot/实际图状态断言，只放开这个多余同帧条件；focused 反例先 FAIL 后 PASS。原失败报告保留，重建并独立目录复验，未改生产图或通知时钟。

  第二次 10 FPS `grounded` 后摘要：manual/进程退出 0 且动作覆盖已完成，但公共视觉 validator 继续拒绝 `grounded_roll_exit` 的旧 Standing 专属标准片段时间与终态：Crouching 片段在 Roll 活跃时短暂贡献，随后源 `Crouching To Standing` 入口停止它，最终实际父状态为 Standing。旧窗口要求 Crouching 分支在片段 `0.8 s` 仍活跃、终态 Crouching，因而缺标准骨姿/两个活跃样本并判错终态。fresh 源图、设置与 C++ 以及本次实际通知/状态链证明应允许 Crouching→Standing 父状态，标准采样点由该片段源 `StartTime + PlayRate × BlendOut` 得 `0.5 s`，终态按不变的 DesiredStance 取 Standing；不移除骨骼、片段身份、权重、时间和时序守卫。对应状态4反例先 FAIL 后 PASS；保留第二份失败报告，同目录重建后另起输出复验。

  10 FPS `grounded` 源分支复验摘要：public runner manual/validator `0/0`、15 项功能覆盖通过；`grounded_roll_exit` 实际 Crouching 片段在 `0.5 s` 精确取得 canonical pose（6 个下肢骨）、2 个活跃 clip 样本，终态父状态为源目标 Standing `1/1`。零 graph error/fault/temporal/watchdog，渲染/贴花通过；原两份失败报告保留，完整性能预算继续延期。下一场同二进制运行 `uphill_jump` 的 10 FPS 接触/姿态恢复。

  首次 10 FPS `uphill_jump` 后摘要：manual/进程退出 0、18 个跳跃/落地窗口与实际 Grounded 动画恢复均完成，但公共时序守卫拒绝 4 项局部旋转/弯曲面突变（首项 pre-rig `calf_l` 在 `uphill_01_sprint_1` 跳跃段单帧 `2.339 rad > 2.2`），因此不能关闭 A01。fresh 原生 `AB_Als_Locomotion` 核对 Right Foot→Loop 过渡为 `LogicType=TLT_Inertialization`、时长 `0.2 s`，目标确用惯性请求和下游 Inertialization；没有证据把它误说成应使用普通 Hermite 交叉淡入。报告显示该节点附近 Jump 片段末时 `1.667 s` 切至 Loop，首个异常在根图仍有 240 活跃通道时出现；物理/惯性根因尚未定。下一步只做一次受控消融：同原 Playground、同 10 FPS、同报告 Gate，暂时禁止本次角色整帧接管以判别该物理分支是否为异常的必要原因；随后原子恢复代码与二进制，不改阈值或图。

  物理消融后摘要：同原 Playground/10 FPS 仅暂时禁止 A01 的角色整帧接管，公共报告仍检出腿部旋转尖峰与弯曲面翻转（触发侧/时刻改变）；因此本次物理分支不是异常的必要原因。临时代码已完全恢复，`source_physics_timing.rs` 对已提交版本无 diff，`target_game/play` 重新编译为正常版本；保留两份前后报告且不把消融运行计为通过。fresh 本地 `AB_Als_Locomotion` 原生读取器正常结束：Right Foot→Loop 源 `TLT_Inertialization/0.2 s` 与目标 Transition 请求对应，Jump 的下游 Inertialization 也存在；公共 source-chain 仅记录整体 Pose 时间/骨数，尚不能定位最早改变 `calf_l` 的子图。下一项先补 focused 的中间骨姿/请求取证，不再盲启 runtime。

  矩阵完成与估时复盘：`moving_base` 120/60/30/20/15/10 FPS、`grounded` 与 `uphill_jump` 10 FPS 均在原 Playground 的 public functional Gate 通过，最终报告均无 graph error/fault/temporal/watchdog。原 50 分钟估时遗漏了 120 FPS 启动判据、低帧率平台相位、Roll 源合法分支及上跳源 IK 正例取证，父项超过允许误差后没有及时重估；后续同类矩阵先分别估算 source 夹具边界与姿态闭环，再按每段 ≤60 分钟拆项并在超时检查点记录新起止时间。A01 角色整帧功能仍须完成单帧 hitch 验收。

  - [x] **BF124.A01-S3b3a — 固定帧率基座与 Grounded 分支功能验收**：开始：2026-09-23 06:11:07 +08:00；预估结束：2026-09-23 07:01:07 +08:00；实际结束：2026-09-23 07:00:00 +08:00；偏差：-1m07s；结果：本子项功能 Gate 通过。原 Playground 的 120/60/30/20/15/10 FPS moving_base 及 10 FPS grounded 均由同入口 public runner 功能通过、实际时钟逐帧复算与 0 截图证明；原 120 首帧平台诊断、15/10 跳离相位和 Roll 实际 Crouching→Standing 窗口的失败已由源几何/函数图证明并留 focused 反例。各最终报告图错误、fault、temporal、watchdog 为 0，性能预算仍按用户暂停范围延期；上跳异常不纳入本子项成功范围。

  - [x] **BF124.A01-S3b3b1 — 10 FPS 上跳首个姿态源链失配定位**：开始：2026-09-23 07:13:38 +08:00；预估结束：2026-09-23 07:58:38 +08:00；实际结束：2026-09-23 07:38:46 +08:00；偏差：-19m52s；结果：首个异常源链与数值 Gate 通过，未证明生产算法差异。估时依据：单个 Right Foot→Loop `TLT_Inertialization`、目标 Transition/Jump/root 三段链和现成 10 FPS 报告。拆分结果：源定义的落地硬切与后续 final 腿部异常分开在下一项验收。

    取证进展：fresh UE 原生 `AB_Als_Locomotion` Right Foot→Loop 为 `TLT_Inertialization/0.2 s`，目标 nested FSM 的请求经 Jump 图 `InertializationPassthrough` 后再过 Air Lean/Additive/Output；本地 UE `AnimNode_Inertialization` 与目标均在求值前推进当前 Delta。已导出 target glTF 的相同局部骨骼在 Right Foot 末帧与 Loop 首帧仅相差 `calf_l≈0.130 rad`、`thigh_r≈0.422 rad`、`foot_r≈0.297 rad`，不足以单独解释首个 pre-rig `calf_l=2.339 rad`；Air Lean 与 Fall 同时刻采样也未显示同量级角差，但其实际输入相位仍待核对。现有公共链只记录 Locomotion Output 的整体 Pose 时间/骨数；复用既有图节点读取器，临时在单一 `uphill_01_sprint_1` 飞行窗口写出 nested FSM→Inertialization→Additive→Two Way Blend→Jump Output→Locomotion Output 的 `calf_l` 局部旋转，uphill focused 测试 2/2 通过。下一步只运行同场诊断以锁定第一个节点，不把诊断代码当作修复。

    单次诊断后摘要：同二进制原 Playground/10 FPS 的公共功能覆盖完整，仍因原四项 temporal/fault 拒绝；诊断 trace 已锁定 `uphill_01_sprint_1` 首项所在 frame。Jump FSM、其惯性节点、Air Lean/Additive 和 Jump Output 的 `calf_l` 相邻帧最大仅约 `1.004 rad`，Locomotion 根输出于游戏时间 `26.0→26.1 s` 才突变 `2.333987 rad`；故 Right Foot→Loop 不是首个异常节点。fresh UE 外层 Jump→Land Movement 源过渡为 `TLT_Inertialization/0.1 s`，目标 T26 同值；其时长等于 10 FPS 当前帧，但是否在外层 FSM、Land 目标姿态或根惯性 consumer 首先分叉尚未证实。下一步只在同一 trace 加 Land Movement、外层 FSM 与根惯性节点取证，保留原报告，不改生产算法或公共阈值。

    Land 中间节点 runtime 前摘要：已对照同一源 `Land Movement`、外层 FSM 与根 `Inertialization` 的 source-visible 节点名和目标 RON 边，诊断仅扩展这一飞行窗口的本地骨旋转记录；uphill focused 2/2、runtime graph asset boundary 和同 `target_game/play` 限流构建通过。无源图/转换规则/原资产变更。仅一次同场 10 FPS 诊断，收集原异常帧的 source/target/consumer Pose，失败仍按公共 validator 保留。

    Land 中间节点后摘要：首个 pre-rig `calf_l` 尖峰的前一游戏帧 `26.0 s`，Jump Output 局部旋转为 `[-0.036925,-0.346291,0.015896,0.937266]`；`26.1 s` 实际落地，Land Movement Output、外层 FSM、根 Inertialization 与 Locomotion Output 均同为 `[-0.002732,-0.997550,0.049145,0.049713]`，相邻角差 `2.333987 rad`。Jump FSM/Jump 内惯性/Additive 最大相邻差仅 `1.004 rad`；首个大跳发生在外层 Jump→Land Movement，不在先前猜测的 Jump→Loop。fresh UE 源过渡 `TLT_Inertialization/0.1 s` 与目标相同，本地 UE `FAnimNode_Inertialization` 于 Evaluate 前累加当帧 `0.1 s` 并在达到时长时结束；当前目标根 consumer 在该帧也输出完整 Land 目标 Pose。此证据支持源允许该粗帧边界的完整目标姿态，但不能代替 UE 实机姿态证明，也未解释稍后的 final `thigh_r`/`foot_r` 与弯曲面异常。临时中间节点记录已按消融删除，`target_game/play` 已从无诊断代码重新限流构建；生产图/算法、原资产和公共阈值未改，报告保存在忽略的 target 证据目录。

  - [x] **BF124.A01-S3b3b2 — 落地边界与 final 腿部时序归因**：开始：2026-09-23 07:38:46 +08:00；预估结束：2026-09-23 08:23:46 +08:00；实际结束：2026-09-23 08:15:10 +08:00；偏差：-8m36s；结果：源数学归因、校准反例与原 Playground/10 FPS 功能 Gate 通过。以 fresh UE 外层 Land 惯性时钟、Land Movement 源姿态与目标中间输出，建立明确的 source-expected 单帧边界反例，保留公共时序守卫在其它帧的原阈值；对晚于落地的 final `thigh_r`/`foot_r` 和弯曲面异常沿 Control Rig/物理写回找首个源目标分叉。估时依据：b1 已锁定 `26.0→26.1 s` 单一外层过渡和后续两种 final 症状，现有 10 FPS 报告/公共时序自测可复用。拆分结果：固定帧率矩阵可汇总，A01 单帧 hitch 另项验收。

    runtime 前摘要：fresh UE T26 为 `TLT_Inertialization/0.1 s`，本地 UE `FAnimNode_Inertialization` 在 Evaluate 前把当帧 Delta 加入 elapsed，达到 duration 即停止；目标 `Land Movement`、外层 FSM、根惯性 consumer 在该 100 ms 帧均输出同一源目标骨姿。新增仅限 `uphill_jump` 实际 Flight→Land Movement（outer=4）、Grounded 且 Delta≥100 ms 的一次性 temporal rebase，普通 50 ms、错误状态与重复触发仍由 focused 自测拒绝；uphill 3/3 与公共 epoch 边界自测通过。无图、资产、物理算法或全局阈值修改。现在同目标目录构建并运行一次原 Playground/10 FPS，重点审计原首项以及后续 final 腿部异常。

    runtime 后摘要：同入口 10 FPS `uphill_jump` 的公共手动 6/6、18 个 motion windows、graph error 0，原首个 `pre-rig calf_l` 告警消失；公共时序仍拒绝 4 个稍后的 final 告警（`calf_l 2.2748`、`thigh_r 2.35898`、`foot_r 2.44743 rad`、右腿弯曲面 `-0.71018`）。逐帧足部报告在 17.6–18.8 s 的 IK 权重为 1，effector/knee 求解误差接近 0；本地 UE `AlsAnimationInstance.cpp` 相邻位置差超过配置 `TeleportDistanceThreshold` 即 `MarkTeleported`，故目标冲刺 10 FPS 多次超过 0.5 m 的检测本身不是已证实差异。下一步仅沿该窗口的源 Control Rig TwoBoneIK pole 语义和目标求解输入定位首个差异；不以全局阈值放宽或重复运行绕过失败。

    fresh Rig 与数值闭环：仓库原生 reader 重新提取当前 `/ALS/ALS/Character/CR_Als`，`ApplyFootIk` 的实际 Basic IK 为 `PoleVectorKind=Location`，Pole 由 Calculate Pole Vector→Scale/Add→Damper Exact 提供；`RefreshFootIk` 左右轴与目标一致。当前 UE `TwoBoneIKSimplePerItem` 和 `AnimationCore::SolveTwoBoneIK` 以全局 Pole 位置求弯曲面；将 10 FPS 报告的当帧 hip/target/pole 及源资产初始腿长代入源位置公式，6 个异常相邻帧的膝位置误差均 ≤0.0000127 m，18.7→18.8 s 的源公式腿平面点积 `-0.7101818`，与公共报告 `-0.71018` 一致。pre-rig thigh 与 Rig 求解输入同帧角差 ≤0.0007 rad，排除这一段的帧滞后；3 个局部旋转尖峰仅发生在 final，且 `foot_r` 的世界转角仅 `0.494 rad`，其 `2.447 rad` 局部差来自同帧 calf 约 `2.368 rad` 的 IK 弯曲。未发现源/目标 Rig 算法差异；现补源合法 100 ms 大幅 IK 输出与真正 180° 翻转的正反例，再仅校准公共时序判据。A48 所述 Direction 分支不在此足部调用链，留待其编号独立核验。

    校准后 runtime 前摘要：源公式正例 100 ms 下局部 `2.4474256 rad`、腿面约 `-0.71018` 先在原规则稳定 FAIL；同帧真正 180° 旋转/翻面为负例。只为大角度的绝对跳变和腿面翻转增加 `30 rad/s` 速率条件，界于源正例最大 `24.4743 rad/s` 与 100 ms 全翻转 `31.4159 rad/s` 之间；短帧原 `2.2 rad`、`-0.5 dot` 与 `80 rad/s` 判据仍在。公共 temporal 11/11 与 validator self-test 通过。消融删除先前为 Land 边界加的 `uphill_jump` 专用重置状态/分支及其测试，文件恢复提交版，正例仍 PASS；现在只重建同 `target_game/play` 并跑一次原 Playground/10 FPS 功能 Gate。

    完成摘要：同 `target_game/play` 的 `uphill_jump`/10 FPS public runner manual/validator `0/0`，18/18 姿态窗口、6/6 功能结果通过，557 pose samples；graph error、fault、temporal anomaly、watchdog 均为 0，渲染/贴花通过。完整性能预算继续按用户暂停范围延期。分别消融局部大角速率与腿面速率条件后，源合法 100 ms 正例各自稳定 FAIL；恢复后 temporal 11/11 与 validator self-test 再次通过。最终无 `uphill_jump` 专用新状态/分支、无生产运动或 Rig 算法更改，仅保留公共速率判据、源正反例和声明式阈值。

- [x] **BF124.A01-S3b4 — 原 Playground 单帧 200 ms 卡顿 Gate**：开始：2026-09-23 08:17:18 +08:00；预估结束：2026-09-23 09:02:18 +08:00；实际结束：2026-09-23 08:30:36 +08:00；偏差：-31m42s；结果：单帧卡顿、源时钟位移和原 Playground 功能 Gate 通过。fresh 本地 UE `ControlledCharacterMove→PhysWalking→GetSimulationTimeStep` 仍消费完整 200 ms、普通刚体保持 `1/30 s` 封顶；S2 已有 200 ms 隔离运动反例。本项仅在 `run_manual.bat` 的原 `manual_playground` 公共功能入口，以既有手动时钟为基线，在实际 Flight/Air 帧注入一次 200 ms、随后恢复基线；核对实际时间跳变、位移、物理/接触/图/姿态及平台落地/Mantle 返回。估时依据：现成同入口 runner、公开动画窗口与 S3b3 固定帧率报告。拆分结果：A01 可做综合收尾；性能按用户暂停范围延期。

  runtime 前摘要：重新核对本地 UE `ControlledCharacterMove` 的整帧 Delta 与 `PhysWalking/GetSimulationTimeStep` 分步消费，目标 S2 的角色剩余时间扫掠在固定 200 ms focused 夹具已有反例；本次只扩展手动场景入口。新增 `FrameHitchSeconds` 仅允许 `moving_base`、已给基线且 `baseline < hitch ≤ 0.25 s`；例子在公共 `moving_base_jump` 已稳定窗口的 Last 阶段设置下一帧 200 ms，并在该帧 Last 恢复基线。focused 1/1 通过，去掉窗口触发后稳定 FAIL、恢复后再 PASS；validator 自测的单次/缺失/重复/未移动反例通过，runner 对小于基线的非法参数启动前拒绝。无生产运动、图、资源变更。下一步同 `target_game/play` 限流构建，原 Playground/60 FPS 基线只运行一次公共 hitch 功能 Gate。

  首次 runtime 后摘要：原 Playground 10/10 手动功能、2 个姿态窗口均通过，graph error/fault/temporal/watchdog 为 0；实际时间记录恰有一个 `0.200000 s` 且角色移动约 1 m，但 public hitch 附加 Gate 正确拒绝：该帧是 `Exit/Grounded` 而非 Air。静态回读发现 `moving_base_jump` 窗口由原夹具同时覆盖 Exit 和 Flight，旧触发只看窗口标签，过早设置下一帧；无生产物理/动画失败证据。保留原失败报告，只将一次性触发收紧为原 `moving_base_trace` 当帧 `Flight/Air`，补该边界 focused 反例后再按同入口复验。

  复验 runtime 前摘要：一次性触发改为读取同场 public trace 最后一个已观察帧的 `Flight/Air`，避免误用同时覆盖 Exit 的窗口标签；最小 focused 测试同时证明 `Exit/Grounded`、`Flight/Grounded` 不触发、`Flight/Air` 仅触发一次并恢复基线，1/1 通过。保持原 `0.2 s` 和 `1/60 s` 输入、同一 Playground 与全部既有功能判据，现在限流重建后另目录复验。

  完成摘要：同 `target_game/play` 的原 `moving_base` 场景 public runner manual/validator `0/0`，877 个基座 trace 相邻时间仅一次 `0.2 s`，其它为 `1/60 s`；该帧 `Flight/Air`、位移 `1.1294 m`，水平位移与当帧速度乘 200 ms 的误差约 `0.0087 m`，会拒绝仅按 `1/30 s` 积分的反例。10/10 功能结果含 Ride/Jump/新支撑/Mantle/返回，2 个姿态窗口通过；graph error/fault/temporal/watchdog 均为 0，渲染/贴花通过。validator 自测单次、缺失、重复、未移动、错误模式和封顶位移边界通过，原错误 Exit 帧报告保留。最终只保留场景专用一次性时钟注入与校验，无生产运动/图更改；完整性能预算依用户暂停范围延期。

- [x] **BF124.A01 — 角色运动完整帧时间与封顶物理时间**：开始：2026-09-23 08:32:03 +08:00；预估结束：2026-09-23 08:47:03 +08:00；实际结束：2026-09-23 08:32:33 +08:00；偏差：-14m30s；结果：源合同、实现、消融与原 Playground 功能 Gate 全部闭合。估时依据：S1→S3b4 的 source/target、最小失败反例、模式/基座消融和实际入口报告均已独立提交；拆分结果：本编号无剩余功能 Gate，完整性能预算依用户此前暂停范围延期，后续反馈逐编号独立激活。

  汇总：UE 角色 PhysWalking 消费完整帧，Chaos 普通刚体只消费封顶时间；目标已在公开 Avian 扩展点实现 Grounded/Air 角色剩余时间的逐子步扫掠，维持 Ragdoll、Mantle 与移动基座的源所有权。100 ms/200 ms 最小位移反例、墙/地接触、平台相对运动及必要消融通过；原 Playground startup、grounded/uphill、moving_base 六种固定帧率与单次 200 ms Air 卡顿的公共功能报告通过，最终图错误、fault、temporal、watchdog 均为 0。已核对 55 条反馈索引唯一且齐全（A01–A55）、S1→S3b4 的分项提交和无未提交生产代码；A02 是下一执行项。

- [x] **BF124.A02 — 旋转响应视角角速度单位**：开始：2026-09-23 08:34:13 +08:00；预估结束：2026-09-23 09:14:13 +08:00；实际结束：2026-09-23 08:41:35 +08:00；偏差：-32m38s；结果：源单位、失败反例、最小修复、消融与实际入口 Gate 通过。fresh 本地 UE `RefreshView` 以度制 Yaw 差/秒计算 `ViewState.YawSpeed`，`CalculateGroundedMovingRotationInterpolationHalfLife` 以 `300 deg/s` 为参考；目标此前将弧度差/秒直接除以同一 300。估时依据：一个 producer、一个插值 consumer、现成旋转响应测试与公共场景；拆分结果：本项无剩余功能 Gate，A03/A05 等相邻分支按编号另行核验。

  source/static 与 red 反例：fresh 本地 `AlsCharacter.cpp` 的 `RefreshView` 将 UE 度制 `Rotation.Yaw−PreviousYawAngle` 的绝对值除以 Delta，`CalculateGroundedMovingRotationInterpolationHalfLife` 以 `ReferenceViewYawSpeed=300.0f` 归一；目标 `motion.rs` 的 producer 给出弧度/秒，consumer 却仍用 300。真实 `rotation_response_system` focused 反例采用前一 View 0°→当前 90°、Delta 100 ms、Grounded/Running/ViewDirection，源速率 900 deg/s 应使 half-life 饱和到 0.05 s；旧目标 actor yaw `0.33195138 rad`，源合同期望 `0.6516161 rad`，稳定 FAIL。其它 GaitAmount 连续曲线和独立旋转模式不在当前链路改动范围。

  runtime 前摘要：只在上述 producer 的绝对角差后转换为 `deg/s`，同一真实系统反例与现有半衰期/目标速度 focused 2/2 PASS；去掉唯一转换后回到 `0.33195138` 并稳定 FAIL，恢复后 2/2 再 PASS。未改旋转模式选择、半衰期配置、资产或图；下一步同 `target_game/play` 限流构建，原 Playground `standing_response` 的 public functional/visual Gate 复验，性能预算沿用户暂停范围延期。

  完成摘要：`target_game/play` 限流重建后，原 `standing_response` 场景 public runner manual/validator `0/0`，6/6 功能结果覆盖 reverse/cycle/idle，9 个响应姿态窗口、129 个样本通过；graph error/fault/temporal/watchdog 为 0，渲染/贴花通过。新反例的 90°/100 ms 源半衰期 `0.05 s`、目标 actor yaw `0.6516161 rad` 通过，删除 `to_degrees()` 即重现 `0.33195138 rad` 失败。只保留一处单位转换与一个真实系统回归，完整性能预算依用户暂停范围延期。

- [x] **BF124.A03 — 第一人称非瞄准身体旋转与限角**：开始：2026-09-23 08:43:36 +08:00；预估结束：2026-09-23 09:33:36 +08:00；实际结束：2026-09-23 09:06:56 +08:00；偏差：-26m40s；结果：源分支、最小反例、消融、第一人称实际入口功能 Gate 及关联时序回归通过。fresh 本地 UE `RefreshGroundedRotation` 静止/移动与 `RefreshInAirRotation` 均以 `Aiming || FirstPerson` 进入身体瞄准/限角；目标现复用已有 Aiming 路径。估时依据：一个已有第一人称组件、现有 Aiming 状态/限制器、公共场景。拆分结果：A03 用原 `camera` 公共功能/视觉 Gate 验收，原 `state_machine` 的两项与 A03 无关的覆盖缺口继续如实保留；常规 Air 策略 A04 独立执行。

  source/static 与 red 反例：fresh C++ 的 Grounded 静止、Grounded 移动、InAir 三个入口均明确 `Aiming || FirstPerson`，后者不是只控制相机/脊柱；`RefreshGroundedAimingRotation` 静止直接设 View 目标并应用次级/主限角，移动以 `1000 deg/s` 与 `0.1 s` 平滑后主限角，`RefreshInAirAimingRotation` 直接设 View 目标并以 `0.1 s` 平滑后主限角。目标已有 `AlsFirstPersonView` 组件和完整 Aiming 限制器，却只按 RotationMode 进入；系统测试三条边界中第一条 Grounded 静止 100° View/100 ms 先稳定 FAIL（actor yaw `0`，源预期约 `15.00338°` 且需限制器生命周期）。只沿现有 Aiming 分支合并条件，普通非第一人称 Air 策略仍留 A04。

  runtime 前摘要：目标只增加 `AlsFirstPersonView` 查询，将第一人称非瞄准映射到已有身体 Aiming 目标选择、`1000 deg/s`/`0.1 s`、静止次级限角和 Air `0.1 s` 路径；常规非第一人称模式仍走旧策略。最小真实系统测试覆盖 Grounded 静止 ViewDirection、Grounded 移动 ViewDirection/VelocityDirection、已有 70° 主限角及 Air VelocityDirection；去掉第一人称条件即重现静止 yaw=0 的 FAIL，恢复后旋转相关 23/23 通过。无相邻 A04 Air 常规算法、资产或动画图改动；现在限流重建 `target_game/play` 并运行原 Playground `state_machine` 公共功能/视觉 Gate。

  首次 runtime 后摘要：同入口 `state_machine` 跑至 `StateMachineRotate` 后退出 91；首个时序告警在本夹具 `phase_elapsed=3.9057674 s`，恰跨第一人称 View yaw `-90°→+90°` 的固定 3.9 s 分支。该帧 pre-rig 手部速率约 41.49/40.96 m/s、final `foot_l 86.15 rad/s` 与左腿平面 `-0.6286` 被公共守卫拒绝；无图错误，watchdog 未触发。后续 `p1_turn_and_rotate_both_sides` 与 `generated_stop_quick_transition_notify` 未覆盖，可能是先前时序/分支行为的连带结果，尚不能归因。原报告保留；下一步先核对源 Rotate In Place 的第一人称输入/阈值及夹具明确的 1.3 s 左右分段，再对首次突变与覆盖链分别定位，不调宽阈值或盲重跑。

  一次运行时分支消融前摘要：fresh 源 `RefreshRotateInPlace` 在 FirstPerson 非 Aiming 时使用 `65°` 阈值，目标已有 `rotate_in_place.allowed`/`first_person_threshold` 的源图输入；夹具 `StateMachineRotate` 每帧将 Transform yaw 强设初始值，却未同步物理 Rotation，且每 1.3 s 瞬时切换世界 View yaw，A03 正确身体跟随使这些旧夹具前提可能失效。尚未证明失败是否由 A03 新分支触发；只暂时移除本次 `FirstPerson→Aiming` 条件、同原 Playground/同 public Gate 对照一次，然后立即恢复源码与 `target_game/play`，不修改图、阈值或已完成项。

  消融后与下一 Gate 前摘要：同原 `state_machine`、同 public runner 仅暂时关闭 A03 条件后，temporal/fault 均为 0，却仍缺 `p1_turn_and_rotate_both_sides` 与 `generated_stop_quick_transition_notify` 两项，manual 仍退出 91；故两项覆盖是此任务前已存在的边界，不据此改完成的图/通知模块。A03 源正确身体跟随使夹具 `3.9 s` 固定 180° View yaw 跳变成为新时序反例；源码已恢复 A03 条件，准备同 `target_game/play` 重建。先用原 `camera` 场景验证实际第一人称可见功能，再针对已证实的夹具瞬时边界做有限处理；不把消融场景计为通过。

  `camera` runtime 后与夹具修正前摘要：正式 A03 二进制的原 Playground `camera` public runner manual/validator `0/0`，11/11 功能结果覆盖 `camera_first_person`，1,032/1,189 个相机 trace 为实际第一人称，135 个姿态样本；graph error/fault/temporal/watchdog 为 0。`state_machine` 的两个缺项在未启用 A03 的消融中仍相同，不作为本编号修复目标；本编号只需避免旧夹具在 `3.9 s` 人为将 View yaw 从 `-90°` 瞬时切到 `+90°` 时把新合法身体响应跨边界比较。复用既有 `rebase_temporal_fixture`，只在这一已观测跨越帧重置视觉历史，再用同一原场景报告核对时序告警消失，未覆盖项仍如实失败。

  有限夹具修正 runtime 前摘要：在 `StateMachineRotate` 已定义的 `3.9 s` 分支跨越帧调用现成 `rebase_temporal_fixture`，没有新增状态、阈值或生产旋转路径；此前同 A03 正式二进制的原报告是该条件缺失时的最小自动 red 复现，消融版则证明两项功能覆盖与此无关。`camera` 已通过 A03 可见功能 Gate；现在仅限流重建并重跑一次原 `state_machine`，核对 temporal/fault 是否清零，保持其旧覆盖失败原样报告。

  完成摘要：正式 A03 的系统反例覆盖 Grounded 静止、Grounded 两种非瞄准旋转模式、持续主限角及 Air，源目标 View yaw 与结果数值均通过；去掉 `FirstPerson→Aiming` 条件稳定重现静止 actor yaw=0，恢复后旋转模块 23/23。原 Playground `camera` 功能/视觉 runner manual/validator `0/0`，11/11 结果、1,032 个真实第一人称相机采样、图错误/fault/temporal/watchdog 0。`state_machine` 在修正其人工 3.9 s 输入跳变边界后，temporal/fault/图错误归零；但仍因 `p1_turn_and_rotate_both_sides` 与 `generated_stop_quick_transition_notify` 两项退出 91，这两项在 A03 运行时分支消融的同场报告中也缺失，故不把它们算作本项通过或修改其 producer。完整性能预算按用户暂停范围延期。

- [x] **BF124.A04-S1 — 空中旋转策略源合同与失败反例**：开始：2026-09-23 09:08:15 +08:00；预估结束：2026-09-23 09:43:15 +08:00；实际结束：2026-09-23 09:18:11 +08:00；偏差：-25m04s；结果：实例源枚举、共享 reader 取证与真实系统 red 反例 Gate 通过。UE `RefreshInAirRotation` 的非瞄准策略由实际角色 Settings 值选择，目标目前在 Air 沿用地面 TargetYaw。估时依据为源 C++ switch、两份资产原生读取和现有旋转响应夹具；拆分结果：S2 使用可复跑目标配置实现与真实 Playground Gate，A03 已关闭且常规 Air 是本编号范围。

  原生读取边界：fresh 本地 `AlsCharacterSettings.h` 类默认 `RotateToVelocity`，但当前 `CS_Als_Default` 资产的 `InAirRotationMode` 经仓库 reader 的 `ExportTextItem_Direct` 输出 `(INVALID)`，不能用类默认推断实例值。对照 UE `FEnumProperty::GetUnderlyingProperty/GetSignedIntPropertyValue` 后，仅在通用资产读取器追加枚举底层整数和枚举名两个只读取字段，不修补原始资产或猜值；先自动重建 reader，再重取同一源资产和核对脚本 watchdog 报告，然后决定失败反例。

  fresh 枚举事实：reader 自动重建、watchdog `success=true/exit_reason=completed`，同一 `CS_Als_Default` 的原始 `InAirRotationMode` 底层值为 `3`，枚举名 `EAlsInAirRotationMode_MAX`，UE `ExportText` 因哨兵值输出 `(INVALID)`；C++ switch 的 `default` 分支在该值下执行 `RefreshTargetYawAngleUsingActorRotation`，与类默认值 `RotateToVelocity=2` 不同。不能修补原资产或改为类默认；下一步 fresh 读当前角色 CDO 的 Settings 引用，证明此资产确实为实际实例输入，再以无瞄准 Air 保持 Actor yaw/目标重置建立反例。

  活动 source 闭包：fresh 原生 `B_Als_Character` CDO 的 `Settings` 明确指向 `/ALS/ALS/Data/Character/CS_Als_Default`，所以 `InAirRotationMode=3/EAlsInAirRotationMode_MAX` 确为本项目角色输入。源 enum 有 0 KeepWorld、1 KeepViewSpace、2 RotateToVelocity，值 3 落入 C++ default 并执行 Actor yaw 保持/TargetYaw 重置；这不是为了达成预期而可改写的原始资产错误。目标 `RotationParams` 当前无该资产字段，Air 的 View/Velocity 非瞄准路径仍沿地面 TargetYaw 选择。下一步先让实际旋转系统在移动/静止 Air 反例失败；具体目标配置生成与实现放 S2。

  完成摘要：`run_ue_source_reader.ps1 -SelfTest` 的停滞进程/子进程终止、完成保留、机器报告与 reader 构建状态 5/5 通过；新旧同资产导出从仅 `(INVALID)` 变为底层 `3`/枚举名 `EAlsInAirRotationMode_MAX`，不修改原始 uasset。C++ default 路径源合同要求非瞄准 Air 保持 actor yaw 并把 Target/SmoothTarget 设为当前 Actor；真实 `rotation_response_system` 反例在 `VelocityDirection`、`ViewDirection` 移动/静止三类输入中的首例稳定 FAIL（原 actor `0.3 rad` 被旧目标转为 `0.09614921 rad`）。反例暂置 ignored，S2 接入实际源值后启用；未把类默认 `RotateToVelocity=2` 当作当前实例事实。

- [x] **BF124.A04-S2 — 源空中旋转配置生成与角色运行时**：开始：2026-09-23 09:19:46 +08:00；预估结束：2026-09-23 10:09:46 +08:00；实际结束：2026-09-23 09:29:54 +08:00；偏差：-39m52s；结果：源配置可复跑生成、最小修复/消融与原 Playground 功能 Gate 通过。用仓库原生 reader 从当前 `B_Als_Character` CDO→`CS_Als_Default` 生成目标 `3/MAX`，不改原资产；本实例非瞄准 Air 走 C++ default 的 Actor yaw 保持/TargetYaw 重置，Aiming/FirstPerson 仍由 A03 处理。估时依据：一个输入资产、一个 Air 分支、一个已有失败系统测试与现成 public 场景；拆分结果：A04 可综合关闭，未证实的 mode1/2 明确由生成器拒绝，A05 未启动。

  runtime 前摘要：新离线工具自测验证 CDO 引用、源 enum `3/MAX`、支持的 `0/KeepWorld` 和拒绝未实现 `2/RotateToVelocity`；fresh 双资产经仓库 reader 完整生成目标 `in_air_rotation_mode.rs=3u8`，成功报告只指向源对象路径，不改 uasset。运行时读取该目标值，普通非瞄准 Air 仅对当前已支持的 0/3 源语义保持 Actor yaw 并把 Target/SmoothTarget 复位，Aiming/FirstPerson 仍走已验路径。S1 移除 ignored 后反例及旋转模块 24/24 PASS；消融禁用新 Air 分支再次稳定 FAIL、旧 yaw `0.09614921 rad`，源码已恢复。未加未证实 mode1/2 算法或新依赖；开始同 `target_game/play` 限流构建和原 `uphill_jump` 功能 Gate。

  首次 runtime 后/基座复验前摘要：同二进制原 Playground `uphill_jump` public runner manual/validator `0/0`，6/6 结果覆盖 3 组楼梯、18/18 实际姿态窗口、1,408 个 pose samples；graph error/fault/temporal/watchdog 为 0，渲染/贴花通过，完整性能预算依用户暂停范围延期。目标差异影响所有普通 Air，因此再用同 `target_game/play` 对现有 `moving_base` 的 Air 跳离→新支撑→Mantle 返回做一次必要跨路径复验，若失败先审计该报告。

  完成摘要：同二进制 `moving_base` public runner manual/validator `0/0`，10/10 结果覆盖 Ride/Jump/新支撑/Mantle/返回、2 个姿态窗口与 404 pose samples；graph error/fault/temporal/watchdog 为 0，渲染/贴花通过。`uphill_jump` 与基座两个原 Playground 入口均证实 Air→Grounded/动作恢复无回归。生成器与报告记录当前源 CDO/Settings 身份和 raw `3`，self-test 拒绝未实现 mode2 与错误 CDO 链；目标只留一个生成值及非瞄准 Air 保持分支，未触碰 AnimBP 转换规则或原始资产。完整性能预算依用户暂停范围延期。

- [x] **BF124.A04 — 空中旋转独立策略**：开始：2026-09-23 09:30:48 +08:00；预估结束：2026-09-23 09:45:48 +08:00；实际结束：2026-09-23 09:30:48 +08:00；偏差：-15m00s；结果：S1 源合同/失败反例与 S2 生产修复/两场真实功能 Gate 已各自提交，工作树干净。源项目角色实际引用的设置值是 `EAlsInAirRotationMode_MAX=3`，C++ switch 因而保持普通 Air Actor yaw；目标现在由离线生成的值驱动该行为，Aiming/FirstPerson 继续走已验独立路径。最小反例、消融、24 项旋转回归、原 Playground 上跳和移动基座均通过；完整性能预算依用户暂停范围延期。估时依据：只复核两个已通过子项和提交/索引；拆分结果：本编号无剩余功能 Gate，A05 是下一项。

- [x] **BF124.A05-S1 — 连续 GaitAmount 旋转曲线源闭包与反例**：开始：2026-09-23 09:31:35 +08:00；预估结束：2026-09-23 10:16:35 +08:00；实际结束：2026-09-23 09:39:15 +08:00；偏差：-37m20s；结果：源设置/两条递归曲线与真实系统 red 反例 Gate 通过。fresh 本地 `CalculateGroundedMovingRotationInterpolationHalfLife` 以 `max(1,GaitAmount)` 对实际 gait 曲线连续求值后乘 View yaw 响应；当前角色 CDO 的 MovementSettings 指向 `MS_Als_Normal`，目标虽有 `AlsGaitAmount`，旋转却只按离散 Gait 取三点。估时依据：一份 movement 设置、实际仅两条唯一曲线和现有 gait/rotation 夹具；拆分结果：S2 生成目标曲线并实现/真实入口验收。

  source/static 闭环：仓库原生 reader 对 `MS_Als_Normal`、`CF_Als_RotationInterpolationSpeed_Normal`、`Responsive` 的 watchdog 均 `success=true`；实际六个 RotationMode×Stance 槽位中仅 VelocityDirection/Standing 选 Responsive，其余五个选 Normal。fresh RichCurve key 未显式存 InterpMode，UE `FRichCurveKey` 默认 `RCIM_Linear`：Normal 在 gait 0/1/2/3 为 `0.15/0.15/0.15/0.05 s`，Responsive 为 `0.10/0.10/0.05/0.05 s`；源 2.5→Normal `0.10 s`、1.5→Responsive `0.075 s`。目标 VelocityDirection/Standing 还误选 Normal profile，非仅插值问题。真实 `rotation_response_system` 的 ViewDirection/Standing、连续 amount=2.5 反例先 FAIL：actor yaw `0.32282805 rad`、源曲线预期 `0.43643063 rad`；第二个 VelocityDirection/Standing amount=1.5 边界已编入同一 focused 测试，暂置 ignored。原始资产未改，S2 需复用原生 reader/现有目标数据路径生成这两条曲线并核对 profile 消费。

- [x] **BF124.A05-S2 — 生成并采样连续旋转曲线**：开始：2026-09-23 09:40:55 +08:00；预估结束：2026-09-23 10:35:55 +08:00；实际结束：2026-09-23 09:57:30 +08:00；偏差：-38m25s；结果：曲线生成、连续采样/档位选择、消融及两场原 Playground 功能 Gate 通过。复用仓库原生 reader 与 RichCurve parser 将 `MS_Als_Normal` 两条实际线性 profile 生成为目标数据；运行时消费已有 `AlsGaitAmount`，源 `max(1,amount)` 线性采样，Standing VelocityDirection 选 Responsive、Crouching 选 Normal。估时依据：两条唯一曲线、一个连续 producer 和既有参数/测试；拆分结果：A05 可汇总关闭，不新建通用曲线框架或触碰 AnimBP 转换规则。

  runtime 前摘要：新生成器 self-test 验证六槽位、两条曲线及非线性拒绝；fresh 原生 reader 重取 `MS_Als_Normal` 与两条 CurveFloat 后生成目标 `rotation_half_lives.rs`，数值为 Normal `[0.15,0.15,0.05]`、Responsive `[0.10,0.05,0.05]`，没有原始资产修改。目标在已有 `AlsGaitAmount` 上按源 `max(1,amount)` 连续线性采样、端点精确返回源 key，Standing VelocityDirection 用 Responsive，Crouching 用 Normal；S1 真实系统三案例与 `als-core --lib` 284/284 通过。消融将采样改回离散时第一案例 FAIL、Standing profile 改回 Normal 时第二案例 FAIL、Crouching 改 Responsive 时第三案例 FAIL，三次均已恢复。当前仅一个目标数据文件和局部参数/consumer 改动；接下来同 `target_game/play` 限流构建，原 Playground `sprint` 与 `standing_response` 公共功能 Gate。

  `sprint` runtime 后/`standing_response` 前摘要：同 `target_game/play` 的原 Playground `sprint` public runner manual/validator `0/0`，8/8 结果包含实际 Sprint、曲线 relay、加速与稳态分支，2 个姿态窗口与 283 pose samples；graph error/fault/temporal/watchdog 为 0，渲染/贴花通过。接下来不重编同二进制，运行 `standing_response` 核对普通 ViewDirection 与静止响应；性能继续按用户暂停范围延期。

  完成摘要：同二进制 `standing_response` public runner manual/validator `0/0`，6/6 功能结果、9 个响应姿态窗口与 128 pose samples；graph error/fault/temporal/watchdog 为 0，渲染/贴花通过。源 Normal 2.5、Responsive 1.5、Crouching Normal 1.5 的真实系统数值及 1/3 精确 key 端点回归通过；曾由 `lerp` 精度使 gait=3 返回 `0.049999997`，现直接取源端点 `0.05`，`als-core --lib` 284/284 通过。三种消融分别证明连续插值、Standing profile 和 Crouching profile 均必要；完整性能预算依用户暂停范围延期。
