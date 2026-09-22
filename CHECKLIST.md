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
