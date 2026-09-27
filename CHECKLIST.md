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

## BF131 — c609ed5 审阅 v17 的 37 项反馈

来源：内置浏览器《复核移植差异》的 v17 `current_open_findings.md`，2026-09-27 读取。审阅目标为 `c609ed596d420f93e2cc383904733928a64114a6`；本批修复基线为 `e6782b8`。对标 ALS `b754d6f0f2bb03741d301f8fb88077ebfe561e17`、UE `396c9f059903aed5fec78ecd3d437a40c6415368`。保留原编号，共 A13、A40、A56–A90；A13/A40 仅处理残留边界。浏览器正在进行的后续审阅不属于这 37 项。

执行：按下列模块顺序逐项复核当前路径及本地 source，先留下失败反例，再修复、消融和回归；每项自身 Gate 通过后勾选并单独提交。网页的静态结论不代替本地复现。涉及转换规则时必须重生成全部已安装 AnimBP 及递归依赖，并核对完整产物差异；涉及动画行为时使用公共 Playground 视觉 Gate。未通过的项目保持未勾选。

估时与拆分：参考归档 BF124.A13/A40/A39 的 source、focused 修复、真实入口三阶段记录，逐项分为 S1 源合同与失败反例、S2 最小修复与消融、S3 真实入口验收，每个执行窗口不超过 60 分钟；复杂物理或转换闭包在 S1 后继续细分。各项开始时根据 fresh source 闭包填写实际开始和预估结束时点，排队条目不冒记为已开始。

### 当前 Avatar 与材质生命周期

- [x] **BF131.A13 — 当前脚骨映射已更新，但保留旧 Avatar 时脚目标仍可能按旧坐标系转换**。Gate：新旧 Avatar 使用不同平移/旋转/缩放，目标转换只依赖当前 Assembly/player；旧实体保留与两种拆装顺序均通过，原脚锁行为不回退。
- [x] **BF131.A40 — 透明标准材质替换没有显式退出标记时，旧贴花代理残留并可抢占渲染材质**。Gate：Opaque→透明材质同帧交接不留旧代理，当前材质保留；显式退出和正常贴花接收控制通过。

### 动画图、曲线、同步和根运动

