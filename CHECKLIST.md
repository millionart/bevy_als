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

- [x] **BF124.A05 — 连续 GaitAmount 旋转曲线**：开始：2026-09-23 09:58:18 +08:00；预估结束：2026-09-23 10:13:18 +08:00；实际结束：2026-09-23 09:58:18 +08:00；偏差：-15m00s；结果：S1 源曲线/反例与 S2 离线生成/生产采样/两场真实功能 Gate 均单独提交，工作树干净。当前 `MS_Als_Normal` 只引用 Normal/Responsive 两条线性曲线，目标按 `AlsGaitAmount` 连续采样并保持 Standing/Crouching 原选曲；核心 284 项、三项消融及 `sprint`/`standing_response` 原 Playground 报告通过。完整性能预算依用户暂停范围延期。估时依据：复核已通过子项与提交/索引；拆分结果：本编号无剩余功能 Gate，A06 是下一项。

- [x] **BF124.A06 — 落地残余移动不得解除旋转阻止**：首次开始：2026-09-23 09:59:42 +08:00；首次预估结束：2026-09-23 10:44:42 +08:00；首次实际结束：2026-09-23 10:08:54 +08:00；首次偏差：-35m48s；2026-09-23 10:16:22 +08:00 因 0.1 m/s 新边界复开，R1 于 10:24:13 +08:00 通过并最终关闭。fresh UE 落地阻止标志只在有新输入的 Grounded moving Velocity/View 分支解除，静止 VelocityDirection blocked 仍保留上次 TargetYaw；目标现覆盖 1 m/s、0.1 m/s 与零残速及明确新输入的生命周期。估时依据：一个已有状态、一个 source Grounded switch 和现成上跳场景；拆分结果：R1 单独验收/提交，A07 S2 现在可继续。

  source/static 与 runtime 前摘要：fresh 本地 UE `AlsCharacter.cpp` 落地时设置 blocked、模式切换设置 blocked；仅 Grounded moving 且 Velocity/View 非瞄准满足 `bHasInput || !blocked` 才进入消费目标的分支，无输入而被阻止则落到 `RefreshTargetYawAngleUsingActorRotation`。目标原 `moving || has_input` 清除在 Landing 后把残余速度误作新输入。真实 `rotation_response_system` 反例 Air→Grounded、无输入但水平速度 1 m/s 的 Velocity/View 两模式先稳定 FAIL，修复后 blocked/Actor yaw/Target/SmoothTarget 连续两帧保持，明确新输入才解锁；Air 与 FirstPerson/Aiming 的原标志生命周期也加在既有 focused 测试中。消融早清标志或移除目标 yaw 保持分别 FAIL；消融额外的早期 Smooth 重置和 actor 直持分支信号不变，已删除冗余分支，仅保留必要条件。旋转模块 26/26 通过。现在同 `target_game/play` 限流构建并运行原 Playground `uphill_jump` 公共落地/姿态 Gate，性能预算继续延期。

  完成摘要：同 `target_game/play` 重建后原 Playground `uphill_jump` public runner manual/validator `0/0`，6/6 结果与 18/18 上跳落地姿态窗口、1,316 pose samples；graph error/fault/temporal/watchdog 均为 0，渲染/贴花通过。原 red 中 0.4 rad Actor yaw 在首个无输入残速帧不再被速度 yaw 拉走，Target/SmoothTarget 同步保持，直到显式新输入才解除 blocked；移除早清限制或移除目标保护各自重现失败。无资产/图/阈值修改，完整性能预算按用户暂停范围延期。

  复开复盘：A07 当次 fresh 源 `RefreshGroundedRotation` 的静止 VelocityDirection 分支在 blocked=true 时强制用 `LocomotionState.TargetYawAngle`，无论 `bHasVelocity`；目标 A06 的 `blocked_residual_motion` 只覆盖 `moving=true`，当无输入速度降到 `0.1 m/s`（高于 has-velocity 0.01、低于 moving 0.5）时重新消费实际速度 yaw。旧 focused 只枚举 `1.0 m/s` 和 `0`，真实上跳 Gate 未逐帧断言该狭窄速度区间，故漏检。先取消本项勾选并以 R1 补反例和有限修复；A07 只保留已提交 S1，不启动 S2。

  - [x] **BF124.A06-R1 — 静止低残速 blocked 目标选择**：开始：2026-09-23 10:16:22 +08:00；预估结束：2026-09-23 10:56:22 +08:00；实际结束：2026-09-23 10:24:13 +08:00；偏差：-32m09s；结果：静止残速源分支、最小反例/消融及原 Playground 功能 Gate 通过。旧真实系统反例扩展到 `0.1 m/s` 的 Velocity/View、标志/目标/Actor 连续帧；仅在源静止 VelocityDirection blocked 分支复用上次目标，旋转相关回归和上跳场景通过。估时依据：一条源静止分支、一个目标 match 守卫和现成测试/场景；拆分结果：A06 可恢复关闭，A07 S2 再继续。

    runtime 前摘要：扩展旧系统反例后，VelocityDirection 在无输入 `0.1 m/s` 首帧旧 actor yaw 从 `0.4` 变为 `-0.29828933 rad`，目标实际速度 yaw 被错选，稳定 FAIL；fresh 源静止 blocked 分支明确使用上次 TargetYaw。仅在该分支保留 runtime.target_yaw，focused PASS；删除该守卫后再稳定 FAIL，已恢复。旋转模块初次回归暴露旧 `velocity_direction_retains_rotation_target_after_velocity_stops` 用默认 blocked=true/无输入却期待转向速度，违背源条件；把其前提改为已解除 blocked 的状态后 26 passed/1 个 A07 ignored。无新状态/资产/阈值，现限流重建同 `target_game/play` 并复验原 Playground 上跳落地。

    完成摘要：同 `target_game/play` 原 `uphill_jump` public runner manual/validator `0/0`，6/6 功能结果、18/18 姿态窗口、1,333 pose samples；graph error/fault/temporal/watchdog 为 0，渲染/贴花通过。`0.1 m/s` 的 source-blocked 反例由 `yaw=-0.29828933` 的 FAIL 修为维持 `0.4 rad`，删除守卫再次 FAIL；旧错误前提测试已改为明确“此前输入已解除 blocked”，不把旧目标误行为当成回归要求。无生产新状态，完整性能预算依用户暂停范围延期。

- [x] **BF124.A07-S1 — 碰撞前期望速度朝向源/目标链与反例**：开始：2026-09-23 10:10:22 +08:00；预估结束：2026-09-23 10:55:22 +08:00；实际结束：2026-09-23 10:14:39 +08:00；偏差：-40m43s；结果：源 Settings/CMC 时序、目标公开状态与真实响应系统 red 反例 Gate 通过。fresh UE `PhysWalking` 缓存碰撞前 `MoveVelocity`，角色一次性消费并在低速/缺失时回退实际 VelocityYaw；当前设置开关为 true，目标此前只用碰撞后朝向。估时依据：两个源函数、一个现有目标马达→旋转链及受控滑墙输入；拆分结果：S2 接入目标数据与实际物理/Playground Gate，A08 不提前展开。

  source/static 闭环：fresh 原生 `CS_Als_Default` 读取 watchdog `success=true` 且 `bRotateTowardsDesiredVelocityInVelocityDirectionRotationMode=True`；本地 CMC 的 `MoveVelocity=Velocity` 在 `MoveAlongFloor` 前形成，实际位移/时间覆写 `Velocity` 前写入一次性缓存，角色只在水平量 ≥1 cm/s 时用期望 yaw，否则回退实际 yaw。目标 `grounded_dynamics_motor_system` 于 Tnua Logic 后/Motors 前写现有 `AlsMovementBaseDynamicsState(world_velocity,motor_boost,support_velocity)`，其中 `world_velocity+motor_boost` 是当帧马达碰撞前水平世界速度；`snapshot_actual_velocity_system` 在 Physics 后刷新碰撞后的 `LinearVelocity`，`rotation_response_system` 随后消费，Grounded 时保留当前马达状态、Air 时清空。真实旋转系统夹具令马达意图 `-Z`、碰撞后实际侧滑 `+X`、输入有效，旧目标 `target_yaw=-π/2` 而源期望 `0`，稳定 FAIL。反例暂置 ignored；S2 需确保当帧期望状态有效、低速/缺失回退及不影响基座/Air 生命周期，然后修复与验收。

