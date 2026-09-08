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
- [x] BF115.01d Standing曲线与直接播放器初始化：开始：2026-09-08 23:47:00 +08:00；预估结束：2026-09-09 00:37:00 +08:00。依据为已闭合的23个Pose图/32个sequence资产、现有AlsPoseCurves采样器、两处明确source消费者与已构建的target_game入口。提取当次源AttributeCurve，建立pose/curve同时间同权重的失败fixture，修复通用转换器与Standing的Pose/Feet消费者，并核对reset-child/缓存重入；完成静态与focused Gate后登记可见六向Gate。若源读取或权重闭包超出本段估时，按同项规则复盘重估，不以Actor硬编码值或曲线默认值伪装闭合。
  - 曲线source闭包（2026-09-09 00:17:00 +08:00）：32个去重sequence直接读取当前uasset；WalkForward与SprintAcceleration的fresh native分别49.393s/33.162s确认非零FootPlanted/PoseGait，后者非additive sequence。fresh SK_Als的43个曲线元数据均无LinkedBones，Standing两处分骨骼混合沿用UE默认Override；TwoWay、ApplyAdditive与ModifyCurve的C++明确同权重混合、基础加权累积、缺失值0。实际图fixture先稳定FAIL曲线缺失，修复后覆盖Sequence/Evaluator/BlendSpace同时间采样、双向/additive混合；扩展到分骨骼覆盖后再次FAIL并修复，8路输出通过。复用AlsPoseCurves与现有源曲线读取器，删除独立落脚曲线时钟与12个重复加载；Actor改读上一轮实际图曲线。RotationYawSpeed只在独立生成RON内做离线坐标转换，原始导出未改。尚未通过完整静态或runtime Gate；下一步公开重新生成、源曲线逐字段对照、reset-child/缓存重入最小验证与消费者回归。


  - 静态Gate与拆分：实际结束2026-09-09 00:45:00 +08:00，偏差+8m，未超过20m容差。源曲线涵盖32动画/249 keys（245 linear、4 cubic）；WalkForward与RotateLeft的native文本交叉核对时间、值、插值及实际生效的cubic切线，差异限于native加载后为linear段重算但不参与该段采样的切线，未放宽数值门限。实际图8路曲线组合通过；直接Sequence重新激活先FAIL旧位置0.5对源起点推进0.3，修复与关闭重置的消融均通过；改为清空既有播放器状态，删除两条伪初始化时间边。328个als-ext、4个生成输入、35个Standing测试及4个工具self-test通过，删除的旧方向测试只覆盖已移除的Actor重复采样器。
  - 生成与安装：第一次公开曲线生成273.627s通过；移除初始化时间边后第二次公开源读取/编译277.471s、unsupported 0，最终隔离安装因已有输出且未传replace标志被拒绝。随后对同次完整编译输出调用既有installer的replace路径，并确认文件集合不变；实际安装只覆盖原AB_Als owner中的108文件，10个改变，未覆盖owner清单。target_game/play最终双example编译108.797s，23个Pose图spec加载3.776s；编辑器修复/复查108文件零改动，声明式Gate2.672s通过62子图与108文件一致性、位置及运行资产边界。原始动画未重写，数据直接生成进运行RON，未新增依赖。
  - 本次拆分保留未通过边界：原“核对缓存重入”已确认UE SaveCachedPose初始化计数与relevance判断没有被当前缓存实现表达；本项完成曲线和直接播放器初始化，不宣称缓存重入或Standing功能通过。该差异转01e独立修复；尚未启动Standing startup或运动runtime。
