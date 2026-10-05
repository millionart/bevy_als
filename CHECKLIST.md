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

## BF134 — 全范围 P2 及以上移植差异审计与修复

用户于 2026-10-05 将全部移植范围选为当前任务。按本地 ALS/UE source 与真实 target 消费链逐模块核对；排除附录 A 的刻意差异、网络同步、AI 示例和仅编辑器功能。P0 为核心入口不可用/数据损坏，P1 为常用核心行为错误，P2 为受支持输入、配置或生命周期下的明确行为错误。未经 source 证实的观察不定级、不修复。

- [x] BF134.00 建立 fresh source/target 覆盖清单与差异台账。开始：2026-10-05 06:12 +08:00；预估结束：2026-10-05 06:42 +08:00；实际结束：2026-10-05 06:17 +08:00，偏差 -25 分钟，结果：完成覆盖登记。依据：本地插件 146 个 C++/header、目标 181 个 Rust 文件、已安装 15 AnimBP/递归资产与 BF131–BF133 的真实记录；先确认实际源位置、模块入口、既有 Gate 与刻意差异，不重新运行已归档任务。Gate：覆盖清单包含全部移植模块和未审计边界，台账记录每个确认差异的源函数/资产、触发条件、target consumer、严重级、反例与修复验证。
- [ ] BF134.01 角色状态、输入、运动与动作逐链审计及修复；按发现项拆为独立 Gate/提交，完成 native 算法、参数、坐标与生命周期闭包。
  - [x] BF134.01A 地面刹车遗漏源低速归零。S1 开始：2026-10-05 06:18 +08:00；预估结束：2026-10-05 06:53 +08:00。依据：fresh B_Als_Character、UE CalcVelocity/ApplyVelocityBraking 与全部五条 target helper/预测/motor 调用链；先以实际 motor 和零刹车/零 Delta/正向超速控制复现，再生成源停止阈值、修复并消融。S2 为当前入口预构建、startup/grounded/standing_stop Gate，启动时登记估时。子步划分差异单独审计，不提前修改。
    - S1 实际结束：2026-10-05 06:35 +08:00；偏差 -18 分钟。actual motor 低速反例 exit 101 后，源 10 cm/s、长度平方小数阈值与 MIN_TICK_TIME 经已有 movement importer 生成；仅三项新增值，其他生成常量 diff 0。15 项 dynamics 恢复回归通过（79.497 秒），停止阈值与 minimum Delta 两独立消融均语义 red，原始字节/mtime 恢复；无新增 helper/state/adapter。声明式根 ABI、全部 ALS/Camera position、510 个 editor 产物及 runtime 资产边界通过。两次测试构建 profile 不匹配和首次消融 shell 参数错误保留为工具失败，不计反例/Gate。为完整执行共享转换规则纪律，将后续独立拆为 S2 全量转换核对、S3 入口验收。
    - S2 开始：2026-10-05 06:35 +08:00；预估结束：2026-10-05 07:15 +08:00。依据：BF132 同类 15 root/39 source/374 全量产物 fresh 转换实际 1545 秒；按全部当前 installed manifests 重新建立集合，重放完整转换并核对新旧 diff。S1 源/static 闭包已经通过，runtime 尚未启动；本轮不得选择性缩小转换集。
    - S2 实际结束：2026-10-05 07:05 +08:00；偏差 -10 分钟。全量 native 转换 1513.581 秒，15 root/39 source/374 产物新增、删除、修改均 0；fresh 345 图规格、15 个完整递归 source closure、authoring/layout/editor/boundary 与 closure self-test 通过。独立 spec 程序最初未注入绝对资产根而退出 8，注入后 1.918 秒通过；审计摘要首次访问不存在 totals 字段已纠正，不计 Gate。当前最终正常优化 Playground 预构建已完成 156.072 秒，尚未启动 runtime。
    - S1b 开始：2026-10-05 07:06 +08:00；预估结束：2026-10-05 07:32 +08:00。最终 caller 核对发现 simulation 的严格 `> 1e-6` 漏掉源允许的精确 MIN_TICK_TIME，导致同一低速归零未到达 consumer；旧反例覆盖 0.001 秒和直接 helper 的零 Delta，漏掉 outer loop 等号边界。先补该边界 red，再使用已有生成 minimum Delta 修复 loop，恢复对应回归；不重复未变的生成树/原生事实。后续真实入口沿本项继续。
    - S1b 实际结束：2026-10-05 07:14 +08:00；偏差 -18 分钟。精确 Tick 反例旧值 `(0.08 m/s, 0 step)`、源期望 `(0, 1 step)` 稳定 red；仅把已有循环条件改为生成 minimum Delta 的 `>=`，15 项恢复通过（76.850 秒），没有新分支/状态。当前最终 `target_game/play` 预构建 146.507 秒通过；生成器/产物/source 与 root ABI 未改变，S2 全量证据仍有效。
    - S3 开始：2026-10-05 07:15 +08:00；预估结束：2026-10-05 07:35 +08:00。复读当前闭包后确认：源停止阈值、零/微小/精确 Tick、实际 motor、无刹车/纯摩擦/超速保速、两个必要性消融、15 定向回归与全部 source/static/完整生成 diff 已闭合；下一边界只限最终二进制的 startup/grounded/standing_stop 公共 Playground Gate。沿既有截图关闭/完整性能暂停范围采用 CaptureCount=0/FunctionalOnly，保留性能失败，不宣称完整性能验收。
    - Runtime 摘要（07:17 +08:00）：同一最终二进制 startup 15.517 秒功能 runner/validator 退出 0，issues 空、root-ready 与 graph/pose 检查通过；原始 max_eval_p95_ms 性能失败保留，full_pass=false。下一边界为 Grounded 与 Standing Stop 动作覆盖；不调整阈值、不重跑已有效 source/static。
    - S3 实际结束：2026-10-05 07:21 +08:00；偏差 -14 分钟，结果：本项修复完成。最终 startup 15.517 秒、Grounded 15/15（42.178 秒）、StandingStop 10/10（26.562 秒）功能 runner/manual/validator 均退出 0；ready=true、graph/primary failure/fault/temporal 异常全 0。source/static/两个消融/精确 Tick red→green/15 回归/345 规格/全 15/39/374 重放与完整 diff=0 闭合；最小修改只含三项离线源常量、停止门槛/Delta 条件与原循环等号，没有新状态/适配层。截图仍关闭，max_eval_p95_ms 延后失败保留，结论只为功能验收。四段的实际结束/偏差与中途漏检已记录；勾选单独提交，整体 BF134 仍未完成。
  - [x] BF134.01B 鼠标 Look 数值被额外乘以 0.0025 radians/count。S1 开始：2026-10-05 07:23 +08:00；预估结束：2026-10-05 07:58 +08:00。依据：fresh B_Als_Character 的上下/左右 sensitivity=1、原生 IMC 默认 Mouse2D 无 mapping modifier、UE EnhancedPlayerInput 直接消费 RawValue、项目关闭 LegacyInputScales、AddYaw/PitchInput 按 degrees 累加。还需提取 IA 自身 modifier 与实际 controller 证明完整输入链，再用真实 Action→bridge→view 反例确认并修复；旧测试以目标常量重算期望，未提供源角度。影响动作场景的注入 counts 必须按同一源单位重算，不修改 source 算法或放宽 Gate。
    - S1 实际结束：2026-10-05 07:42 +08:00；偏差 -16 分钟。IMC/IA/Controller 原生读取成功，IA 无 modifier；实际 raw MouseMotion Action/bridge 已到达但角度只有源约 1/7，Rust 反例稳定 red，修复后 11 input/6 view 通过。删除额外倍率，沿 std degree/radian 换算，没有新 helper/state/branch；rate 与鼠标 Delta 生命周期保留，场景注入已统一反算单位。当前 root ABI/layout/editor/boundary/source hashes 通过；转换器、图、资产未修改，不重转有效的 S2 15/39/374 全量证据。首次猜错 IMC 路径退出 3 已通过实际文件定位纠正，不记 source fact。
    - Static manifest 说明（07:42 +08:00）：最初尝试只引用已通过的 root ABI 而不重复 graph 声明，validator 拒绝空 graph 和缺 editor/layout scope；已读取失败报告并显式登记同一 root ABI/原验证命令，再通过源 hashes/声明式 Gate。失败属于 manifest schema，未改工具或降级 Gate，也未启动 runtime。
    - S2 开始：2026-10-05 07:42 +08:00；预估结束：2026-10-05 08:02 +08:00。依据：17 项恢复、完整输入源链及 std 单位换算；预构建实际入口，顺序运行 startup/camera/head/standing_turn 公共功能 Gate，核对精确 source branch/姿态/图/进展结果。只有 scope 内功能验收，保留截图关闭与完整性能延后。
    - S2 实际结束：2026-10-05 07:55 +08:00；偏差 -7 分钟，结果：本项完成。最终同启动器预构建 145.414 秒，startup 15.436 秒、Camera 11/11（20.192 秒）、Head 14/14（21.793 秒）、StandingTurn 11/11（46.225 秒）功能 runner/manual/validator 均退出 0，ready=true，graph/primary/fault/temporal 异常全 0。17 回归、真实 raw MouseMotion 失败→修复、源自述单位期望、输入清零/鼠标不乘 Delta、源参数/ABI/layout/editor/boundary hashes 通过；只删除额外倍率，无新运行时分支/状态/适配层。图、生成器、资产没有变化，完整转换 diff=0 证据有效；截图关闭、完整 max_eval_p95_ms 失败保留。勾选单独提交，整体目标未完成。
  - [x] BF134.01C 输入 Action 累加方式：fresh IA_Als_Look 为 TakeHighestAbsoluteValue，目标 ActionSettings 默认为 Cumulative；追踪实际 source mapping 顺序与相同绝对值时后者覆盖、目标 MaxAbs 的前者优先差异，再读取 Move 自身设置并留下组合键/跨设备真实反例。只登记 source 已指向的输入闭包，开始时登记 S1 时点。
    - S1 开始：2026-10-05 07:56 +08:00；预估结束：2026-10-05 08:36 +08:00。依据：fresh IMC/Look 原生记录、目标两个 Axis2D Action 和共享 bridge、UE per-axis `>=` 与 Bevy MaxAbs `<`，以及上一项真实输入反例约 80 秒的定向构建。先核对普通 mapping 重排/迭代次序、Move 自身 accumulation 与实际 Binding 顺序，留下真实组合键/跨设备/平值边界；复用公开 ActionSettings 与绑定次序，不改依赖。S2 真实入口在源/static/反例/消融闭合后另记开始/结束。
    - source/static 摘要（08:25 +08:00）：fresh Move 原生读取 34.561 秒成功、与 Look 同为 TakeHighestAbsoluteValue；Source 普通映射保持 W/A/S/D/gamepad、Up/Left/Down/Right/gamepad 的资产顺序，逐轴平值后者覆盖。真实组合键反例 red 后，原生 ActionSettings::MaxAbs 和 source 反向绑定使 13 项输入（组合/普通/跨设备强弱与平值/清零/触发）green；两处分别消融均 11 pass/2 semantic fail，已 byte/mtime 恢复，最终回归正在执行。公开 SpawnRelated 的 Bindings 需要 Spawn(Bundle)，测试 Axis::set 接受 Into 而不用再次 .into，两次编译错误按实际 API 修正；消融摘要最初只查 stderr 而漏掉已捕获的 stdout panic，原失败证据经读取核实有效，仅补跑另一个消融。没有新 aggregator/evaluator/state/cache/依赖；fresh 源 hashes 与 root ABI/layout/editor/boundary 通过，图/资产/转换器未改。下一步最终回归与实际入口。
    - S1 实际结束：2026-10-05 08:29 +08:00；偏差 -7 分钟，结果：源算法、映射次序、真实 keyboard/gamepad 两条反例、两独立消融、13 最终恢复回归（75.685 秒）与源/static 闭合。保留原公开 prefab/API，无依赖、资产或 converter 改动；上项全量 15/39/374 diff=0 证据仍有效。
    - S2 开始：2026-10-05 08:30 +08:00；预估结束：2026-10-05 08:50 +08:00。依据：13 恢复/两消融与 actual Action→bridge 证明，预构建当前 target_game/play 后运行 startup/interface/grounded 公共功能 Gate，核对输入/UI/原图姿态与进展。字段/行为范围只含两个 Axis2D Action accumulation 和 source 平值优先级，不扩展已发现的 dead-zone、rate 时钟或 pitch 待项。
    - Runtime 前摘要（08:34 +08:00）：最终 target_game/play 同启动器预构建 146.414 秒通过，13 恢复/两独立消融/current 源 hashes、根 ABI、全部 layout/editor/资产边界闭合。现在启动无驱动 startup，之后只执行既定 interface 与 grounded；未改 source 图、算法默认参数或资产，不扩大当前行为范围。
    - S2 实际结束：2026-10-05 08:44 +08:00；偏差 -6 分钟，结果：本项完成。最终 startup 15.921 秒、Interface 15/15（23.612 秒）、Grounded 15/15（42.382 秒）功能 runner/manual/validator 全退出 0，ready=true，graph/primary/fault/temporal 异常为零；原始 max_eval_p95_ms 失败保留、截图关闭，不宣称完整性能验收。源 ordinary mapping 次序/per-axis `>=`、actual keyboard/gamepad 失败→修复、accumulation/顺序两独立消融 11 pass/2 semantic fail、byte/mtime 恢复后13 green、source/ABI/layout/editor/boundary 闭合。只改变两个现有 ActionSettings 和原绑定的次序；无新 aggregator/evaluator/cache/state/消息/依赖、图或资产。S1 提前7、S2 提前6分钟，工具准备失败与消融摘要误读均保留且纠正。勾选单独提交，下一项 .01D；全范围目标未完成。
  - [ ] BF134.01D rate Look 非缩放时钟：fresh IA_Als_Look 使用 ScaleByDeltaTime，EnhancedPlayerInput 将 NonDilatedDeltaTime 传给 modifier，目标 view 使用 Time<Virtual> 的 Delta；验证 Slomo/正常/暂停和鼠标独立性。开始时登记 S1 时点。
  - [ ] BF134.01E view pitch 边界：fresh Controller CameraManagerClass=None，UE SpawnPlayerCameraManager 明确回退原生类；其构造限值 ±89.9 degrees，目标为 ±85。核对 Source Controller 图无覆盖，再以当前真实 view consumer 验证边界并修复。开始时登记 S1 时点。
  - [ ] BF134.01F CMC/braking 子步划分：source GetSimulationTimeStep 与 ApplyVelocityBraking 使用 remaining/2 及独立 braking 子步，目标统一 min-step 并共用预算；已明确源函数/实际 motor/预测消费者，需独立失败反例后修复。开始时登记 S1 时点。
  - [ ] BF134.01G 输入有效性归属：source SetInputDirection 先 GetSafeNormal，RefreshInput 再判 bHasInput，InputBlocked 在 ConsumeInputVector 清零；目标 gait、rotation、actual-motion、provider 使用不同原始输入门槛，provider 未读 blocked。以归一化阈值/真实阻塞消费者和帧阶段核对后分项定级修复。开始时登记 S1 时点。
  - [ ] BF134.01H gamepad dead-zone 二维归属：fresh IMC 的 Left2D/Right2D 以二维值消费各自 DeadZone，目标 Axial preset 展开为两个独立 binding 再分别执行同一个 modifier；需按源 Type/Lower/Upper 与斜向/仅单轴实际输入留下最小反例后定级修复。当前只登记受 source 映射证明的相邻输入闭包，不把轴向竞争测试当二维 dead-zone 的验收。