- [x] **BF124.A07-S2 — 一次性碰撞前目标 yaw 消费**：开始：2026-09-23 10:26:16 +08:00；预估结束：2026-09-23 11:16:16 +08:00；实际结束：2026-09-23 10:39:31 +08:00；偏差：-36m45s；结果：最小一帧速度、低速/缺失回退、四项消融、核心回归与两场原 Playground 功能 Gate 通过。复用 `AlsMovementBaseDynamicsState` 的马达阶段结果，PostPhysics 旋转响应优先消费有效且水平 ≥源 1 cm/s 的方向，无值/低速回退实际速度；Grounded/Air 生命周期不复用陈旧值。估时依据：现有一个状态组件、一个 motor writer、一个 rotation consumer 和公共场景；拆分结果：A07 可汇总关闭，A08 不提前展开。

  runtime 前摘要：`grounded_dynamics_motor_system` 在源等价碰撞前阶段把 `world_velocity+motor_boost` 放入既有状态的 `pre_collision_velocity: Option<Vec3>`；`rotation_response_system` 无论是否因动作跳过均一次性 take，Grounded VelocityDirection 且水平 ≥现有源阈值 1 cm/s 时优先使用，否则回退后碰撞实际 VelocityYaw。Air 的现有快照清理已加断言。S1 受控“马达 -Z、物理侧滑 +X”反例启用后源期望 TargetYaw=0，通过；下一帧无新样本与 0.005 m/s 低速样本分别回退实际 -π/2；马达 writer 断言样本等于实际 pre-physics world+boost。四项消融（不写值、不消费目标、不 take、忽略低速阈值）各自 FAIL 并已恢复，最终 `als-core --lib --jobs 2` 286/286。无新组件、资产/图/阈值或依赖更改；开始同 `target_game/play` 限流构建，再用原 Playground `stairs` 和 `moving_base` 功能 Gate。

  `stairs` runtime 后/基座前摘要：同 `target_game/play` 原 Playground `stairs` public runner manual/validator `0/0`，9/9 功能结果覆盖 01/02/03 三种阶高的上下六路线，18 个运动/姿态窗口、3,228 条物理运动 trace 与 1,935 pose samples；graph error/fault/temporal/watchdog 为 0，渲染/贴花通过。保持同二进制再跑 `moving_base`，核对携带/跳离/新支撑/Mantle 返回及本次世界/相对速度分解无回归；完整性能预算依用户暂停范围延期。

  完成摘要：同二进制 `moving_base` public runner manual/validator `0/0`，10/10 结果覆盖 Ride/Jump/新支撑/Mantle/返回、2 个姿态窗口与 403 pose samples；graph error/fault/temporal/watchdog 为 0，渲染/贴花通过。源开关 true 与 pre-physics MoveVelocity 合同用现有 motor 世界速度加 boost 重构；受控侧滑原 `target_yaw=-π/2` 现在按马达期望为 `0`，一次消费后与 0.005 m/s 低速时准确回退实际 `-π/2`。四项消融和核心 286/286 均通过，完整性能预算依用户暂停范围延期。

- [x] **BF124.A07 — VelocityDirection 碰撞前期望速度朝向**：开始：2026-09-23 10:40:27 +08:00；预估结束：2026-09-23 10:55:27 +08:00；实际结束：2026-09-23 10:40:27 +08:00；偏差：-15m00s；结果：S1 源/目标合同与 red 反例、S2 最小一次性消费和真实入口 Gate 分别提交，工作树干净。当前角色源设置优先碰撞前期望速度，目标现从已有马达状态消费一次并对缺失/低速回退；核心 286 项及原 Playground `stairs`/`moving_base` 两场功能报告通过。完整性能预算依用户暂停范围延期。估时依据：仅复核已完成两个子项及索引/提交；拆分结果：本编号无剩余功能 Gate，A08 是下一项。

- [x] **BF124.A08 — 动作期间 TargetYaw 与瞄准限角生命周期**：开始：2026-09-23 10:42:41 +08:00；预估结束：2026-09-23 11:27:41 +08:00；实际结束：2026-09-23 10:56:06 +08:00；偏差：-31m35s；结果：源 late 生命周期、真实系统 red/修复/消融与两场原 Playground 功能 Gate 通过。目标 Mantle/Roll/Action 早退时现按当帧 Actor 保持 Target/SmoothTargetYaw、刷新 View/模式历史并让瞄准限角下一帧重置，Actor 自身旋转不被此系统改写。估时依据：一个早退入口、已有响应状态/夹具和两个公共场景；拆分结果：本编号无剩余功能 Gate，A09 独立处理 Roll 阻尼。

  source/static 与 red 反例：fresh `AlsCharacter.cpp` Tick 顺序为 Early 清 `bAimingLimitAppliedThisFrame`→RefreshView/Locomotion/Rotation→Super Tick→Late；Late 在 LocomotionMode 无效或动作有效时仍调用 `RefreshTargetYawAngleUsingActorRotation`，再令 `bResetAimingLimit=!bAimingLimitAppliedThisFrame`。目标动作早退漏掉两者，也留存上一帧 View yaw/模式历史。扩展已有 `custom_mode_preserves_actor_rotation_and_physical_stance` 夹具：Actor yaw=-0.6、旧 Target/Smooth=1.0、先前限角 active70/applied=true，Grounded+Mantling 下一帧旧 Target 未复位，稳定 FAIL；源期望 Target/Smooth=-0.6，限角下一帧 reset。只改目标生命周期，动作期间 Actor Transform/Rotation 保持不动。

  runtime 前摘要：只在现有 Mantle/Roll/Action 或无效 LocomotionMode 早退路径同步 Target/SmoothTarget 至当帧 Actor、刷新 View/模式历史并标记限角本帧未应用；普通 Actor 旋转与 Action 资产不改。真实系统测试追加 Mantling→Roll 期间 Actor yaw 两次变化、Target/Smooth/View 历史跟随，以及退出动作后原 70° 限角重置并重新应用；三项消融（跳过目标维护、留存限角 applied、留存旧 View 历史）分别 FAIL，源码均恢复。`als-core --lib --jobs 2` 286/286 通过。复查 public 场景后，`locomotion` 仅含 Jump 而不覆盖这里的 Roll/Mantle，故把真实 Gate 修正为现有 `grounded` Roll 与 `moving_base` Mantle；同 `target_game/play` 限流构建后各运行一次，性能预算沿用户暂停范围延期。

  `grounded` runtime 后/`moving_base` 前摘要：同 `target_game/play` 原 Playground `grounded` public runner manual/validator `0/0`，15/15 功能结果覆盖 Roll 开始/Slot/结束/退出过渡、站蹲、移动瞄准与快速停止，9 个动作姿态窗口与 1,895 pose samples；graph error/fault/temporal/watchdog 为 0，渲染/贴花通过。保持同二进制再跑移动基座 Mantle 动作/返回，验证另一早退路径，性能继续延期。

  完成摘要：同二进制原 Playground `moving_base` public runner manual/validator `0/0`，10/10 功能结果覆盖 Ride/Jump/新支撑/Mantle/动作返回、2 个姿态窗口与 401 pose samples；graph error/fault/temporal/watchdog 为 0，渲染/贴花通过。真实系统原动作路径 Target=1.0 保留的 FAIL 已修为随 Actor -0.6→-1.1 连续更新，View 历史跟随，Roll 退出后旧 70° 限角重新计算到 >100°；三项消融各自 FAIL、`als-core` 286/286 通过。无资产/图/阈值修改，完整性能预算依用户暂停范围延期。

- [x] **BF124.A09 — Roll 近半圈角差的源转向偏好**：开始：2026-09-23 10:58:05 +08:00；预估结束：2026-09-23 11:43:05 +08:00；实际结束：2026-09-23 11:10:53 +08:00；偏差：-32m12s；结果：源函数/资产、近半圈 red/修复/消融与原 Playground Roll 功能 Gate 通过。目标 Roll 阻尼现按同一 175° 源阈值从 +176° 选逆时针，并消费源 `InvExpApprox` 半衰期，资产/图不变。估时依据：一个 C++ helper、一处目标函数和已有动作场景；拆分结果：本编号无剩余功能 Gate，A10 独立处理蹲起缩放。

  source/static 与 red 反例：fresh 原生 `CS_Als_Default.Rolling.RotationInterpolationHalfLife=0.1 s`，reader watchdog `success=true`；C++ `RefreshRollingPhysics` 的非零半衰期路径调用 `UAlsRotation::DamperExactAngle`，源函数先 `UnwindDegrees`，对 `delta>175°` 减去 `360°`，使用 `UAlsMath::DamperExactAlpha(InvExpApprox)` 再 Unwind。仓库已有 `transitions::remap_counterclockwise_degrees` 正是该 175° 规则，`animation::inv_exp_approx` 已用于普通身体 yaw。目标 Roll 当前 +176°/0.1 s 得 `+1.5358897 rad`，源合同预期 `-1.6060647 rad`，真实 helper focused 先稳定 FAIL；174°、175°、180° 正反边界同时登记，不新增第二套阈值。

  runtime 前摘要：Roll helper 现在以已有 `COUNTERCLOCKWISE_REMAP_THRESHOLD_DEGREES=175°` 在弧度域比较，`delta>175°` 时减 `2π`，并复用现有 `inv_exp_approx` 求半衰期比例；先转回度会使精确 175° 因 f32 round-trip 误入逆时针分支，已在 focused 反例中定位并删除该转换。旧测试把 UE 的近似衰减当作精确 0.5，已改为源 `InvExpApprox` 正值。174/175/176/180° 与 Roll 模块 8/8 通过；消融逆时针映射时 176° 首项 FAIL，消融源指数近似时数值差约 0.00036 rad 再 FAIL，均恢复。无新阈值/状态/资产/图或依赖；准备同 `target_game/play` 限流构建，运行原 Playground `grounded` Roll 公共功能/姿态 Gate，完整性能预算依用户暂停范围延期。

  完成摘要：同 `target_game/play` 限流构建后，原 Playground `grounded` public runner manual/validator `0/0`，15/15 功能结果覆盖 Roll 开始/Slot/结束/退出过渡及站蹲/瞄准，9 个姿态窗口、1,992 pose samples；graph error/fault/temporal/watchdog 为 0，渲染/贴花通过。focused +176°/0.1 s 从旧 `+1.5358897 rad` 修为源 `-1.6060647 rad`，174°、精确175°和180°边界同验；两项消融分别 FAIL，Roll 8/8 通过。完整性能预算依用户暂停范围延期。