- [ ] **BF131.A56 — Scoped cached pose lookup ignores linked-instance ownership**。Gate：同图多个 linked instance 的缓存只在所属实例内解析，正常同实例复用与缺失诊断通过。
- [ ] **BF131.A57 — Graph footstep dispatcher mishandles reverse playback**。Gate：正播/倒播、跨循环和多圈步进按源事件区间分派，不漏发或重复。
- [ ] **BF131.A58 — Point NotifyTriggerChance is imported but not consumed**。Gate：点通知概率 0/1 及中间值按源分派规则生效，保留其他过滤条件。
- [ ] **BF131.A59 — FSM getter requires one arena-wide static GUID match instead of owning linked-instance scope**。Gate：同 GUID 的多 linked instance 从所属实例读取状态，无关实例不造成歧义。
- [ ] **BF131.A60 — CurveFloat importer rejects valid single-key constant curves**。Gate：单关键帧常量曲线可导入并在区间内外正确采样，空/无效输入仍按源合同处理。
- [ ] **BF131.A61 — LinkedGraph reintroduces inactive or unweighted input asset samples after child evaluation**。Gate：子图未消费及零权重输入不重新出现；实际输出样本和权重正确。
- [ ] **BF131.A62 — AnimSequence asset RateScale is discarded before generated playback**。Gate：资产 RateScale 与节点倍率按源规则组合，正/负/零倍率及循环控制通过。
- [ ] **BF131.A63 — TimeFactor BlendProfile is accepted then evaluated as WeightFactor**。Gate：TimeFactor/WeightFactor 的逐骨混合分别符合源公式，零/单位/非单位因子可区分。
- [ ] **BF131.A64 — Animation curves lose case-insensitive FName identity and name-based basis conversion**。Gate：大小写变体共享曲线身份，名称驱动的坐标转换一致且只执行一次。
- [ ] **BF131.A65 — BlendSpace Player accepts then discards per-bone sample weight smoothing overrides**。Gate：逐骨平滑覆盖由 source 进入生成与实际混合；未覆盖骨保持默认，连续时间及重建通过。
- [ ] **BF131.A66 — CurvesBlend omits fractional update weight before its child FSM runs**。Gate：子 FSM 更新时已收到实际分数权重，0/中间/1 权重及嵌套更新通过。
- [ ] **BF131.A67 — TransitionLeader/TransitionFollower are not excluded during fade-in**。Gate：淡入阶段按角色和源有效权重选 leader，退出淡入后恢复候选资格。
- [ ] **BF131.A68 — Sync marker mapping does not filter to common marker names**。Gate：先求组内共同 marker 集，再定位/映射，非共同 marker 不改变有效相位。
- [ ] **BF131.A69 — Modify Curve accepts WeightedMovingAverage but evaluates it as stateless Blend**。Gate：逐实例历史参与加权移动平均，初始化/更新/重建及其他模式控制通过。
- [ ] **BF131.A70 — Repeated marker-name pairs collapse later timeline occurrences into the first pair**。Gate：同名重复 marker 对保留时间位置，跨周期与倒播不跳到第一对。
- [ ] **BF131.A71 — Custom 状态迁移按总时长结束，遗漏 FAlphaBlend 提前达到终值的完成语义**。Gate：源曲线提前达到终值时迁移完成，未达到终值及正常总时长控制通过。
- [ ] **BF131.A72 — 惯性过渡的显式 BlendProfile 在生成、请求和逐骨消费链中丢失**。Gate：显式 profile 贯穿生成、惯性请求与逐骨持续时间，默认 profile 和无覆盖控制通过。
- [ ] **BF131.A73 — 惯性化 root 历史没有按组件／Actor 附着空间变化重表达**。Gate：组件/Actor 附着空间改变后历史按源坐标系重表达，世界连续姿态和不变空间控制通过。
- [ ] **BF131.A74 — 关闭跨被跳过缓存分支的惯性请求转发仍向全部消费者复制请求**。Gate：关闭转发时跳过分支不接收请求，开启转发及实际求值消费者保持源语义。
- [ ] **BF131.A75 — SequencePlayer 初始化未保留默认倒播起点语义，非循环负倍率停在首帧**。Gate：默认负倍率从源规定终点初始化；显式起点、循环/非循环及重初始化通过。
- [ ] **BF131.A76 — StateResult 完全混入／完全混出回调被解析和生成，但 FSM 从不触发**。Gate：实际状态权重到达端点触发对应回调，连续帧不重复，打断/重入符合源生命周期。
- [ ] **BF131.A77 — 根运动平移按固定轨迹坐标相减，遗漏前一 root 旋转的相对空间转换**。Gate：带旋转轨迹按前一 root 局部空间提取位移，正倒播和循环连续，直线控制通过。
- [ ] **BF131.A78 — GroundPlane 将 yaw 同时发布为根运动并保留在输出 root 姿态中**。Gate：消费 yaw 后输出 root 去除对应旋转，保留源规定的高度/其他轴，角色不重复旋转。

### PhysicsAsset、关节与布娃娃回读

