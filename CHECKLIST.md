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

## BF115 按用户指定顺序恢复转换图与Actor功能

用户于2026-09-08补充真实症状：转换版下蹲、跳跃、攀爬、冲刺不可用，站立六向移动存在跛足与滑步；ragdoll流程已手动确认还原良好，相机视角和距离另有问题。UE ALS-Refactored是唯一事实标准；旧手工图与当前Bevy Actor只能作为待审查实现和差异定位线索，二者一致不能证明正确，也不能决定转换器的目标语义。

执行顺序固定为 `AB_Als_Standing → AB_Als_Grounded → AB_Als_Crouching → AB_Als_Locomotion → AB_Als_Layering → AB_Als_Head → CR_Als → AB_Als`。每项同时修复通用转换器及其对应Actor消费者；上一图自身Gate通过并提交后才开始下一图。相机剩余视角/距离问题保留在Actor集成闭包；在有新source证据前，不把它归因于已通过的ragdoll物理流程。

- [x] BF115.01a Standing六向症状与source链取证：计划开始：2026-09-08 20:30:00 +08:00；预估结束：2026-09-08 21:20:00 +08:00。估时依据为当前已完整提取的Standing 62个可达source subgraph、现有六向真实场景和旧手工实现；先稳定复现跛足/滑步，比较实际状态、clip/phase/curve、pose混合与Actor速度/步幅消费者，定位首个source差异。未定位前不以平滑、速度倍率或换回手工图作为修复；后续实现按证实的最小闭包另立不超过60分钟的验收段。
  - 依赖实际完成后登记开始：2026-09-08 20:50:00 +08:00；预估结束：2026-09-08 21:40:00 +08:00。此前计划时间为等待ragdoll依赖，并未提前开展Standing；本轮只核对已选Standing source/runtime闭包。
  - 初步定位（2026-09-08 21:02:00 +08:00，非UE等价Gate）：MCP健康检查明确拒绝连接；纠正首次遗漏mount层的asset路径后，当次native读取37.388s完成，258 raw graphs中当前根可达62 graphs/376 nodes。两侧phase编码不一致，但不足以决定修复方向；此时按Actor合同扩充converter的方案与fixture已在下述纠正中撤回。
  - 判定纠正（2026-09-08 21:07:00 +08:00）：用户重申唯一标准为UE ALS-Refactored。01a过早勾选：两侧phase编码不同是事实，但新增fixture只验证“按Actor合同覆盖converter”的功能，不能证明Actor本身符合UE。已撤回该未提交实现、配置和fixture，未生成/安装或运行新资产。fresh UE进一步确认group按实际动画权重/role选leader，BlendSpace还按过滤后的sample权重选择内部leader；当前Actor按gait/direction/stride分支选择，必须独立验证实际采样与生命周期，不能靠改编码把两份待审查实现对齐。本项恢复未完成；继续原20:50→21:40取证窗口。
  - 已撤回的01b方案：开始2026-09-08 21:02:00 +08:00、预估结束21:42:00、实际撤回21:07:00（偏差-35m，未验收）。下一修复条目须在UE算法、资产输入和target实际消费的最小复现闭合后重新定义，不沿用上述失效方案。
  - Source读取边界（2026-09-08 21:16:00 +08:00）：正在以公开Standing uasset入口重新读取其6个WalkRun BlendSpace和相关sequence timing，输出仅到隔离目录。本地UE证明BlendSpace按过滤后实际sample权重选择内部leader并推进marker，当前Target的leader分支与phase解码均须对照这一执行结果。复用native reader增加可选、有限帧的UBlendSpace::TickAssetPlayer探针，记录输入、过滤值、sample权重/时间和marker区间；不构造UE Actor替身，也不修改任何动画或物理资产。该探针只作为此已证实source调用链的直接事实，完整AB状态机和Actor仍须后续实际验证。
  - UE执行/最小失败Gate（实际结束2026-09-08 21:36:00 +08:00，相对实际预估21:40偏差-4m）：公开Standing转换280.809s完成，62个可达graph与6个BlendSpace静态支持边界通过，全部输出仍在隔离目录。native reader新增可选最多600帧的真实TickAssetPlayer探针，默认入口自动构建77.549s，150帧读取35.205s通过；空时间线以明确错误/exit7拒绝，既有进展watchdog保留。UE第一帧由0.226666674推进至0.243333340s，gait输入切到run后仍由过滤后的walk样本领先，直至第39帧run权重0.532922。直接用此UE输出测试当前Actor时钟，稳定FAIL：233个sample时间不符，最大45.833ms。旧测试只验证零Delta初始化或手写leader选择，未与实际UE Tick比较。原生数值保存为可复跑fixture；这才是01a的源事实与失败复现Gate，不是Standing功能完成。