- [x] BF115.01e Standing共享缓存与FSM初始化：开始：2026-09-09 00:48:00 +08:00；预估结束：2026-09-09 01:33:00 +08:00。估时依据为Standing 9个SaveCachedPose、38个Use、4个FSM和已通过的实际图测试基座；fresh UE仅在初始化请求且上一轮已不相关时重建缓存子图，持续相关的共享缓存保持时钟。先用实际FSM/共享缓存图复现Move→Stop连续与Idle间隔后重入，再用公开扩展能力保留正确初始化边界；不能通过只重置组相位或复用旧Actor时钟掩盖过滤器/FSM状态。完成后继续Standing Actor条件和真实可见Gate，不转入Grounded。
  - 新段前全程复盘（2026-09-09 00:48:00 +08:00）：20:50至今四段分别以UE执行推翻Actor时钟、修正原生sample排序、接入图内时钟、闭合曲线与直接初始化。原“节点类别支持+静态连接”Gate遗漏的是跨帧执行，不是地形/物理或旧图观感。剩余拆分按真实生命周期建立小图反例，再回到同一Playground；不扩大到未选相邻图，不为通过运行而调整步幅/平滑/容差。已有结果复用，缓存初始化只补一次最小能力；全Standing仍以最终实际源状态、pose/curve和Actor表现验收。
  - Source纠正（2026-09-09 01:01:00 +08:00）：重新逐项追踪SaveCachedPose后发现安装版与本地5.8源码均没有写入该节点的UpdateCounter；先前依据Initialize注释和条件式推导“间隔重入须重置”并不成立。实际FSM fixture已建立，初次失败是测试误用相同默认nil StateId，已改为不同身份，不能算行为反例。当前不实施缓存重置；复用native reader添加固定有限帧的SaveCachedPose实际Initialize/Update/PostGraphUpdate探针，核实连续、间隔、实例重新初始化三种调用，并以执行结果决定是否存在target差异。
  - 有效失败闭包（2026-09-09 01:07:00 +08:00）：native插件76.956s构建、Standing读取及九帧调用43.924s通过；共享缓存间隔重入继续0.7s，实例初始化才重回0.3s，UpdateCounter全程未更新，故删除拟议的缓存重置设计。相同实际target图的直接缓存与关闭FSM重新相关初始化两路PASS；开启该选项的嵌套FSM在第6帧仍0.7s，对UE StateMachine::Initialize→SetState→StatePoseLink.Initialize要求的0.3s稳定FAIL。四个Standing FSM均启用该选项；target遗漏入口子图初始化是本项最小根因，复用现有直接状态清理，不改缓存节点、依赖或新增反射层。


  - 本段Gate完成：实际结束2026-09-09 01:12:00 +08:00，偏差-21m。九帧native记录与实际图三路初始化边界通过；移除入口初始化的旧实现稳定FAIL，关闭重新相关初始化的对照保留0.7s；329个als-ext回归PASS（watchdog 19.818s），native reader五项watchdog/build新旧判断自测PASS。旧状态输出测试也改用两个不同状态ID，避免默认nil身份合并造成假覆盖。运行改动仅复用状态清理到入口选择，未新增缓存状态、反射层或依赖；尚未运行完整Standing场景，同入口预构建留待下一静态闭环后的runtime Gate。
  - 后续边界：fresh Standing 22个实际状态的bAlwaysResetOnEntry均为False；UE SetState只在目标已无权重或强制重置时初始化，而当前生成FSM过渡始终reset_target_state=true且运行时未按实际目标权重拦截。此源边属于同一Standing初始化闭包，转01f独立修复；Actor输入和真实Standing Gate仍未完成。
- [x] BF115.01f Standing活动状态重返合同：开始：2026-09-09 01:12:00 +08:00；预估结束：2026-09-09 01:57:00 +08:00。依据为当次Standing 4个FSM/22个实际状态、UE SetState已提取的条件、既有实际状态权重与interrupt fixture；预计复用现有SourceTransitionCondition保留目标状态初始化策略，再以实际过渡中的重返反例验证。修复通用转换与source状态策略，完成生成/静态Gate并提交后继续Actor输入；不把bAlwaysReset默认值藏在Standing专用分支，也不改变非source FSM显式重置语义。
  - focused闭包（2026-09-09 01:19:00 +08:00）：实际ProfiledBlend过渡A→B→A先稳定FAIL（A仍有权重但被重置0.3s，应继续0.6s）；源状态策略接入后，保留/强制重置/无源策略兼容三路PASS。源字段解析fixture先FAIL缺失always_reset_on_entry，再通过默认False及显式True；conduit最终目标策略与RON Some(true)逐步保留，五个converter相关self-test PASS。22个Standing状态在fresh native中均False，既有UE AnimStateNode构造也默认False。新Option字段只为明确区分源策略与旧资产既有显式重置；未新增独立状态或辅助执行层。下一步公开uasset重新读取生成、受owner限制安装、同入口编译与完整静态Gate。


  - 本段Gate完成：实际结束2026-09-09 01:28:00 +08:00，偏差-29m。公开uasset转换287.584s完成、unsupported 0、隔离安装通过；只覆盖原AB_Als owner内108文件，两张图改变且差异严格限于51条已解析的目标状态策略。330个als-ext回归PASS；target_game/play双example编译122.539s、23张Pose图实际加载3.145s；108文件编辑器修复/复查零改动，声明式Gate5.318s流程通过62个源子图、174个源Pose节点、141条Pose边、4个FSM和30个条件图，以及位置/运行资产边界。消融以强制重置和无源字段策略分别确认配置与兼容边界；未改原始导出或相邻图。
  - Standing剩余边界：图时钟、曲线与已证实初始化差异已闭合，仍须核实Initialize/RefreshStandingMovement、RefreshGroundedMovement、Pivot/HipsDirection与Rotate等已登记source函数在Actor中的算法、输入及调用阶段；然后完成真实startup和运动Gate。该函数列表来自当前转换IR/dispatch，不以插件名出现在集成报告中作为功能正确证据。
- [ ] BF115.01g Standing Actor输入与生命周期：开始：2026-09-09 01:28:00 +08:00；预估结束：2026-09-09 02:23:00 +08:00。依据为当前62个可达源子图、IR明确的运动/方向/pivot/rotate函数及现有Actor集中输入provider；前两段实际闭环分别24m与16m，本段优先比较C++函数、source数据与调用阶段，再只修复证实差异。GroundedMovement只核对Standing直接依赖的共享函数，不提前展开Grounded/Crouching图；原始曲线与已完成ragdoll维持边界。完成focused/static闭环后单独登记真实Standing运动Gate，不能把旧Actor输出或插件provider声明当作UE事实。

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
