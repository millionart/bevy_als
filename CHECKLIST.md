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

- [x] **BF131.A56 — Scoped cached pose lookup ignores linked-instance ownership**。Gate：同图多个 linked instance 的缓存只在所属实例内解析，正常同实例复用与缺失诊断通过。
- [x] **BF131.A57 — Graph footstep dispatcher mishandles reverse playback**。Gate：正播/倒播、跨循环和多圈步进按源事件区间分派，不漏发或重复。
- [x] **BF131.A58 — Point NotifyTriggerChance is imported but not consumed**。Gate：点通知概率 0/1 及中间值按源分派规则生效，保留其他过滤条件。
- [x] **BF131.A59 — FSM getter requires one arena-wide static GUID match instead of owning linked-instance scope**。Gate：同 GUID 的多 linked instance 从所属实例读取状态，无关实例不造成歧义。
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
  - S1 实际结束 2026-09-27 20:49 +08:00，偏差 -6 分钟；结果：fresh 原生 AB_Als_Layering reader 成功；UE `UseCachedPose` 通过 `FPoseLinkBase::AttemptRelink` 从当前 AnimInstance 对象取得节点地址。目标缓存键仅编码源资产和缓存名，两个实例相同；真实 NodeContext 子树夹具第二实例预期 2、实际读到 1，稳定 red。三处 lookup 均需同一实例所有权规则；当前转换器已用 LayeredGraphNode 表达 external linked graph/layer，用 LinkedAnimLayerNode 表达内部 layer，可复用该语义区分。
  - 接口边界与维护代价：锁定基座 `345588a8` 的父子关系已在 GraphContextArena.hierarchy 中，但类型化公开 API 未提供父节点读取；GraphIoEnv 也只提供数据/时间通道。允许本项在现有专用支撑分支上补一个只读父上下文查询及其回归，目标端复用现有层级判断实例根；维护范围为该 API、对应 Cargo.lock 和缓存 resolver，不增加并行所有权注册表。依赖仓库 master 与锁定基线不同，保持原 checkout，另建锁定基线隔离工作树；Codex managed worktree 工具不能指定另一仓库，故依赖仓库使用 Git 原生工作树。
  - S2 开始 2026-09-27 20:50 +08:00；预估结束 2026-09-27 21:35 +08:00。依据：基座一处只读 getter、三处重复查找和已 red 的双实例夹具；先本地验证再更新专用依赖引用，补同实例/缺失缓存/父子层级控制与消融。S3 将单独登记原 Playground 动画公共 Gate，当前不修改转换规则或生成资产。
  - S2 source/static：基座只读 getter 复用既有 hierarchy，父/子/孙/状态分支与重建、快照 2/2 通过；本地依赖覆盖下缓存相关 11/11 通过，包括双实例 Pose/Curve 隔离、同实例兄弟 context 复用、缺失缓存不借用其他实例及原 FSM 生命周期/权重。三处全局扫描合并为一个 resolver，按现有 LayeredGraphNode 外部实例边界匹配 owner；未新增状态或序列化字段。删除 owner 过滤后同一双实例反例重新得到 1 而非 2，消融 red，现已恢复。下一步提交该已验证基座 API 并更新现有支撑分支引用，去掉本地覆盖后复核，再预构建真实入口。
  - S2 实际结束 2026-09-27 21:14 +08:00，偏差 -21 分钟；结果：支撑分支只读 API 提交 `a119afe`，锁文件仅更新同仓五个 Git revision；无本地覆盖时 `--locked --offline` 缓存回归 11/11，作者合同通过。在线锁文件更新卡在 crates.io 索引，被固定无进展 watchdog 结束；已有 Git/registry 缓存的离线路径成功，未更新其他依赖。S3 开始 2026-09-27 21:14 +08:00；预估结束 2026-09-27 21:34 +08:00。依据：同 target_game/play 预构建与既有 startup/state_machine/default_overlay 公共功能路径；双实例边界由实际 NodeContext 失败/通过夹具证明，真实场景用于现有图、状态与姿态回归。
  - S3 首轮：startup PASS；state_machine 图/姿态/时序/watchdog 均正常，唯一失败为 P1 原地旋转覆盖。静态追踪证明旧 Gate 要求 `AlsTurnDrive.rotate_weight > 0`，但当前运行发布器仅维护旋转 flags/lr/rate，该权重保持默认 0；现成 Standing telemetry 已直接读取实际 FSM 状态与 Pose。fresh AB_Als_Standing reader 成功，目标已映射实际 `Rotate Left/Rotate Right` 为状态 3/4。仅修复本 Gate 消费者，采用已就绪且完成迁移的实际状态；旧判断先抽成可测 helper 并加入失败反例，不修改玩法或旋转算法。
  - Gate 读者静态闭环：旧权重判断在实际状态 3、Pose ready、迁移结束时给出 0 而非左旋覆盖位 4，focused red；改读 source state/pose 后 green，同时拒绝未就绪、迁移中、Idle 和无 telemetry，原左右 Turn 位保持。生产旋转逻辑无改动。重新预构建后复跑 startup/state_machine/default_overlay，仍保留原失败报告；本轮尚在 S3 预估结束后的 20 分钟容差内。
  - S3 超时复盘：原开始 2026-09-27 21:14 +08:00，原预估结束 2026-09-27 21:34 +08:00，实际复盘结束 2026-09-27 21:57 +08:00，已超过 20 分钟容差，暂停新实现/运行并回读 source/target 与两轮报告。缓存修复、基座 2 项和缓存 11 项回归已闭合；漂移来自旧 state_machine Gate：先修复零权重假阴性，复跑后旋转仅覆盖左侧。余下边界为场景是否通过真实输入保持 Aiming，以及原地旋转的实际状态完成条件；不更改玩法或取消失败 Gate。已复审 S1/S2/S3 全过程，后续按输入合同→最小驱动修正→同入口验收收敛。新一轮开始 2026-09-27 21:57 +08:00，预估结束 2026-09-27 22:27 +08:00，按上一轮 20 分钟窗口的 1.5 倍登记；S3 与未执行的 default_overlay 保持开放。
  - 输入合同闭环：本地 ALSExtras Input_OnAim 调用 SetDesiredAiming(bool)，ALS 根据 bDesiredAiming 选择 Aiming；目标 aim_rotation_mode_system 同样以 effective_input.aim_held 决定模式，并在未按住时恢复自由模式。P1 驱动每帧 reset 输入却只写 RotationMode/request，旧运行 mask=0111；已补一行在前两个 Aiming 窗口保持 aim_held，后两个第一人称窗口释放。既有输入覆盖/按住/释放/冲刺优先级 5/5 通过，保留原失败场景作为可重复反例，不改窗口时长或玩法。
  - 输入修正后真实 state_machine 覆盖 10/10，左右旋转均达到实际状态；公共视觉仍失败于 6 条时序异常，全部恰在脚本 1.3 秒/2.6 秒的 180 度视角或模式切换帧。现有脚本只为 3.9 秒同类跳变 rebase。已沿用公共边界机制补齐三处明确人工输入边界，不改连续帧阈值或玩法；旧边界 helper 在 1.3 秒返回 None 的 focused red 已确认，新增检查同时约束边界前后和暂停时不得重复 rebase。下一步 green 后以新二进制复跑原三场。
  - S3 新一轮实际结束 2026-09-27 22:17 +08:00，较新预估 22:27 提前 10 分钟；结果：最终 startup/state_machine/default_overlay 均退出 0/0、公共功能 PASS，状态机 10/10、DefaultOverlay 17/17，图/故障/时序/watchdog 为 0。拆分结果：A56 闭合，基座支撑提交 a119afe；P1 旧读者/输入/人工边界已纳入可重复回归，生产旋转与阈值不变。无新实例注册表/配置/资产字段，三个缓存查找共用现有层级；基座 2、缓存 11、瞄准 5、Gate 读者/边界 2 项定向检查通过，owner 过滤消融 red/恢复 green。性能预算维持既定延期；本轮估时偏差主要来自旧大场景验收驱动，后续共享图变更先核对声明式驱动的真实输入及过期观测字段。