- [ ] **BF131.A79 — 线性化关节的两个独立 Swing 限位被当成椭圆锥／圆锥，界内斜向姿态受到错误修正**。Gate：源独立轴界内斜向姿态保持，越界轴单独修正，限位模式和坐标基控制通过。
- [ ] **BF131.A80 — 自由线性轴的位置／速度驱动通过最终校验，却未保留到运行时并可使纯驱动关节禁用**。Gate：纯线性位置/速度驱动保留并启用，目标/强度/阻尼/限力按源消费，自由无驱动控制通过。
- [ ] **BF131.A81 — 完全自由关节只按 Twist 位置驱动判定启用，导致有效 Swing／角速度驱动被 JointDisabled 屏蔽**。Gate：逐驱动启用条件与实际活动轴一致，纯 Swing/速度驱动可工作，完全无约束控制通过。
- [ ] **BF131.A82 — PhysicsAsset 刚体禁碰撞响应被记录但未消费，布娃娃激活又恢复默认阻挡层**。Gate：Body 响应在生成、安装、激活/退出及重建中保留，禁碰撞体不恢复默认阻挡。
- [ ] **BF131.A83 — PhysicsAsset 的 COMNudge／InertiaTensorScale 未进入运行质量属性，质量相同但质心与转动响应错误**。Gate：坐标/单位正确的 COM 与惯量实际安装，非零偏移、非均匀惯量和单位控制通过。
- [ ] **BF131.A84 — 关闭角速度覆盖时仍将 BodyInstance 原始 MaxAngularVelocity 作为强制上限，遗漏项目有效值**。Gate：按覆盖开关解析项目/实例有效上限，多个默认值、新建/重建与实际向量钳制通过。
- [ ] **BF131.A85 — 原生 PhysicsAsset 读取器使用 BodySetup 材质而绕过 BodyInstance PhysMaterialOverride，安装错误接触系数**。Gate：有效材质路径与属性同源，单体覆盖不污染共享材质，无覆盖正确回退，运行摩擦/恢复系数通过。
- [ ] **BF131.A86 — 关闭源陀螺力矩后，目标仍按非球对称惯量自动施加陀螺修正**。Gate：开关贯穿源到积分；非球对称多轴旋转、开关切换、睡眠唤醒/重建及其他力学作用控制通过。
- [ ] **BF131.A87 — 单个物理形状的 CollisionEnabled 未进入运行过滤，QueryOnly 等模式被当成普通阻挡 Collider**。Gate：六种 shape 模式与 Body 模式按源交集消费；同 Body 主阻挡形状与辅助 QueryOnly 并存，查询和物理解耦。
- [ ] **BF131.A88 — 源端允许相互碰撞的关节仍保留 JointCollisionDisabled，在用户钩子前屏蔽合法碰撞对**。Gate：关节标志与资产对子表四种组合经过实际 spawn/替换/JointGraph/broad phase，运行开关和重建通过。
- [ ] **BF131.A89 — 自定义关节缺少源端速度约束阶段，单子步下位置纠错留下非物理速度**。Gate：正确调度位置求解活动轴速度；单子步误差、零迭代、自由轴、双 Dynamic、非零锚臂和接触控制通过。
- [ ] **BF131.A90 — 缩放布娃娃回读归一化忽略逐骨屏蔽，改写本应保留的动画骨骼**。Gate：按实际成功回读及父空间归一化；root/child 四种组合、全关闭、缺失实体、中间未映射骨及完整回读控制通过。

### 活动记录