- [x] **BF124.A10-S1 — 蹲起有效缩放与净空源合同/反例**：开始：2026-09-23 11:13:05 +08:00；预估结束：2026-09-23 11:58:05 +08:00；实际结束：2026-09-23 11:18:34 +08:00；偏差：-39m31s；结果：UE/Avian 缩放空间合同与两个最小物理/几何 red 反例通过。fresh UE Crouch/UnCrouch 使用 Capsule 世界 `GetShapeScale=三轴绝对值最小值`，中心位移和站起净空均以该有效缩放换到世界；目标两个消费者仍用未缩放值。估时依据：UE 两个函数、目标一个 capsule_reshape consumer 与既有物理夹具；拆分结果：S2 实施及真实 Playground Gate，A11 不提前展开。

  source/static 闭环：本地 UE `UCapsuleComponent::GetShapeScale` 调用 `GetComponentTransform().GetMinimumAxisScale()`，后者取 `abs(X/Y/Z)` 的最小值；Crouch 的 `ScaledHalfHeightAdjust=unscaled_delta×ComponentScale` 保持底部，UnCrouch 的 `GetPawnCapsuleCollisionShape` 从已缩放半高/半径再按该世界差补足 Standing query。目标 `capsule_center_for_stance` 未乘 scale，站起直接把 `capsule_for` 的 scale=1 查询体交给 `shape_intersections`；Avian 查询消费给定 Collider 的 `shape_scaled`，不会自动套角色 Transform。新最小反例 scale=2 时源蹲下中心/底部期望 `1.12 m`，旧目标 `1.46 m`，稳定 FAIL；非均匀/负 scale 用绝对最小轴核对。独立 3.2 m 低天花物理夹具中，scale=2 的蹲姿可容身、站姿应受阻，旧目标却进入 Standing，稳定 FAIL。两测试暂置 ignored；旧清单只覆盖 scale=1，未检出这两个单位边界。S2 只改中心换算与候选站姿查询体，保留原 Playground 地形和源资产。

- [x] **BF124.A10-S2 — 缩放中心与候选站姿查询修复**：开始：2026-09-23 11:20:49 +08:00；预估结束：2026-09-23 12:10:49 +08:00；实际结束：2026-09-23 11:29:55 +08:00；偏差：-40m54s；结果：源缩放修复、两条红绿物理/几何反例、双消融、核心及默认原 Playground Gate 通过。按源 Capsule 有效最小绝对轴 scale 修正 Stand/Crouch 中心世界位移，并对站起候选查询体应用同一世界 scale；物理组件仍由原尺寸/Transform 所有权管理。估时依据：一个 helper 参数、一个 Collider query scale 和已有夹具；拆分结果：scaled 可见角色实际入口留 S3，A11 不提前展开。

  runtime 前摘要：当前真实角色在 `character_assembly` 中以根实体生成，胶囊的 Transform scale 即其组件世界 scale；目标以 `abs(X/Y/Z).min` 计算 UE 有效 scale，中心半高差乘该值，站起查询的候选 Collider 单独 `set_scale(Vec3::splat(effective),16)`，成功写入物理组件时仍用原未缩放定义供 Avian 正常同步。S1 的 scale=2 中心旧 `1.46 m`→源 `1.12 m`、负/非均匀 min-abs 基底与 3.2 m 低天花拒站 focused 均 PASS；去掉中心乘数或查询 set_scale，各自稳定 FAIL 并已恢复；scale=1 原测试与 `als-core --lib --jobs 2` 289/289 通过。无新组件/资产/图或阈值。下一步同 `target_game/play` 限流构建并运行原 Playground `grounded` 功能 Gate，后续 scaled 可见路径单独核验。

  完成摘要：同 `target_game/play` 原 Playground `grounded` public runner manual/validator `0/0`，15/15 结果覆盖 Roll、站蹲与退出过渡，9 个姿态窗口、2,006 pose samples；graph error/fault/temporal/watchdog 为 0，渲染/贴花通过。scale=2 与负/非均匀有效 scale 的 focused 反例、两项消融与核心 289/289 已闭合。该真实入口角色仍为 scale=1，故不把它冒充可见 scaled 角色验收；原场景内的 scaled 路径单独列 S3。

- [x] **BF124.A10-S3 — 原 Playground 可见缩放角色蹲起 Gate**：开始：2026-09-23 11:33:34 +08:00；预估结束：2026-09-23 12:18:34 +08:00；实际结束：2026-09-23 12:07:22 +08:00；偏差：-11m12s；结果：scale=2 可见角色、100 次蹲起/低顶阻站/实际恢复、公共功能 Gate 与缩放分支消融通过。复用原 `base` 的 CrouchClearance 地形、角色、相机和 canopy，先保留 scale=1 前序、再进入同一角色 scale=2 阶段、结束后恢复；没有场外空白世界。估时依据：既有场景阶段/角色和 canopy、已通过的物理夹具；拆分结果：A10 可汇总关闭，A11 不提前展开。

  runtime 前摘要：`crouching` 现有场景前序方向/状态保持 scale=1，进入 `CrouchClearance` 才在同一原角色根 Transform 设 uniform scale=2；按当前原场景 Standing 底部高度将中心抬升一个额外源 Standing 半高，`stance_center_from_standing_center` 再按 scale=2 换算实际 stance，既有 canopy 由世界 crouch 半高形成顶部上方 8 cm 的阻站间距；离开阶段恢复 scale=1 且用现有 temporal epoch 记录人工 scale 边界。没有新空白世界、角色资产或相机替代。局部测试复算源底部与 canopy 区间 PASS，`cargo test --lib crouch --jobs 2` 24/24。现同 `target_game/play` 限流重建并运行一次原 Playground `crouching` 公共功能/视觉 Gate，重点审计 CrouchClearance 实际 scale/站起阻挡/恢复，性能仍按用户暂停范围延期。

  首次场景审计与入口纠正：同二进制 `crouching` public runner manual/validator `0/0`，7 项方向/转身结果通过、graph error/fault/temporal/watchdog 为 0；但报告中 `CrouchClearance` 样本为 0，源码 `initial_phase(Crouching)=CrouchDirectionRegression`，它不会进入本次缩放分支，故该通过不能算 S3 Gate。fresh 核对 `CrouchRecovery` 的完成出口仅在非 StateMachine 时进入 `CrouchClearance`，而完整原 `base` 场景结果表要求 `crouch_canopy_*`；下一次仅改为 `-Case base`，保持原二进制、场景和 strict 报告，核对实际 scale=2 样本及阻站/恢复。

  `base` 首次 runtime 后摘要：原 Playground 的公共报告确有 `CrouchClearance` 834 个 scale≈2 的姿态样本，100 次蹲起、canopy 可见/阻站、离顶移动均覆盖，graph error/fault/temporal/watchdog 为 0；但阶段运行到固定 90 s 上限仍未 `crouch_canopy_recovered`，其后方向/Mantle/Roll 缺项均是连带结果，manual 退出 91，不能关闭 Gate。报告末从 canopy 初始附近移开约 1.72 m，对角线两个轴各约 1.22 m；现有 scale=1 夹具按中心径向 `1.65 m` 判“离顶”，而实际 canopy 水平半宽约 0.9 m、scale=2 胶囊半径 0.6 m，角部需要中心径向大于 `sqrt(2)×0.9+0.6≈1.873 m` 才能净空，故旧阈值提前停止输入。只按源角色有效半径把该夹具离顶距离从 scale=1 的 1.65 m 增至 scale=2 的 1.95 m，保留原场景、物理和查询算法；先补几何 focused 反例，再同入口另目录复验。

  夹具消融/复验前摘要：几何计算证明增距到约 1.95 m 可避开角部，但现有 `crouch_canopy_recovered` 已是更直接的实际站起/图恢复证据。按最小设计删去 `MoveAway` 中的 1.65 m 中心径向推断，不新增半径公式或状态，只持续发送已有横向输入直到实际恢复，再进入既有 AwaitRecovery；90 s 固定阶段上限仍是无进展退出边界。A10 生产中心/查询修复、源资产与原 Playground 均不动；正在做 crouch focused 回归，随后同入口另目录复验。

  第二次 `base` 后/最终缩放证据 Gate 前摘要：同二进制 public runner manual/validator `0/0`，完整 Base 50/50 结果、24 个姿态窗口、1,440 pose samples，CrouchClearance 有 54 样本且盆骨世界 scale 最高约 `2.0000002`；100 次蹲起与 canopy visible/blocked/recovered/cleaned 均 true，graph error/fault/temporal/watchdog 0，后续 Turn/Mantle/Roll 亦完成。删除旧径向推断后实际恢复信号推进成功，生产算法未改。但旧 Base 结果表未要求 scale=2，理论上移除人工缩放也能通过；现只在同阶段已观测到 canopy 阻站且角色 Transform 有效 scale≈2 时置 `crouch_scaled`，新增必需结果行，再同入口另目录一次最终复验。无新世界、资产或一般阈值。

  最终缩放 Gate 后/消融前摘要：同 `target_game/play` 的原 Playground `base` public runner manual/validator `0/0`，51/51 必需结果中 `crouch_scaled=true`，canopy visible/blocked/recovered/cleaned 均 true；24 个姿态窗口、1,437 pose samples，CrouchClearance 54 样本中 53 个盆骨世界 scale≈2，graph error/fault/temporal/watchdog 为 0，完整性能预算按用户暂停范围延期。为验证新增结果真能拒绝移除 scaled 分支，下一步只暂时移除该阶段的 scale=2 赋值、同入口一次 public Base Gate 作消融，然后立即恢复源码和 `target_game/play` 二进制，不把消融运行计为通过。

  缩放分支消融后摘要：同原 `base` 场景仅暂时删掉 scale=2 赋值，manual/validator 退出 `91/1`；完整结果中唯一未覆盖为新增 `crouch_scaled`，CrouchClearance 的 scale≈2 样本为 0，其他 canopy visible/blocked/recovered/cleaned 均 true，graph error/fault/temporal/watchdog 仍为 0。这证明公共结果能区分真正的 scaled 角色与只跑默认角色的假通过；消融不计成功。现在恢复该一行与正常 `target_game/play`，核对 diff 与局部 Gate 后提交 S3；不重复已通过的同代码 Base 场景。

  完成摘要：scale=2 赋值已恢复，正常 `target_game/play` 重新限流构建成功，局部 canopy 世界底部/间距测试再通过；最终同代码原 `base` 报告 manual/validator `0/0`、51/51 必需结果（含 `crouch_scaled`）、24 姿态窗口、1,437 pose samples，CrouchClearance 中 53/54 样本盆骨世界 scale≈2，canopy 四项与其后的 Turn/Mantle/Roll 均通过，graph error/fault/temporal/watchdog 0。对照消融仅 `crouch_scaled` 失败；完整性能预算依用户暂停范围延期。旧 1.65 m 径向离顶条件被已有真实恢复信号取代，没有新增魔法半径。