- BF131.A57-S1：开始 2026-09-27 21:16 +08:00；预估结束 2026-09-27 21:46 +08:00。A56 预构建期间只核对 source/target，不修改运行源码。依据：`graph_footstep_clocks` 的正播固定方向、dispatcher 的非循环回退早退、既有倒播感知 EventClock 与实际 clip 分派夹具；对照本地 UE `GetAnimNotifies` 的带符号步进、循环分段和反向端点合同。下一步核对源时钟可用信息，先复现非循环倒播与反向跨圈，沿用源循环预算，不展开 A58 概率过滤。
  - S1 超时复盘：原开始 2026-09-27 21:16 +08:00，原预估结束 2026-09-27 21:46 +08:00；实际复盘结束 2026-09-27 22:06 +08:00。源读取与调用链核对已完成，目标未开始实现；漂移主要为等待 A56 的旧验收入口修复。已确认 SourceRelevantAsset 缺少有效播放方向，UE 的 DeltaTimeRecord 保存经倍率/同步调整的带符号位移；尚需原生核对反向多圈预算及通知顺序，再建立 dispatcher 失败反例。暂停本项新实现/运行并按剩余边界重拆：原生时钟证据与 red fixture 为 S1，修复与实际 footstep Gate 后续分别登记。新一轮开始 2026-09-27 22:06 +08:00，预估结束 2026-09-27 22:51 +08:00，按上一轮 30 分钟窗口的 1.5 倍；继续同一 A57，不扩大到概率或同步角色修复。
  - S1 实际结束 2026-09-27 22:32 +08:00；结果：新增可选原生 NotifyClockProbe，默认 reader 输出不变；真实 Walk Forward 的 12 组正/负、循环/非循环 GetAnimNotifies 检查完成。循环 +2.6 周期输出事件索引 [1,0,1]，-2.6 周期为 [0,0,1,0,1]，证实反向多圈预算及每段保留源数组顺序。现有真实剪辑 Rust 夹具扩展倒播后，非循环脚步 [0,0] 而应 [1,1]，稳定 red。SourceRelevantAsset.delta_seconds 另被 FSM 使用，保留其帧时间语义；通知链需单独携带有效带符号播放位移。S2 开始 2026-09-27 22:32 +08:00，预估结束 2026-09-27 23:22 +08:00。剩余范围为所有当前时钟 producer、通知分段/顺序和原生矩阵一致性，随后独立登记真实入口 S3；概率过滤仍留 A58。
  - S2 实际结束 2026-09-27 22:49 +08:00，较预估 23:22 提前 33 分钟；结果：正播圈数改为带符号播放位移，原 delta_seconds 帧时间保持；Sequence/BlendSpace 与 GetUp/Mantle 时钟已接通，位置型 slot 延续原路径。leader 使用有效位移，follower 按 UE 的当前位置差及方向回绕规则；逐段通知保持原数组顺序，预算按原生探针保留。实际播放器的 12 个原生矩阵逐项一致，通知 13、FSM 33、SequencePlayer 11、BlendSpace 11 共 68 项定向回归通过，reader/watchdog 自测通过。消融方向后反向短区间多报 [1]，去顺序修正后 -2.6 周期输出 [0,1,0,1,0] 而应 [0,0,1,0,1]，均 red，恢复 green。S3 开始 2026-09-27 22:49 +08:00；预估结束 2026-09-27 23:09 +08:00。依据：原生/实际 ECS 闭环已就绪、同 target_game/play 预构建与既有 startup/notify/footprint 公共功能路径；默认资产保持正播，用真实入口验证既有通知与材质行为无回归。
  - S3 首轮：startup/notify 公共功能 PASS；footprint 的 GPU 36/576 通过、图/姿态/时序正常，但移动表面两项失败。报告显示平台阶段没有新增通知。静态回溯找到 sample_transition_stack 会将位置型时钟升级为 has_previous_position=true，却漏补播放位移；该数据补全消费者纳入本项。现有真实过渡栈测试加入实际时钟断言，在 playback_delta_seconds=0 稳定 red；现已同处补 current-previous，下一步回归过渡栈再重新预构建。漏检复盘：以后时钟 DTO 迁移还需检索字段后续赋值，不能只查结构体构造点。
  - S3 实际结束 2026-09-27 23:06 +08:00，偏差 -3 分钟；结果：最终 startup/notify/footprint 全部退出 0/0、公共功能 PASS，移动平台新增脚步/绑定/移动 GPU 恢复，GPU 36 请求/576 采样通过，图/故障/时序/watchdog 为 0。动态过渡补全时钟 red→green，过渡栈 14 与通知 13 回归通过；连同 FSM、SequencePlayer、BlendSpace 共 82 项定向测试。拆分结果：A57 闭合，原生 12 例与真实播放器矩阵、两项消融及正播真实入口证据保留；无新增依赖或概率逻辑，性能预算保持延期。A58 为下一项。