- BF131 登记：开始 2026-09-27 19:28 +08:00；预估结束 2026-09-27 19:43 +08:00；实际结束 2026-09-27 19:40 +08:00；偏差 -3 分钟；结果：37/37 原编号录入，`git diff --check` 通过；拆分：后续逐项 S1/S2/S3。当前 HEAD 与修复基线相同，工作区初始干净；未把继承的审阅结果记成本地通过。旧 `crates/als-ext` 等路径按 `src/README.md` 映射。网页附件下载未取得文件，清单内容直接取自预览；具体修复仍重新提取本地 source。
- BF131.A13-S1：开始 2026-09-27 19:34 +08:00；预估结束 2026-09-27 20:14 +08:00。依据：归档 A13 已验证当前 player 骨骼映射和同位姿 Avatar 替换，本轮限定为脚目标坐标转换的 Avatar 身份与不同变换反例；先追踪转换函数全部调用者及本地 ALS 当前 Mesh socket/组件空间合同，再决定最小修改。
  - Source 闭包：fresh 原生 `CR_Als` reader/watchdog 成功；本地 `AlsAnimationInstance::RefreshFeetOnGameThread/RefreshFeet/RefreshFootLock` 从当前 Mesh 读取世界 socket，并用同一当前组件逆变换输出 Rig 输入。目标 `refresh_current_foot_bones`、`publish_control_rig_inputs` 已使用当前 Assembly；`refresh_foot_lock_inputs`、`detect_foot_lock_teleports`、`observe_rig_results` 仍选第一个 Avatar 子实体。仅复开归档 A13 的该残留边界；最小不同平移/旋转/缩放反例已加入，正在运行。watchdog focused self-test 通过。下一步确认 red 后统一这三处当前 Avatar 选择，并让既有真实替换 Gate 保留旧 Avatar 标记。
  - S1 实际结束 2026-09-27 19:50 +08:00，偏差 -24 分钟，结果：原生 source 与 Rust 失败反例闭合。首版夹具未给旧 Avatar 子树，ECS 顺序偶然 green；补齐旧子树后稳定 red，局部目标为 `(-17.49831, 3.1499999, -3.7979918)` 而期望 `(0.2, 0.1, 0.3)`。S2 开始 2026-09-27 19:50 +08:00；预估结束 2026-09-27 20:25 +08:00。依据：三处同文件身份搜索、既有 ECS 夹具与真实替换阶段；直接使用 Assembly 身份，不增加缓存、helper 或配置。
  - S2 静态闭环：三处均直接取 `Assembly.avatar`；定向 `foot_ik_apply::tests` 9/9、原 Avatar 缓存替换 1/1 green，旧子树 red 作为删除身份选择后的反例。无新增运行分支或持久状态。348 份图编辑器审计、位置与运行资产边界通过。作者合同发现基线既有标签漂移：BF129.4-B 已将运行 wrapper 标题改为 `AB_Als`，manifest 仍为 `Generated AB_Als`；仅校正该元数据并绑定本轮 fresh `CR_Als`，不改图或转换规则。下一步重跑作者合同后，按 `run_manual.bat` 的 `target_game/play` 优化配置预构建，执行 startup 与加强后的 foot_ik 公共功能 Gate；沿用归档中用户暂停性能工作的验收范围。
  - S2 实际结束 2026-09-27 19:59 +08:00，偏差 -26 分钟，结果：作者合同复核与实际 `target_game/play` 优化预构建通过。S3 开始 2026-09-27 19:59 +08:00；预估结束 2026-09-27 20:19 +08:00。依据：已有 startup/foot_ik runner、原替换 watchdog 与本轮只保留旧 Avatar 标记/改变其隐藏变换的受控路径；先 startup 后 foot_ik，审计退出码、坐标/姿态和全部场景覆盖，失败时回到静态链路诊断。
  - S3 首轮：startup 功能 PASS；foot_ik 21/22，脚骨/脚锁/相机替换通过，唯一失败为 `animation_graph`。保留旧 Avatar 标记后，未停播旧播放器，而当前输入发布器只维护选中 Assembly；旧图因缺少 Rig 查询上下文报错。该运行不作为通过证据，报告对旧实例错误缺 `primary_failure`，已保留原日志。受控路径改为停播旧动画，仍保留旧 Avatar/骨骼/标记及不同变换；新实例继续真实图求值，不改变生产输入或放宽 Gate。下一步重新预构建，startup 后重跑同一场景。
  - S3 第二轮：startup 功能 PASS；foot_ik 仍 21/22，但当前图、姿态、时序与 watchdog 均无错误。静态回溯确认 `observe_manual_integration` 与 `anim_probe_logger` 仍按角色枚举首个 Avatar，分别把旧实例的 `AnimationSource::None` 当作当前图失效和 A-pose。该两处观测 consumer 属本次保留旧 Avatar 的直接链路，纳入本项并复用现有 Assembly 查询；删除首项扫描，不改变成功条件。真实保留旧实例场景即其可重复失败/回归步骤，下一步同入口复验。
  - S3 实际结束 2026-09-27 20:19 +08:00，偏差 0 分钟；结果：最终 startup/foot_ik 公共功能 Gate 均退出 0/0，foot_ik 22/22，fault/graph error/temporal/watchdog 为 0。生产与观测共五处使用现有 Assembly 身份；无新抽象/缓存/配置，原错误选择稳定 red、修复后 10 项定向测试及真实保留旧实例 Gate green。拆分结果：A13 残留闭合，归档状态恢复，A40 接续。性能预算维持既定延期，未声称完整性能验收；估时校正：后续实例切换条目的 source 闭包应同时检查验收/日志身份选择。