- [x] BF115.01b Standing时钟执行合同：开始：2026-09-08 21:36:00 +08:00；预估结束：2026-09-08 22:26:00 +08:00。以UE实际时间序列为唯一预期，验证首帧推进、过滤后sample leader、group实际权重与marker连续性；使用动画图公开扩展点确定时钟归属，消除新执行合同中Actor对动画选择的重复推算。先通过同一native fixture及最小实际graph权重/生命周期测试，再定义生成安装与可见六向Gate；不修改依赖或以旧图观感兜底。估时依据为已定位的两个执行边界、现有marker/weighted-context实现和已确认的公共temporary state能力，复杂度尚未以Actor相位配置掩盖。
  - 执行合同进展（2026-09-08 22:30:00 +08:00）：150帧native时序已对齐；实际图fixture通过BlendSpace/Sequence混合、共享缓存最大权重、marker接续及关闭同步的失败对照，325个als-ext回归通过。UE SaveCachedPose、ApplyAdditive、LayeredBoneBlend的Update边进入本项权重闭包；公开arena反射快照隔离FSM清理与缓存，保留一次正式状态提交。补充160帧native读数62.817s完成，发现等权sample排序边界：仅改first-max在第52帧仍失败；改用当次UE烘焙三角形、缓存索引与实际小数组排序后，合计310帧过滤值/权重/采样时间通过20微秒合同。正在完成消融、旧资产兼容与回归收口；尚未安装新生成图，也未通过Standing真实运动Gate。下一闭包为converter输出上述数据、移除转换图对Actor phase provider的依赖，再审计缓存重入与真实六向表现。
  - 合同Gate完成：实际结束2026-09-08 22:38:00 +08:00，偏差+12m，未超过20m容差；`cargo test -p als-ext --lib --jobs 2` 326 PASS，watchdog正常完成27.603s。310帧native采样、实际图混合/缓存/FSM、旧资产反序列化及原生三角形/缓存索引/组同步消融通过。字符串化节点标识已简化为既有context/node二元组；未新增依赖。此项只完成可复用执行合同，当前已安装转换图和Actor phase旧路径仍待01c迁移，不能据此宣称Standing功能完成；最初Actor失败fixture暂保留为待迁移记录。
- [x] BF115.01c Standing生成与消费者静态接入：开始：2026-09-08 22:38:00 +08:00；预估结束：2026-09-08 23:28:00 +08:00。估时依据为同次7份native资产记录、62个可达graph/376 nodes、4个sync group和已通过的执行合同，公开重新读取生成曾用280.809s。converter从原生BlendSpaceData生成三角形与clock配置，按source节点/组语义替换旧phase provider，迁移相应Actor/视觉读数消费者；完成生成、安装边界与规定静态Gate后，再为真实六向场景登记独立Gate。source已暴露的缓存重入、Standing输入/曲线时序须明确核对；相机剩余问题维持既定Actor集成边界，不用旧手工图或旧Actor作正确性基准。
  - 静态Gate完成：实际结束2026-09-08 23:47:00 +08:00，偏差+19m，未超过20m容差。公开uasset转换280.576s/unsupported 0；只覆盖既有AB_Als安装清单内108个Standing文件，23个文件改变，未创建重叠owner。326个als-ext、36个Standing、32个视觉读数测试与4个工具self-test通过；同入口target_game/play编译63s，23个Pose图真实spec加载2.14s，108文件编辑器修复/复查零改动，位置/运行资产边界通过。新声明式manifest通过同次7份native证据、62子图/174个源Pose节点/141条Pose边与108个安装RON逐文件编译一致性审计（含4 FSM/30条件图）；旧手工可见map不适用于编译辅助节点，skill已明确既有生成图审计路径。沿用用户已批准的不影响运行的编辑器注释省略。
  - 消费者边界：删除Actor对转换版四个同步相位的写入及无消费者的detail phase推算；视觉窗口改读本帧实际图同步组，旧Actor注入值不能充当结果。最初失败fixture已由相同native数据的实际节点测试接替。GetUp仅因共享context字段新增而补默认构造，未改已验收行为。真实Standing功能尚未验收：fresh `RefreshPose/RefreshFeet` 从AttributeCurve读取PoseGait/FootPlanted/FeetCrossing，而当前转换资产没有对应曲线采样通道；Sprint双向混合的reset-child时钟也须核对。下一项沿这些明确source边闭合，不启动运动试跑定位。
  - 第四段前复盘（2026-09-08 23:47:00 +08:00）：自20:50起已完成取证、时钟执行、静态接入三段。两次直接UE时间线推翻了Actor phase和简化等权排序；当前真正剩余边界是同一动画的pose/curve/初始化时序，不是相邻图或物理调参。旧Gate只覆盖图类/连接与独立Actor采样，未约束曲线必须来自当前实际播放器。后续以Standing内32个去重sequence资产和现有AlsPoseCurves采样能力为闭包，先读源曲线并稳定复现，再共用图内采样/权重与实际曲线投影，最后才做完整可见Gate；不新增通用审计框架或重复手工Actor状态机。
- [ ] BF115.01d Standing曲线与初始化闭环：开始：2026-09-08 23:47:00 +08:00；预估结束：2026-09-09 00:37:00 +08:00。依据为已闭合的23个Pose图/32个sequence资产、现有AlsPoseCurves采样器、两处明确source消费者与已构建的target_game入口。提取当次源AttributeCurve，建立pose/curve同时间同权重的失败fixture，修复通用转换器与Standing的Pose/Feet消费者，并核对reset-child/缓存重入；完成静态与focused Gate后登记可见六向Gate。若源读取或权重闭包超出本段估时，按同项规则复盘重估，不以Actor硬编码值或曲线默认值伪装闭合。

## 附录 A：与 ALS 的刻意差异（非缺陷）

| ALS | bevy_als | 理由 |
|---|---|---|
| AnimBP 状态机 | 显式 system 路由 + animation graph RON | ECS 数据流，无隐藏图执行 |
| GameplayTags | `bevy_gameplay_tag` 的真实层级 Tag | 保留 UE Tag identity 与 exact/hierarchical query 语义 |
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