- BF131.A58-S1：开始 2026-09-27 23:14 +08:00；预估结束 2026-09-27 23:44 +08:00。依据：本地 UE FAnimNotifyQueue 构造/过滤/Reset 与 FRandomStream 源码，目标两条点通知分派入口及现成实际剪辑夹具。先取得 0/1/中间值、状态通知绕过和队列重置的原生结果，再确认零概率真实分派 red；S2 单独处理概率消费与源队列生命周期，S3 复用 notify/footprint 公共功能 Gate。
  - S1 实际结束 2026-09-27 23:24 +08:00，较预估 23:44 提前 20 分钟；结果：实际 Walk 播放夹具将加载后的 Footstep TriggerChance 设为 0，仍发出 5 次而应 0，已编译且稳定 red。fresh 本地 NotifyQueue 源码确认固定种子 0x05629063、点通知逐次抽样（含概率 0/1）、状态通知绕过、不因 Reset 清空事件而重置随机流。可选 NotifyChanceProbe 经导出的 AddAnimNotifies 原生入口取得概率 [0,1,.25,.5,.75,0,1,.5] 的接受序列 [false,true,true,false,true,false,true,true]；状态通知接受且 seed 保持 530668907，清空后继续到 -511842670，新队列首 seed 为 1546474474。首版未导出方法出现链接错误，改导出入口；第二版缺 NotifySource 被前置过滤，结果无效；最终有效资产引用结果位于 target/bf131/a58/notify_chance_valid_source.ue.json，reader/watchdog success。生产尚未修复，A58 保持未勾选。S2 剩余边界：两条分派路径都在既有权重/角色等过滤后消费概率，并明确随机流的所属实例、持续与重建；不能逐事件重置或让状态/标记误消耗随机数。
  - S2 开始 2026-09-27 23:29 +08:00；预估结束 2026-09-28 00:09 +08:00。依据：已 red 的实际剪辑、原生概率矩阵和两处已有分派状态；进一步核对 AnimInstance 与 Proxy 各有 NotifyQueue，Montage 与普通资产播放的随机流分离。限定概率抽样、队列归属与生命周期，保留现有时间/角色/权重过滤；以原生序列、真实 ECS 分派和消融闭合后再进入 S3。
  - S2 实际结束 2026-09-27 23:42 +08:00，偏差 -27 分钟；结果：两条分派入口在既有过滤后、消息和副作用前消费源随机流；Montage 复用角色事件状态，普通资产按所属 linked instance 保存队列，帧间保留、实例重建重置，状态/标记不耗随机数。复用 A56 owner resolver 并原样迁到 source_instance；没有新依赖、转换规则或图资产变更。原生 9 次抽样与种子、两角色独立队列、低权重/状态/标记、真实 Walk 的 0/0.5/1 及重建、双 linked graph 全部通过；通知 14、缓存 4、实例 1、Sequence 11、BlendSpace 11 共 41 项回归通过。三项消融均 red：移除概率得到零概率脚步 5 而非 0；合并 owner 得到 5 而非 6；重置种子得到中间概率脚步 5 而非 3，均已恢复 green。reader/watchdog 自测通过。
  - S2 静态边界：当前 authoring 合同（`--no-execute`）、源资产 ALS/ALSCamera 节点位置、全部 510 个 editor 文件及 runtime asset boundary 通过。额外执行旧 manifest 全量命令发现缓存重生成产物仍指向已迁走的 assets/generated；全 assets 位置扫描另报 Validation/Graphs 的三个旧验证图。失败报告保留于 target/bf131/a58 与 target/bf1295，不把它们记作通过，也不扩大到未改动的转换/验证图；本项 source 闭包为 NotifyQueue、RandomStream、AnimInstance/Proxy 和 Montage HandleEvents，实际通知分派及其队列归属已由 fresh native 和 ECS fixture 闭合。
  - S3 开始 2026-09-27 23:42 +08:00；预估结束 2026-09-28 00:02 +08:00。依据：41 项恢复后的定向回归、三项消融和现成 startup/notify/footprint 路径；先按启动器相同 target_game/play 预构建，再运行公共功能 Gate。真实入口仍待通过，性能预算沿用已登记延期，不改验收阈值。
  - S3 首次 startup 退出 92，报告包含有效 primary_failure，错误 MismatchedStateType，根 pose 缺失，截图由当前配置禁用，notify/footprint 未启动。原因：新实例状态与 linked Pose 输入缓存同用默认 NodeId 槽；原双 linked 夹具没有外部 Pose 输入，漏掉了该边界。已把实际输入缓存写入加入同实例/兄弟 context 夹具，先确认 red，再修复状态槽归属。
  - 附属审计入口修复（用户 2026-09-27 23:46 +08:00 明确要求）：开始 2026-09-27 23:47 +08:00；预估结束 2026-09-28 00:17 +08:00。依据：12 份 generated authoring manifest 保留迁移前命令路径，根闭包审计又引用旧 materialization；修正当前资产与产物入口、保留最小失败检查并执行完整声明命令，不以 --no-execute 代替通过。此前“不展开旧命令”边界由用户指示更新；不修改无关 Validation 图姿态或布局。
  - 命名迁移补全（用户 2026-09-27 23:56 +08:00 扩展要求）：开始 2026-09-28 00:00 +08:00；预估结束 2026-09-28 00:40 +08:00。清点为 20 个跟踪文件、一个设置子目录及一个 Python 缓存文件仍带 generated；范围为 target/target_game 外的正式文件/目录、对应模块/测试入口和活动引用。12 份正式审计清单去前缀，3 份同名旧手工清单改为 legacy 名以保留历史边界；设置常量回到 settings，通知/输入 provider 按职责命名，不删除转换功能。本轮完整命令已证实总图等 10 份通过，Camera 仅旧 wrapper 名不符（已修正），remaining overlays 的 source_actor 暴露输入目录遗漏 Default；先闭合这些入口和新名称，再恢复 A58 runtime。
  - 用户随后允许移除无用旧手工清单；核对三份旧 Standing/Crouching/Grounded 清单无当前调用入口、仍引用迁移前手工图，而正式清单与 native closure 已覆盖当前图并通过，因此删除这三份临时 legacy 副本；保留源记录与历史验收文档。
  - 附属审计入口实际结束 2026-09-28 00:15 +08:00，偏差 -2 分钟；12 份正式 authoring 清单的完整命令均通过（最后 Bow 单项复核通过），349 个当次规格、510 个 editor 文件有效。不存在/空根目录的假阳性已有 red→green；删除重复 wrapper 检查仍由全 assets 检查覆盖。Overlay actor/object 两个导入入口修正迁移前 graph/mesh/scene 输出路径及缺失的 Default 源输入，13 个 class、8 个挂件模式和 Bow 完整输出检查与现有产物一致，没有改动运行资产。
  - 命名迁移静态闭环：20 个正式文件改名，旧设置空目录及 Python 缓存移除，3 份旧手工清单删除；target/target_game 外包含隐藏/忽略文件的实际文件扫描已无 generated 路径。对应模块、插件/系统类型、Cargo 测试入口、工具默认输出与活动文档已同步；通知 14、缓存 4、provider 13、transition notify 8、图资产加载 2 共 41 项通过，三个常量导入器自测通过。全量转换已于 00:12 启动，按上一轮全量记录 23 分 51 秒，预计 00:37 完成；15 个全部入口与递归依赖经原生 reader 重提，输出仅写 target，之后完整 diff 确认影响范围。
  - A58-S3 重新估时：原窗口 23:42→00:02，首次运行 23:45 失败后已停止后续场景；根因夹具修复及用户新增的迁移/审计工作现已静态闭合，余下为全量转换 diff、同入口预构建和 startup/notify/footprint。新一轮开始 2026-09-28 00:15 +08:00，预估结束 2026-09-28 00:45 +08:00，按原 20 分钟窗口的 1.5 倍登记；保留初次 runtime 失败报告，不降低功能 Gate。
  - Runtime 前最终静态闭环（2026-09-28 00:41 +08:00）：15/15 入口与递归闭包转换成功，39 份当次原生记录，374/374 新旧安装产物完整 diff 为零；转换开始 00:12，预估 00:37，实际 00:37，约 25 分 20 秒，比归档同类记录多约 1 分 29 秒。所有正式审计命令切换到该批 source/materialization，349 份 fresh spec 和 12/12 完整 authoring Gate 通过；同 target_game/play 的实际启动器预构建成功。状态槽冲突已有包含外部 Pose 输入的 red→green，原实例/缓存隔离保持；当前仅待真实 startup/notify/footprint 功能 Gate，未复开其他完成项或修改运行资产。
  - 命名迁移实际结束 2026-09-28 00:45 +08:00，偏差 +5 分钟；结果：文件/目录命名清理、3 份无用手工清单删除、默认导入输出和全部活动引用同步完成。新旧 374 份产物零 diff、fresh source/规格对应的 12 份完整审计通过、41 项 Rust 定向回归与 3 个常量导入器自测通过；最终 startup/notify/footprint 公共功能验收通过，21/21 脚印、12/12 通知、GPU 36 请求/576 采样，故障/图错误/时序异常/watchdog 均为 0。完整报告仍保留性能失败，性能验收沿用延期；本次没有扩大到修改资产算法、清理历史验收记录或放宽 Gate。
  - A58-S3 实际结束 2026-09-28 00:46 +08:00，较重新预估偏差 +1 分钟；结果：startup/notify/footprint 均退出 0，公共功能 PASS，图/故障/时序/watchdog 为 0。新增通知实例状态使用独立槽，与既有 Pose 输入上下文共存；包含该输入的夹具和真实入口均 green。A58 的 source/native 概率序列、两条分派路径、实例归属/重建、其他过滤与三项消融闭合，勾选本项；命名/审计入口单独提交 d7b6d0e。性能 Gate 继续延期；下一项 A59，未开始其实现。