- [x] **BF124.A10 — 蹲起中心位移与站起查询的有效缩放**：开始：2026-09-23 12:08:31 +08:00；预估结束：2026-09-23 12:23:31 +08:00；实际结束：2026-09-23 12:08:31 +08:00；偏差：-15m00s；结果：S1 源尺度/两个 red、S2 生产中心/查询修复、S3 原 Playground scale=2 可见 Gate 各自通过并单独提交，工作树干净。UE 有效 scale 为组件世界三轴绝对值最小值；目标中心与候选站姿查询均使用同一尺度。核心 289 项、默认 `grounded` 功能和完整原 `base` 51/51（含 `crouch_scaled`）通过，缩放消融被公共 Gate 明确拒绝；完整性能预算依用户暂停范围延期。估时依据：只复核三个已验收子项和索引/提交；拆分结果：本编号无剩余功能 Gate，A11 是下一项。

- [x] **BF124.A11-S1 — Ragdoll 找地查询原始响应源/目标合同与反例**：开始：2026-09-23 12:11:03 +08:00；预估结束：2026-09-23 12:56:03 +08:00；实际结束：2026-09-23 12:18:35 +08:00；偏差：-37m28s；结果：fresh UE 源/资产合同与真实 SpatialQuery red 反例确认。fresh 本地 UE `RagdollTraceGround` 用角色 UpdatedComponent 的对象通道与 `InitCollisionParams`，后者取该组件原始 BodyInstance 响应和移动忽略集；目标在 Ragdoll 时暂把角色 `CollisionLayers` 设 NONE，却以只排除自身/布娃娃实体的 ALL-mask SpatialQuery 找地。本项重新提取当前角色 Capsule 碰撞响应资产，核对 Avian 查询 mask 与临时所有权快照语义，建立“某层可命中但原角色响应 Ignore”的真实查询最小失败反例；仅做 source/static Gate。估时依据：源三段调用链、目标已有碰撞层所有权和 ragdoll_ground_follow，一个受控 collider 反例；拆分结果：S2 修复/真实入口，A12 缩放尺寸不提前展开。

  source/static 完成摘要：fresh 原生 `B_Als_Character` reader watchdog `success=true`，CDO `CollisionCylinder.BodyInstance` 为 `ObjectType=ECC_Pawn`、`CollisionProfileName=Pawn`，`CollisionResponses` 中 `Visibility=ECR_Ignore`；UE `UMovementComponent::InitCollisionParams` 直接调用 UpdatedPrimitive 的 `InitSweepCollisionParams`，其 `OutResponseParam.CollisionResponse=BodyInstance.GetResponseToChannels()`。目标 `ragdoll_ground_follow` 生成 ALL-mask，仅排除角色和 ragdoll 实体；`AlsCollisionLayerOwnership` 在进入 Ragdoll 暂置 NONE 前已有原始 `CollisionLayers` 快照，但尚无查询 accessor。真实 Avian fixture 放置 Visibility-only 障碍与其下 Default 地面，当前查询稳定错选 Visibility（focused test 编译成功并按预期 red，暂设 ignored 待 S2 开启）；下项直接复用此反例检验原响应 mask，随后用真实 Playground Gate 验收。

- [x] **BF124.A11-S2 — 保留原碰撞响应并完成 Ragdoll Gate**：开始：2026-09-23 12:20:42 +08:00；预估结束：2026-09-23 13:10:42 +08:00；实际结束：2026-09-23 12:36:08 +08:00；偏差：-34m34s；结果：原快照响应查询、物理反例、消融及原 Playground 双场景功能 Gate 通过。依据 S1 已证实的源 Pawn Visibility Ignore、原响应快照和真实 red 反例，缺省角色响应映射目标 Default 层，Ragdoll 找地查询沿用角色进入动作前的过滤响应。估时依据：原快照读取、一个查询 mask、一个共用过滤器函数和已存在真实场景/runner；bundle 默认字段经消融删除。拆分结果：A11 可汇总关闭；A12 蹲起/布娃娃尺寸另行处理。

  source/static 进展、runtime 前摘要：原有 `AlsCollisionLayerOwnership` 快照新增最小过滤层读取，缺失显式组件时按 UE Pawn 的 Default 响应映射；找地 SpatialQuery 使用快照 mask。真实 Avian Visibility-only/Default 地面反例已从 red 转 green，`als-core character::tests::` 5/5。消融新增 bundle 默认字段后反例信号不变，已删除该字段；去掉查询 mask 的 S1 产物稳定 red，证明当前保留分支必要。剩余边界是原 Playground `ragdoll`/`get_up` 实际运行与报告审计，先以同入口 `target_game` 限流构建。

  runtime 首轮后与最终轮前摘要：同入口 `target_game/play` 限流构建通过；原 Playground `ragdoll` 10/10、`get_up` 11/11 功能结果覆盖并通过，报告 graph error/fault/temporal/watchdog 均 0，首轮各 43/66 pose samples，完整性能预算延期。为让反例直接覆盖生产路径，仅抽出一个被生产和测试共用的过滤器构造函数；消融该函数的 mask 后物理反例再次稳定 red，恢复后 green。最终代码只剩快照读取与查询 mask，下一步按同一真实入口复跑两个场景并审计最终报告。

  runtime 最终完成摘要：最终代码已按启动器同一 `target_game/play` 再构建；原 Playground `ragdoll` 与 `get_up` 公共功能 Gate 各自通过，runner/manual/validator 退出码 0，必需结果 10/10 与 11/11，43/67 pose samples，graph error/fault/temporal/watchdog 0。共用过滤器 mask 消融 red、恢复 green；未保留无贡献的 bundle 字段。完整性能预算依用户既定暂停范围延期，功能 Gate 不冒充完整性能通过。

- [x] **BF124.A11 — Ragdoll 找地查询保留原角色碰撞响应**：开始：2026-09-23 12:36:55 +08:00；预估结束：2026-09-23 12:51:55 +08:00；实际结束：2026-09-23 12:36:55 +08:00；偏差：-15m00s；结果：S1 UE 源/资产及真实 Avian red 反例、S2 最小过滤响应修复与双场景公共功能 Gate 均通过并各自提交（`6762679`、`50b2ab9`）。原 Pawn 胶囊忽略 Visibility；进入 Ragdoll 后角色层暂设 NONE，找地仍使用进入动作前的过滤响应，缺省映射 Default。消融查询 mask 后反例 red，恢复 green；原 Playground `ragdoll` 10/10、`get_up` 11/11，图错误、故障、时间异常、watchdog 均 0；完整性能预算延期。估时依据：仅核对已验收 S1/S2 与提交；拆分结果：A11 无剩余功能 Gate，A12 为下一项。

- [x] **BF124.A12-S1 — Ragdoll 世界胶囊尺寸源合同与失败反例**：开始：2026-09-23 12:38:35 +08:00；预估结束：2026-09-23 13:13:35 +08:00；实际结束：2026-09-23 12:41:24 +08:00；偏差：-32m11s；结果：fresh UE 两个调用点、Capsule inline 与目标两个消费者闭包，非均匀世界 scale focused test 编译成功且稳定 red。fresh 本地 UE `RagdollTraceGround` 在逐帧跟随和退出时均用 `GetScaledCapsuleRadius/HalfHeight`；UE Capsule inline 分别按组件世界水平 XY 的较小缩放与竖直 Z 缩放计算。目标 `ragdoll_capsule_dimensions` 只返回未缩放参数，供布娃娃找地和空中退出落地查询共用。估时依据：源两个调用点、Capsule inline、目标共享尺寸函数及两处消费者、A10 已归档的世界缩放夹具；拆分结果：S2 修复并运行真实入口，A13 不提前展开。

  source/static 完成摘要：源 inline 的半径 `base_radius * min(world_scale.X, world_scale.Y)`、半高 `base_half_height * world_scale.Z`，转目标坐标后为 `radius * min(scale.x, scale.z)` 与 `half * scale.y`；与 A10 `GetShapeScale` 的三轴绝对值最小值是不同 API，不能复用该尺度。目标已有 `scaled_mantle_capsule_dimensions` 实现同一轴映射，S2 可直接复用。两处 `ragdoll_capsule_dimensions` 调用目前都缺 scale 输入；standing/crouching、两组非均匀 scale 的 focused 反例当前在半高断言 red，暂 ignored 待 S2 开启。运行时仍需判断角色当前世界 scale 的读取时序并在原 Playground 验证。