- [ ] BF134.02 动画图、播放器、曲线、同步、通知、根运动与 Control Rig 逐链审计及修复；遵循 animation-graph-equivalence，转换规则修改后全量重转并核对完整 diff。
- [ ] BF134.03 PhysicsAsset/物理时钟、相机、Overlay、关卡、材质与 UI 逐链审计及修复；只有限复开有 fresh source/症状/consumer 证据的旧项。
- [ ] BF134.04 全范围覆盖核销与真实 Playground 最终 Gate；所有确认的 P2+ 项通过自身 Gate 后才可核销总目标。

活动摘要（06:12 +08:00）：仓库起始 HEAD `9105cdf`，工作区干净；实际可读源在插件源码 checkout，工程插件是打包目录的符号链接。BF132 的 49 项与 BF133 的两项已完成，作为漏检边界参考，不凭标题复开。当前只有清单/覆盖登记，尚未宣称任何模块审计通过；下一步从角色和 CMC 的 native 入口建立 source-target 清单。

覆盖 Gate（06:17 +08:00）：当次 inventory 列出全部 146 个源文件（ALS 112/Camera 12/Editor 16/Extras 6）及 SHA-256、181 个 target Rust、15 个 installed root/359 个生成依赖文件，缺失文件 0，连同 15 份 manifest 为 374 个完整生成产物。源码 revision ALS `b754d6f`、UE `396c9f0`；B_Als_Character 原生 reader 39.050 秒退出 0，watchdog 的停滞/子进程/正常完成/构建时效 self-test 全 PASS。台账覆盖八个运行时闭包且明确未核销边界；只完成 inventory，不宣称功能审计完成。下一步 BF134.01 的源刹车归零反例。