- BF131.A59-S1：开始 2026-09-28 00:49 +08:00；预估结束 2026-09-28 01:09 +08:00。依据：getter 唯一调用链 value→target_state 当前扫描整个 arena；已有 A56 实例 owner resolver 和 FSM getter 单实例夹具可复用。重新读取本地 Standing 资产与 UE AnimInstance/Proxy 的三个 getter→GetStateMachineInstance 链，先建立同 GUID 双 linked instance 与缺失本实例的失败反例；不修改转换规则或扩大到其他 getter 语义。
  - S1 实际结束 2026-09-28 00:55 +08:00，偏差 -14 分钟；结果：fresh Standing 原生记录包含 18 个 AnimGetter，Machine Weight 的 bSelfContext=True、SourceNode 指向 Standing States（GUID 72cf65bd-4544-1341-4a84-298739d5b736），与当前 RON 数据一致。UE 三个 getter 经当前 Proxy 以 AnimInstanceObject 取得 FStructProperty 节点地址，不做跨实例唯一性搜索。原测试仅检查 getter 字段及单实例时序；新增实际 NodeContext 双实例、嵌套状态图、三个 getter、帧间更新与缺失本实例夹具，编译后稳定 red：machine is ambiguous。
  - S2 开始 2026-09-28 00:55 +08:00；预估结束 2026-09-28 01:15 +08:00。限定共享 target_state resolver 的 owner 过滤，复用既有 linked_instance_context；恢复单实例、首帧权重、状态累计时间回归并移除过滤做消融。图/转换产物不变，后续单独登记 state_machine/default_overlay 真实入口 Gate。
  - S2 实际结束 2026-09-28 01:03 +08:00，偏差 -12 分钟；结果：生产仅增加共享 owner 解析与过滤（4 行），没有新增状态、抽象或转换规则。双实例同 GUID 的三个 getter、嵌套状态图、两帧更新、单实例控制与缺失本实例不借用外部状态全部 green；移除 owner 过滤恢复 ambiguous 失败，再恢复 green。FSM 34 与 getter 签名 1 共 35 项定向回归通过。Standing authoring manifest 已绑定当次源记录和新反例证据。
  - S3 开始 2026-09-28 01:03 +08:00；预估结束 2026-09-28 01:23 +08:00。依据：同入口预构建、fresh spec 与 Standing 完整声明审计，再运行 startup、manifest 声明的 standing_dynamic 及 state_machine 公共功能 Gate；以 standing_dynamic 替代未声明的 default_overlay，源覆盖仍为 Standing getter/状态机闭包，不改变性能预算。
  - Runtime 前摘要（01:06 +08:00）：当次 Standing 源绑定、18 个 getter 的源节点映射与 UE 当前实例取址合同闭合；35 项回归、owner 消融、349 个 fresh spec、Standing 声明的 12 条完整审计命令全部通过。同 target_game/play 的 manual_playground 预构建成功；生产/图数据边界仅 getter owner 过滤，未更改任何 RON 或导入规则，开始公开入口功能验证。
  - 首次 runtime：startup PASS；standing_dynamic 图/时序/故障均为 0，但左侧验收未覆盖，state_machine 未启动。源链核对限定新增 consumer 为该场景的过渡观察器：ALS PlayQueuedTransitionAnimation→UE PlaySlotAnimationAsDynamicMontage→Montage_PlayInternal 会停止同组旧实例，目标 stack 同样停止/移除旧实例；观察器却只读 latest 并要求当前项走到整段时长。报告记录左侧已播放约 0.9 秒后被同组新实例替换，观察器丢失已观察实例并永远等待左侧成为最后一项。先保留 latest 语义写失败夹具，再按 instance_id 跟踪已播放实例的停止；不改玩法、选择条件、0.8 秒播放量、脚锁恢复、Idle、pose/slot Gate。
  - 观察器 source/static 闭环（01:29 +08:00）：同组替换夹具先在 latest 查找处 red；改为已有 instance_id 后，退回旧整段时长条件仍 red，恢复停止判定 green。Standing/Crouching 两种 kind、仍活动、未播放足量、未见源分支、非 Idle 控制与原脚锁恢复/输入测试通过；只复用既有 stop_playback，不新增状态。目标实例退出后停止采集其动画窗口，终态明确记录 stopped，不把被中断实例报作完成整段。35 项 getter/FSM 回归和 2 项观察器回归通过；图/资产/玩法均未改。重新预构建后执行 startup、standing_dynamic、state_machine，并用共享观察器的 crouching_actions 检查另一 stance。
  - S3 实际结束 2026-09-28 01:34 +08:00，偏差 +11 分钟，未超过 20 分钟容差；结果：startup、standing_dynamic 5/5、state_machine 10/10、crouching_actions 11/11 全部退出 0，公共功能 PASS，故障/图错误/时序/watchdog 为 0。左侧报告保留已观察实例 8 的约 1.20 秒播放量、实例已退出、脚锁恢复和 slot weight=0，验证了同组打断的源生命周期，没有降低播放/运动/恢复阈值。37 项定向测试、owner 与观察器消融、当次源记录/完整审计闭合，勾选 A59；性能 Gate 继续延期。下一项 A60，尚未开始实现。