- [x] **BF124.A12-S2 — 共享 Ragdoll 尺寸换算与基础 Gate**：开始：2026-09-23 12:42:40 +08:00；预估结束：2026-09-23 13:27:40 +08:00；实际结束：2026-09-23 12:50:10 +08:00；偏差：-37m30s；结果：共享源公式、两个世界 scale 消费者、实际物理球扫与原 Playground scale=1 基础 Gate 通过。复用已证实的 source 轴换算，把当前世界 scale 传给两个 Ragdoll 查询消费者；不新增另一套公式。估时依据：一处现有换算、两个消费者、两组反例及已存在 runner；拆分结果：可见非单位 scale 留独立 S3，A13 不提前展开。

  source/static 进展、runtime 前摘要：两个消费者均用现有 `TransformHelper` 计算当帧角色世界 scale，并交给共用尺寸入口；入口复用已验证的 `scaled_mantle_capsule_dimensions`，不另写第二份轴公式。standing/crouching 两组非均匀 scale 与缺失参数 fallback 均 green，真实 Avian 球扫核对落地中心高度 green，`ragdoll_link::tests::` 10/10；去掉共用换算后 focused test red，恢复 green。剩余边界是同入口构建与原 Playground `ragdoll`/`get_up` 功能 Gate，随后 S3 才做可见非单位 scale 分支。

  runtime 完成摘要：同入口 `target_game/play` 限流构建通过；原 Playground `ragdoll` 10/10、`get_up` 11/11 功能结果，runner/manual/validator 退出码 0，43/67 pose samples，graph error/fault/temporal/watchdog 0。物理球扫与非均匀 scale focused green，删去换算的消融 red。此轮验证 scale=1 兼容性，非单位世界 scale 在 S3 可见场景中验收；完整性能预算延期。

- [x] **BF124.A12-S3 — 原 Playground 可见缩放 Ragdoll Gate**：开始：2026-09-23 12:52:16 +08:00；预估结束：2026-09-23 13:47:16 +08:00；实际结束：2026-09-23 13:08:11 +08:00；偏差：-39m05s；结果：原 Playground 胶囊世界 scale=2 的布娃娃找地/跟随、截图、公共功能 Gate 与旧尺寸消融通过。在现有 `ragdoll` 场景的原地形、角色、相机内将 Capsule scale=2、Avatar 相对 scale=0.5 保持网格世界 scale=1，按站姿半高保持初始脚底；公共报告检验缩放后的找地目标、Actor 跟随及恢复。估时依据：现有 Ragdoll phase/coverage/runner、A10 已验证的可见 scale=2 设置、S2 物理反例；拆分结果：A12 可汇总关闭；完整网格 scale=2 的独立物理/读回缺口已证实但不归本项胶囊尺寸修复，A13 不提前展开。

  source/static 进展、runtime 前摘要：现有 `Ragdoll` 场景进入时把同一可见角色置 scale=2，沿既有 `locomotion_start` 抬高一份站姿半高保持脚底；地形、Avatar、相机和退出流程不换。公共覆盖新增 `ragdoll_scaled`：实际 grounded、角色 scale=2、找地目标中心接近源公式 `场景地面+2×站姿半高+最小离地距`，且 actor 跟随目标；源未缩放结果与预期相差约 0.9m。下一步使用同入口 `target_game/play` 构建并跑新 Gate，审计报告和可见 pose；若地形采样导致判据漂移，先回读实际 trace/地面证据。

  runtime 首轮失败摘要：同入口构建成功，`ragdoll_scaled` 与其余 10 项功能覆盖通过、无 watchdog 超时，但公共视觉 Gate 因 39 条 temporal/fault 失败；异常约在 1.37s 布娃娃接管时出现，多个骨段长度从 scale=2 长度跳到约一半，尚不能把功能通过当作验收。停止重跑，先从报告前后 pose、目标 ragdoll spawn/detach 与 fresh UE 网格 scale 生命周期确认究竟是实际缩放丢失还是一次性取样/fixture 时序问题；只修证实链路。

  失败复盘与范围校正：pose samples 在 1.20/1.31s 的 `pelvis→thigh` 为约 0.183m，1.37s 接管后及后续为约 0.091m，证实并非一次性取样。目标依赖 `spawn_ragdoll_avian` 只收 `Isometry3d`，`update_ragdolls_avian` 的 root `.to_isometry()` 丢 scale；UE `StartRagdolling` 则 `DetachFromComponent(KeepWorldTransform)` 保留网格世界 scale。这是完整 scale=2 网格的独立物理资产/读回链路缺口，超出本 A12 胶囊找地公式。为隔离已证实的本项查询，下一次同一原 Playground 把角色 Capsule 世界 scale=2、Avatar 相对 scale=0.5 维持网格世界 scale=1；UE mesh 与 Capsule 允许不同相对 scale，仍用真实角色/地形/相机和原 ragdoll 物理流程，公共报告继续严验胶囊尺寸。完整网格 scale 缺口保留证据，后续独立处理，不能以阈值放宽消除。

  隔离 fixture 的 runtime 后、消融前摘要：同入口 `target_game/play` 构建成功，原 Playground `ragdoll` 以角色 Capsule scale=2、Avatar 世界 scale=1 真正进入/恢复布娃娃，公共功能 11/11（新增 `ragdoll_scaled`），43 pose samples、两张可见场景截图，graph error/fault/temporal/watchdog 0；截图显示原地形/角色/相机，未使用隔离空白世界。首次完整 mesh scale=2 的失败作为独立依赖缺口保留，此轮不修改依赖、阈值或已完成动画图。下一步暂去共用尺寸换算跑同一场景，要求 `ragdoll_scaled` 失败，再恢复最终代码与 Gate。

  消融后、最终 runtime 前摘要：仅去掉共用 Ragdoll 尺寸换算的受控构建在同一 Playground `ragdoll` 场景退出码 91，公共报告仅 `ragdoll_scaled` 未覆盖，另有 18 条由错误胶囊跟随跳变引发的 temporal；graph error 与 watchdog 均 0。消融已恢复源码，下一步同一 `target_game/play` 重新限流构建，复跑带截图的最终功能 Gate，并核对报告 11/11 与图/时间/故障 0。

  runtime 最终完成摘要：恢复尺寸换算后同入口 `target_game/play` 构建成功，最终原 Playground `ragdoll` runner/manual/validator 退出码 0，11/11 必需项（含 `ragdoll_scaled`）、43 pose samples、两张场景截图，graph error/fault/temporal/watchdog 0；截图核对仍是原地形、角色与相机。该 fixture 专测源允许的 Capsule 与 mesh 不同世界 scale；完整 mesh scale=2 的依赖路径会导致骨段持续缩半，未以阈值放宽隐藏。完整性能预算延期。

- [x] **BF124.A12 — Ragdoll 找地与跟随使用世界胶囊尺寸**：开始：2026-09-23 13:09:22 +08:00；预估结束：2026-09-23 13:24:22 +08:00；实际结束：2026-09-23 13:09:22 +08:00；偏差：-15m00s；结果：S1 源公式/red、S2 两消费者修复与物理/常规 Gate、S3 原 Playground Capsule scale=2 可见 Gate 各自通过并单独提交（`1360f86`、`f70c204`、`e99feed`）。UE 半径按世界水平两轴较小 scale、半高按竖直 scale；目标复用现有源换算，逐帧和空中退出查询均读取当帧世界 scale。非均匀尺寸与真实物理球扫 green，最终 `ragdoll` 11/11、43 pose samples、两张截图、图错误/故障/时间异常/watchdog 0；旧尺寸消融只漏 `ragdoll_scaled` 且退出码 91。完整 mesh scale=2 的依赖 ragdoll 生成/读回 scale 丢失是单独缺口，当前以 Capsule scale=2、Avatar 世界 scale=1 的源合法配置隔离验证，未修改依赖或放宽 Gate；完整性能预算延期。估时依据：仅核对三个已验收子项与提交；拆分结果：A12 胶囊尺寸无剩余功能 Gate，A13 为下一项。

