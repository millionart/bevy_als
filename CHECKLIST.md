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