- BF131.A40-S1：开始 2026-09-27 19:56 +08:00；预估结束 2026-09-27 20:26 +08:00。A13 预构建期间只读取这一独立条目的 source/target；生产修改和测试编译留在 A13 完成后，避免更改正在预构建的源码。依据：归档 A40 的显式 `NoDecalReceiver` 交接夹具、`prepare_receivers` 的两个消费者循环及同一 Playground footprint 替换路径；本轮仅核对无显式退出标记时透明材质的交接。
  - Source 闭包：fresh 原生 `M_Als_Footprint_Decal` reader/watchdog 成功，材质域为 `MD_DeferredDecal`；本地 UE `UMeshComponent::SetMaterial` 更换有效材质并标记 render state dirty，`BasePassPixelShader.usf` 的 DBuffer 接收明确排除透明接收材质。目标 `prepare_receivers` 在已转换循环只处理显式排除，新透明标准材质则在第二循环早退，留下旧代理。下一步扩展现有同帧材质交接夹具，先确认无排除标记的透明替换 red，再在同一清理条件修复。
  - S1 实际结束 2026-09-27 20:23 +08:00，偏差 -3 分钟；结果：旧显式排除控制通过，无标记透明替换在 `Old receiver proxy must be removed: excluded=false` 稳定 red。归档 A40 仅此残留边界复开。S2 开始 2026-09-27 20:24 +08:00；预估结束 2026-09-27 20:44 +08:00。依据：一处清理条件与既有材质/缓存测试、原 Playground receiver swap 夹具；新标准材质出现即交还所有权，不新增缓存/材质类型/标记，真实夹具改用透明材质且不添加 NoDecalReceiver。
  - S2 初验：一处条件增加 `meshes.contains(entity)`，同帧透明材质反例与原显式退出、共享缓存、材质更新、GPU 检查模块 8/8 green。当前临时删除新增条件运行消融；恢复后再预构建并执行真实入口。
  - S2 实际结束 2026-09-27 20:29 +08:00，偏差 -15 分钟；结果：删除新增条件后同一反例 red，恢复后 green；最终生产差异仅一个所有权交接条件，无额外抽象。S3 开始 2026-09-27 20:29 +08:00；预估结束 2026-09-27 20:44 +08:00。依据：本轮定向构建与 A13 的约三分钟优化预构建、现成 footprint GPU/交接 watchdog；使用真实 startup 后运行 `ALS_MANUAL_DECAL_RECEIVER_SWAP=1` 的透明替换路径，必须保留标准材质并移除旧代理，原足迹/GPU 检查全部通过。
  - S3 实际结束 2026-09-27 20:37 +08:00，偏差 -7 分钟；结果：startup/footprint 公共功能 Gate 均退出 0/0，透明标准材质保留、旧 Packed/Source 组件移除，GPU 36 请求/576 采样通过，图/姿态/时序/watchdog 无错误。拆分结果：A40 残留闭合，原显式退出保持回归覆盖；性能预算沿既定范围延期。source、8 项定向测试、条件删除 red/恢复 green 与真实入口组成完整闭环；无新增类型/缓存/配置，A56 接续。
- BF131.A56-S1：开始 2026-09-27 20:25 +08:00；预估结束 2026-09-27 20:55 +08:00。A40 验证期间只读取独立的缓存作用域链路，暂不修改源码。依据：`source_cached_pose.rs` 的 prewarm、pose 和 context 三处 arena 全局首项搜索，以及 UE Use/SaveCachedPose 的实例节点指针合同；需要继续确认 target context 父子关系和现有 linked-instance 公开扩展点后建立两个实例同键反例。