- [x] **BF124.A13-S1 — 脚骨缓存重建的 source/target 生命周期与失败反例**：开始：2026-09-23 13:12:10 +08:00；预估结束：2026-09-23 13:47:10 +08:00；实际结束：2026-09-23 13:17:50 +08:00；偏差：-29m20s；结果：fresh UE C++/SKM 源合同、目标缓存生产与两个消费者闭包、保留旧 Avatar 的 focused red 通过。fresh UE `UAlsAnimationInstance::NativeInitializeAnimation` 从当前 Mesh 骨架找脚链，`RefreshFeetOnGameThread` 每次从当前 Mesh 按名字读取脚目标 Socket Transform，并在 pending/无效时重验；目标 `AlsFootBones` 只在缺组件时由角色全子树收集，已有旧骨实体即使存活且有限值也不会重建，脚锁/footprint 消费者可继续引用旧 Avatar。估时依据：UE 初始化和逐帧两段、目标一处缓存生产和脚锁/footprint 两个消费者、既有测试夹具；拆分结果：S2 修复与 S3 公共视觉 Gate，A14 不提前展开。

  source/static 完成摘要：原生 `SKM_Als` reader watchdog `success=true`，当前 SkeletalMesh 为 79 个骨、11 个虚骨，脚链 `thigh/calf/foot_l/r`、`ik_foot_l/r` 与 `VB foot_l/r` 均存在。UE 初始化从当前 Mesh ReferenceSkeleton 查脚链，每帧 `RefreshFeetOnGameThread` 从当前 Mesh 读左右 Socket Transform，pending 更新时重置有效性。目标 core `foot_bone_map_system` 仅匹配 `Without<AlsFootBones>`，`foot_bones_are_valid` 只核有限的全局 Transform，不能识别仍活着但已非当前 Avatar 的旧骨；`refresh_foot_lock_inputs` 与 `spawn_footprints` 继续用旧 Entity。最小 fixture 保留旧 Avatar/骨实体、切 `AlsCharacterAssembly.avatar` 到新 Avatar，当前映射 left 仍为旧 Entity（17v0，预期 24v0）；focused test 编译且稳定 red，暂 ignored 待 S2 开启。下一项须选定当前 Avatar/动画播放器的实体闭包并处理缓存更换时脚锁生命周期。

- [x] **BF124.A13-S2 — 当前 Avatar 脚骨映射与脚锁重置**：开始：2026-09-23 13:20:12 +08:00；预估结束：2026-09-23 14:10:12 +08:00；实际结束：2026-09-23 13:28:52 +08:00；偏差：-41m20s；结果：当前 Avatar/播放器闭包映射、替换/暂缺清理、旧脚锁重置与局部回归通过。以 `AlsCharacterAssembly.avatar` 和当前 `AnimatedSceneInstance.player_entity()` 限定脚骨发现闭包；映射变化或当前骨暂缺时更新/移除缓存并重置脚锁生命周期，使脚印与 IK 消费者不再读取仍存活的旧 Avatar。估时依据：现有 core 建图函数与 project runtime 注册点、两处脚锁清理代码、一个真实 scene instance 选择及已建 red fixture；拆分结果：S3 原 Playground 公共视觉 Gate，A14 不提前展开。

  source/static 完成摘要：项目运行时改用当前 `AlsCharacterAssembly.avatar`，若有 `AnimatedSceneInstance` 则只遍历其当前 player 子树；原 core 无主映射仍保留给其他入口，项目不再重复注册它。`AlsFootBones` 值变化时同步更新/移除缓存并清空既有 input、lock、dynamic、lifecycle、teleport 状态；没有新增所有者状态。S1 red 变 green，并覆盖旧 Avatar 留存、同 Avatar 换骨实体仍活、旧 Avatar 先移除后装新，`foot_ik_apply::tests::` 8/8、`footprint::tests::` 8/8。去掉 lock reset 后 focused 明确 red，恢复 green；显式清空 Rig input 的附加分支对信号无贡献已删。剩余边界是原 Playground 实际动画/脚印/替换路径的公共视觉 Gate。

- [x] **BF124.A13-S3 — 原 Playground Avatar 替换与脚骨视觉 Gate**：开始：2026-09-23 13:32:58 +08:00；预估结束：2026-09-23 14:27:58 +08:00；实际结束：2026-09-23 14:24:06 +08:00；偏差：-3m52s；结果：真实 scene/player 替换、当前脚骨映射/脚锁恢复、公共视觉功能 Gate 与旧缓存消融通过。在既有 `foot_ik` 场景原地形/角色/相机完成运动分支后，预热同一 AnimatedScene 的第二 Avatar 骨架，切当前 Assembly/相机绑定，保留旧脚实体；公共结果严格要求新 player 子树映射与脚锁恢复。估时依据：原 FootIk phase 末尾接线、现有 Scene/Assembly/Gate、S2 focused fixture；拆分结果：A13 可汇总关闭，A14 不提前展开。

  source/static 进展、runtime 前摘要：沿既有 `FootIk` 场景 Roll 结束窗口加载相同 AnimatedScene 的第二个 Avatar，待 player 输出 Pose 和脚骨子树就绪后才切当前 Assembly/相机绑定；旧 Avatar 隐藏但旧脚实体仍存活。公共 `foot_bones_replaced` 需新 map 精确等于当前 player 子树、旧左脚仍活且新脚锁恢复；等待超 10s 自动结束并在报告给出进展阶段、等待时间与当前条件。focused watchdog 边界 3/3 通过；S1/S2 映射与消费者局部 Gate 已完成。下一步同入口 `target_game/play` 限流构建，按 skill 先 `startup` 再 `foot_ik`/`footprint` 公共功能 Gate，审计非零进程也必须读取报告。

  runtime 首轮失败与 source 链复盘：同入口 `startup` 功能通过；`foot_ik` 20/21 必需项通过，仅 `foot_bones_replaced` 未覆盖，graph error/fault/temporal 0，机器可读 watchdog 在等待 10.011s 时记录 `last_progress=await_scene`、`selected=false` 并使进程退出 91。目标 `generated_ab_als_provider` 输入发布 Query 限定 `With<AlsCharacterAvatar>`；预热新 Avatar 时暂删该标记使新 player 无法获得输入 Pose，故不是脚骨重建本身的失败。下一轮保留预热 Avatar 标记与隐藏可见性，Assembly 仍指旧 Avatar，等新 Pose/骨闭包就绪后再一次性切换；不放宽 Gate 或延长超时。

  第二轮失败与诊断改向：保留预热 Avatar 标记后的原 Playground `foot_ik` 仍为 20/21，仅 `foot_bones_replaced` 未覆盖，graph error/fault/temporal 0，watchdog 10.003s `await_scene`、未切换。此前标签把实例、播放器、Pose、脚骨四个条件合并，不能证明第一轮标记推断为根因。先让同一固定上限报告细分最后达成的实例/player/Pose/bone/map/lock 进展，再依据实际失败条件检查目标加载或时序；不重复同一无信息 runtime 调用。

  第三轮诊断与估时核对：细分 watchdog 报告在 10.005s 明确停于 `await_pose`；新 `AnimatedSceneInstance`/player 已存在，旧运动检查 20/21、graph error/fault/temporal 0，但新 player 的 Pose 未被当前报告独立审计。记录时核对当前 UTC 05:57:32 对应香港时间 13:57:32，距本项 13:32:58 开始已过 24m34s，原预估 14:27:58 仍有效；先前多加一小时的超时/重估记载已纠正。source C++/SKM、目标脚骨映射与 focused 三顺序闭包均已通过，不能因新 player 缺 Pose 放宽公共视觉 Gate。纠正方向：静态核对新 player `AnimationSource`、输出与错误状态及 AnimatedScene 图入口，按证实边界改 fixture/代码并运行同一 Gate；不开始 A14。

  第二轮 source/static 进展、runtime 前摘要：fresh 当前 AnimatedScene 指向已安装 root ragdoll wrapper 图；依赖 `animation_player` 遍历全部播放器，并不按 `Visibility` 跳过，故“隐藏导致未求值”没有 source/target 依据。细分报告已证明新 instance 与 player 存在、只缺默认输出 Pose；现把该 player 的 `AnimationSource`、真实 graph error 与输出数写进同一固定 watchdog 的机器可读当前条件。下一次为诊断运行，只在报告证实具体源/目标边界后修复；保持原 10s 上限与所有视觉/功能断言。

  新 player 错误定位与改正方向：诊断 watchdog 为 `await_pose`，`source=graph`、`outputs=0`、原始错误 `GlobalInputDataMissing("control_rig_input.b_use_hand_ik_bones")`。目标 `control_rig_apply::source_update` 只向 `AlsCharacterAssembly.avatar` 的 player 写该字段；预热 Avatar 在切换前永远不能满足此条件，故此前 Pose 前置要求本身与输入生命周期相矛盾。当前项目公开扩展点允许先等 `AnimatedSceneInstance`、player 和脚骨实体完整，再切 Assembly/相机绑定；同一 PostUpdate `source_update` 给新 player 输入后求值。删除错误的 Pose 前置和一次性 diagnostic 状态，保留完整视觉 Gate 与 10s watchdog；下一次按此闭包复验。

  runtime 前 source/static 修正：进一步核对依赖 `AnimatedScene` 构造先 `with_graph` 建 arena、后按 `AnimatedSceneHandle.override_source` 设置动画源。预热场景现显式 `override_source=None`（`AnimationSource::None`），仍加载同一真实 skeletal scene/骨实体但不会在缺当前角色 Control Rig 输入时求值；骨闭包就绪的切换帧复用该 arena 将 player 设回原 `scene.animation_graph`，同时切 Assembly，PostUpdate 输入发布后才求值。此配置消除了已证实的预热图错误，不修改依赖或图。剩余是同入口运行、报告与旧规则消融。

  首次替换成功的 runtime 后摘要：同入口原 Playground `foot_ik` 公共功能 21/21（含 `foot_bones_replaced`），757 pose samples，graph error/fault/temporal/watchdog 0，runner/manual/validator 退出码 0。原旧脚实体存活，新 player 骨映射与脚锁恢复均达成；固定 watchdog 未触发。Bevy 可见性传播另输出 B0004：新 Avatar 有 `InheritedVisibility`，父角色没有；该警告源于 fixture 新增 `Visibility::Hidden/Visible`，不是脚骨算法。下一轮在场景初始化该父角色的默认可见性（Bevy 原生 `Visibility::Inherited`）后再复验，随后跑 `footprint` 与旧映射消融；不把含警告的首轮当最终视觉证据。

  可见性修正后与 `footprint` 前摘要：场景父角色显式采用 Bevy 原生 `Visibility::Inherited`，同入口 `foot_ik` 最终候选 runner/manual/validator 退出码 0、21/21（含旧左脚存活/新 map/脚锁恢复）、753 pose samples，graph error/fault/temporal/watchdog 0；不再出现 B0004。`startup` 功能 Gate 先前通过。下一步用同一最终二进制运行原 Playground `footprint` 公共 Gate，确认脚印消费者没有因映射刷新回归；之后做旧一次性缓存规则消融并最终恢复/复验。

  `footprint` 后、消融前摘要：同一最终候选 `target_game/play` 的原 Playground `footprint` 功能 21/21、1,271 pose samples、render/decal Gate 通过（36 次 GPU readback 请求、576 样本），runner/manual/validator 退出码 0，graph error/fault/temporal/watchdog 0。功能已同时覆盖脚 IK 当前播放器与脚印消费者；下一步仅暂把项目注册的当前 Avatar 刷新系统换成原 `Without<AlsFootBones>` 一次性系统，在同一 `foot_ik` 场景要求 `foot_bones_replaced` 失败，然后恢复最终代码/二进制并复验。

  旧缓存消融后、最终 runtime 前摘要：仅将项目脚骨生产系统换回原 `Without<AlsFootBones>` 一次性规则的同场运行退出码 91，20/21 项通过，唯一 `foot_bones_replaced` 未覆盖；watchdog 固定 10.010s、`last_progress=await_current_foot_map`、`selected=true`，graph error/fault/temporal 0。说明新 scene/player/骨闭包及切换都完成，唯旧 map 留存；源码已恢复当前 Avatar 刷新系统，下一步按启动器同一 `target_game/play` 重建，最终复验 `foot_ik`，确认非消融二进制与先前通过的 `footprint` 报告对应。

  runtime 最终完成摘要：恢复项目当前 Avatar 刷新系统后，同入口 `target_game/play` 限流重建并复验原 Playground `foot_ik`：runner/manual/validator 退出码 0、21/21（含 `foot_bones_replaced`）、734 pose samples，graph error/fault/temporal/watchdog 0；先前同闭包 `startup` 功能和 `footprint` 21/21、1,271 pose samples、36/576 GPU readback 均通过。预热播放器使用 `AnimationSource::None`，待骨实体就绪及 Assembly 同帧切换再启原图，因此没有预热缺输入图错误；旧 player/脚实体仍存活但不再选中。旧缓存消融仅漏新必需项并退出 91；无 B0004。完整性能预算依用户既定暂停范围延期。

- [x] **BF124.A13 — Avatar/骨骼替换后重建脚骨缓存**：开始：2026-09-23 14:25:52 +08:00；预估结束：2026-09-23 14:40:52 +08:00；实际结束：2026-09-23 14:25:52 +08:00；偏差：-15m00s；结果：S1 fresh UE/SKM 与旧 Avatar red、S2 当前播放器映射及脚锁复位、S3 原 Playground 活 Avatar 替换公共 Gate 分别通过并单独提交（`2f13e48`、`c5e477c`、`6d5e6fd`）。源每帧从当前 Mesh 读取脚 Socket；目标映射现在从当前 Assembly/AnimatedScene player 子树构建，变化或暂缺时清旧 map/脚锁，脚印共用新实体。focused 覆盖旧实体留存、同 Avatar 换骨、先拆后装；最终 `foot_ik` 21/21、734 pose samples、`footprint` 21/21、1,271 pose samples、GPU 36/576，图错误/故障/时间异常/watchdog 0；旧一次性映射消融 20/21 且固定进展超时退出 91。完整性能预算延期。估时依据：核对三个已验收子项与提交；拆分结果：A13 无剩余功能 Gate，A14 为下一项。

- [x] **BF124.A14-S1 — Turn 栈跨播放器重建的 source/target 合同与反例**：开始：2026-09-23 14:28:00 +08:00；预估结束：2026-09-23 15:08:00 +08:00；实际结束：2026-09-23 14:34:47 +08:00；偏差：-33m13s；结果：fresh UE C++/AnimBP 生命周期和 target 双消费者闭包、同 Avatar 换 player red 反例通过。fresh 本地 `UAlsAnimationInstance`/`AB_Als` 核对 Turn montage/队列的 AnimInstance 所有权与初始化/停止语义；目标 `AlsTurnMontageStack` 和 Drive 在 Avatar 上，`turn_pin_driver` 与桥接每帧通过可变 `AnimatedSceneInstance.player_entity()` 向当前播放器写输入，换播放器时旧栈可能继续给新播放器采样。估时依据：UE 初始化/Turn play/stop 三处和一个 AB_Als 图、目标两系统与既有 `WorldInstanceReady` 测试模式；拆分结果：S2 修复、S3 原 Playground 公共视觉 Gate，A15 不提前展开。

  source/static 完成摘要：原生 `AB_Als` 与 `AB_Als_Grounded` reader watchdog 均 `success=true`，后者含 `StopTransitionAndTurnInPlaceAnimations` 源函数图；UE `FAlsTurnInPlaceState` 为 `UAlsAnimationInstance` 的 Transient 成员，新实例初始队列为空，`PlayQueuedTurnInPlaceAnimation` 对 `this` 播放动态 Montage，停止也仅对 `this` 的 Transition/Standing/Crouching Slot 执行。目标 `AlsTurnMontageStack` 与 `AlsTurnDrive` 属于 Avatar，`turn_pin_driver` 和 `bridge_turn_slot_samples` 从当下 `AnimatedSceneInstance.player_entity()` 向 player 写 Turn 输入；没有旧 player 所有权核对。focused WorldInstanceReady fixture 保留旧 player Entity，改同一 Avatar 的当前 player 后旧 stack 仍有 1 项（预期 0），编译且稳定 red，暂 ignored 待 S2 开启。下一项需让栈/Drive 在 player 身份变化时同步清空，同时防止桥接一帧转送旧 Pose。

- [x] **BF124.A14-S2 — Turn 栈绑定播放器并同步终止旧 Slot**：开始：2026-09-23 14:37:32 +08:00；预估结束：2026-09-23 15:22:32 +08:00；实际结束：2026-09-23 14:49:19 +08:00；偏差：-33m13s；结果：当前 player 身份绑定、Update/PostUpdate 双入口旧栈与旧输入清理、focused 双顺序及 17 项 Turn 回归通过。让 Avatar Turn 栈记住当前 player，变化时在 Update 与 PostUpdate 桥接前归零栈/Drive；旧 player 仍活时同步清空 Turn pins、Standing/Crouching Slot 权重与 `SourceAnimationInputs` 上下文。估时依据：一处栈身份、一条共用 reconcile、两个现有消费者与 S1 WorldInstanceReady 夹具；拆分结果：S3 原 Playground 公共视觉 Gate，A15 不提前展开。

  source/static 完成摘要：一处 `owner_player: Option<Entity>` 随 Avatar 栈绑定当前 `AnimatedSceneInstance.player_entity()`；玩家身份变化时共享 `reconcile_turn_player` 重置栈/Drive 并返回旧 player，后者的 Turn pins、Slot 权重及 slot context 同步清零。Update 驱动与 PostUpdate 桥接均在推进/采样旧栈前调用，避免播放器在同帧更新时旧 Pose 转入新 player。S1 red 变 green，夹具验证旧 player 仍活、两种系统先看到替换的顺序、旧输入/新输入、Slot runtime 空；`turn_layer::tests::` 17/17。去掉桥接身份核对导致旧栈未清 red，去掉 Update 核对使旧播放器权重残留 red，恢复后均 green。剩余原 Playground 实际 Turn/播放器替换视觉 Gate。

- [x] **BF124.A14-S3 — 原 Playground Turn 姿态/Slot 公共 Gate**：开始：2026-09-23 14:52:29 +08:00；预估结束：2026-09-23 15:37:29 +08:00；实际结束：2026-09-23 14:57:32 +08:00；偏差：-39m57s；结果：同入口 `startup`、Standing/Crouching Turn 公共功能 Gate 通过。S1/S2 真实 `WorldInstanceReady` focused 夹具已覆盖同 Avatar player 替换和双调度顺序；本项在启动器原地形/角色/相机用 `standing_turn` 与 `crouching_actions` 核对实际图状态、Turn 片段/Slot、root Pose、时间守卫与 runner 退出码。估时依据：S2 17 项 Turn 局部通过、既有公共场景/runner、同入口 `target_game/play` 预构建；拆分结果：A14 可汇总关闭，A15 不提前展开。

  source/static 完成、runtime 前摘要：fresh `AB_Als`、`AB_Als_Grounded` 原生 reader 与 C++ AnimInstance/Turn Slot 闭包已记录；目标没有改动 RON/FSM/曲线或源资产，仅将 Avatar 栈/Drive 绑定当前播放器并清旧播放器输入。focused 同一 Avatar 三个 player 依次替换、旧实体保留、PostUpdate 先见与 Update 先见路径 green；局部 Turn 17/17。桥接与 Update 各自身份核对消融均 red，最终源码/工作树恢复。剩余真实入口 `startup`、`standing_turn`、`crouching_actions` 功能 Gate；完整性能预算延期。

  `startup` 后、Turn 动作前摘要：同入口 `target_game/play` 限流构建成功，原 Playground 无驱动 `startup` 公共功能 Gate 通过；未见 graph error/fault/temporal/watchdog，完整性能预算延期。下一步同一二进制依次运行源 Standing Turn 和 Crouching Actions 场景，检查实际 Slot/片段、root Pose 与时序报告。

  `standing_turn` 后、蹲姿 Turn 前摘要：同入口原 Playground `standing_turn` 功能 11/11、2,060 pose samples，runner/manual/validator 退出码 0，graph error/fault/temporal/watchdog 0；实际七次 Turn Slot 惯性结束请求完成。仅完整性能预算中的 max frame/eval 指标延期，不冒充全量性能通过。下一步同一二进制运行 `crouching_actions`，核对 Standing/Crouching 另一个 Slot 分支及动作终止。

  runtime 最终完成摘要：同一 `target_game/play` 构建后的原 Playground `startup` 功能通过、`standing_turn` 11/11 与 2,060 pose samples、`crouching_actions` 11/11 与 810 pose samples；三者 runner/manual/validator 退出码 0，graph error/fault/temporal/watchdog 0。真实同 Avatar 换 player 的 focused Gate 双调度先后均 green，旧播放器 Slot/上下文归零，新播放器无旧 Pose；桥接和 Update 两处身份核对消融各自 red。完整性能预算延期，功能验收不冒充性能通过。

- [x] **BF124.A14 — Turn 播放栈不跨动画播放器替换残留**：开始：2026-09-23 14:58:33 +08:00；预估结束：2026-09-23 15:13:33 +08:00；实际结束：2026-09-23 14:58:33 +08:00；偏差：-15m00s；结果：S1 fresh UE AnimInstance/AB_Als 函数图与真实换 player red、S2 栈身份/旧 Slot 清理及双顺序 focused Gate、S3 原 Playground 公共视觉 Gate 各自通过并提交（`b5c7e2a`、`ce3f424`、`9c65ea3`）。源 Turn 动态 Montage/队列属于单个 AnimInstance；目标 Avatar 栈现绑定当前 player，换 player 时同步归零栈/Drive、旧 player 的 pins/Slot 权重及上下文。局部 Turn 17/17、同 Avatar 三 player 依次替换与两处消融 red；原 Playground `startup`、`standing_turn` 11/11、`crouching_actions` 11/11，图错误/故障/时间异常/watchdog 0。完整性能预算延期。估时依据：只核对 S1–S3 已验收结果与提交；拆分结果：A14 无剩余功能 Gate，A15 为下一项。

- [x] **BF124.A15-S1 — 晚到贴花排除标记的 source/target 合同与反例**：开始：2026-09-23 15:00:29 +08:00；预估结束：2026-09-23 15:40:29 +08:00；实际结束：2026-09-23 15:13:10 +08:00；偏差：-27m19s；结果：fresh UE 角色 Mesh 排除源事实、目标三系统闭包与晚到标记 focused red。fresh 本地角色 Mesh/贴花接收源事实与 FootprintLibrary 合同核对后，追踪 target `configure_footprint_owners`、`avatar_material_bridge`、`prepare_receivers` 顺序：根 `NoDecalReceiver` 晚于 `AvatarMaterialApplied` 时桥接短路，子网格仍可保持 receiver extension。估时依据：本地一个角色源资产与渲染 API、目标三个已有系统、一处现有 fixture；拆分结果：S2 最小修复/材质局部 Gate，S3 原 Playground GPU 可见 Gate，A16 不提前展开。

  source/static 完成摘要：原生 `B_Als_Character` reader watchdog `success=true`，`CharacterMesh0.bReceivesDecals=False`（其余 Arrow/Camera/Capsule 为 True），与目标 `footprint.runtime.json.character_receives_decals=false` 一致。项目 `configure_footprint_owners` 仅在 FootprintLibrary 可用时给 Avatar 根加 `NoDecalReceiver`；`avatar_material_bridge` 一见 `AvatarMaterialApplied` 就跳过，`prepare_receivers` 只按子网格自身 marker 撤掉 receiver extension。现有 World fixture 首次完成材质桥，之后在根加 marker，旧路径子网格仍无 marker，focused test 编译成功且稳定 red；嵌套独立 Avatar 必须继续保留自己的接收设置，暂 ignored 待 S2 开启。第一次独立 `als-ext` 测试编译耗时 9m15s，后续复用缓存；无图/原始材质资产改动。

- [x] **BF124.A15-S2 — 已处理 Avatar 的晚到排除传播修复**：开始：2026-09-23 15:14:44 +08:00；预估结束：2026-09-23 15:59:44 +08:00；实际结束：2026-09-23 15:20:45 +08:00；偏差：-38m59s；结果：晚到根排除传播、nested Avatar 与材质 Slot 保持、原 receiver 撤回路径及消融通过。在现有 `avatar_material_bridge` 内仅对 `AvatarMaterialApplied` 后新加的根 `NoDecalReceiver` 重走子网格标记传播，继续由 `prepare_receivers` 恢复原 StandardMaterial；不重算/覆盖已验收的材质 Slot。估时依据：一处早退条件、现有 World fixture、已缓存的 crate 构建与原 GPU runner；拆分结果：S3 原 Playground `footprint` GPU/可见 Gate，A16 不提前展开。

  source/static 完成摘要：在已处理 Avatar 上仅当根 `NoDecalReceiver` 是新加组件时遍历原材质桥能识别的 Mesh3d 子网格，补同一 marker 后早退，不重写已处理材质 Slot；保留 `avatars.contains(e)` 的嵌套 Avatar 所有权边界。S1 red 转 green，`overlay_materials::tests::` 3/3，现有 `projected_decal` focused 反例证明子网格 marker 使 receiver extension 回退至原 StandardMaterial。移除新增的 late-marker 早退例外后同一 fixture 再 red，恢复 green；没有新系统、状态、材质资产或 shader。剩余是真实 `footprint` 场景的 `footprint_character_receiver`/GPU readback 公共 Gate。

- [x] **BF124.A15-S3 — 原 Playground 角色贴花排除与 GPU Gate**：开始：2026-09-23 15:22:06 +08:00；预估结束：2026-09-23 16:07:06 +08:00；实际结束：2026-09-23 15:26:53 +08:00；偏差：-40m13s；结果：原 Playground 无驱动/脚印公共功能与 receiver GPU Gate 均通过。以同入口 `target_game/play` 限流预构建，先跑 `startup`，再跑原地形/角色/相机 `footprint`，核对角色子网格排除、原 StandardMaterial、receiver pipeline/GPU readback、root Pose 与时间守卫。估时依据：既有 21 项 footprint 公共结果和 36 次 GPU 请求夹具、S2 材质 focused 3/3、无图/源资产改动；拆分结果：A15 可汇总关闭，A16 不提前展开。

  source/static 完成、runtime 前摘要：fresh UE `CharacterMesh0.bReceivesDecals=False` 与目标 runtime library 一致；晚到 marker 的已处理子网格 focused red→green，嵌套 Avatar 与材质值 3/3，receiver extension 回退原 StandardMaterial 的已有 focused Gate 通过；撤销早退例外消融 red，恢复 green。现仅待同入口 `startup` 与 `footprint` 公共功能/GPU Gate，完整性能预算延期。

  `startup` 后、footprint 前摘要：同入口 `target_game/play` 限流构建通过，原 Playground 无驱动 `startup` 公共功能 Gate 通过，图错误/故障/时间异常/watchdog 0；完整性能预算延期。下一步运行同一二进制的 `footprint`，核对至少 15 个角色子网格排除、receiver pipeline 与实际 GPU readback。

  runtime 最终完成摘要：同一最终二进制原 Playground `footprint` runner/manual/validator 退出码 0、21/21（含 `footprint_character_receiver`）、1,690 pose samples，角色 15 个 StandardMaterial 子网格带排除 marker，16 个总 marker，receiver/render Gate 通过；36 次 GPU readback 请求、576 个样本完成。graph error/fault/temporal/watchdog 0，`startup` 功能亦通过。完整性能预算延期；无源资产、shader 或图改动。

- [x] **BF124.A15 — 晚加载贴花排除标记传播到已处理子网格**：开始：2026-09-23 15:27:55 +08:00；预估结束：2026-09-23 15:42:55 +08:00；实际结束：2026-09-23 15:27:55 +08:00；偏差：-15m00s；结果：S1 fresh 角色 Mesh/目标生命周期 red、S2 最小传播修复及 receiver 回退、S3 原 Playground GPU Gate 各自通过并单独提交（`4768935`、`e2edcea`、`0535b5d`）。源 `CharacterMesh0.bReceivesDecals=False`；目标晚加根排除标记时仅补已处理子网格 marker，嵌套 Avatar 和原材质 Slot 不变，现有 receiver 系统回退原 StandardMaterial。局部材质 3/3、撤掉例外消融 red；真实 `footprint` 21/21、角色 15 个子网格排除、GPU 36/576、图错误/故障/时间异常/watchdog 0。完整性能预算延期。估时依据：仅核对三个已验收子项与提交；拆分结果：A15 无剩余功能 Gate，A16 为下一项。
