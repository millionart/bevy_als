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

执行顺序按2026-09-09用户补充调整为 `AB_Als_Standing → AB_Als_Grounded静态接入 → AB_Als_Crouching → Grounded真实Playground验收 → AB_Als_Locomotion → AB_Als_Default → AB_Als_Layering → AB_Als_Head → CR_Als → AB_Als`。每项同时修复通用转换器及其对应Actor消费者；Grounded因消费Crouching输出，真实Gate明确后置，其余仍按自身Gate和依赖顺序提交后继续。相机剩余视角/距离问题保留在Actor集成闭包；在有新source证据前，不把它归因于已通过的ragdoll物理流程。

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
- [x] BF115.01g Standing实例写入、冲刺计时与输入类型：开始：2026-09-09 01:28:00 +08:00；预估结束：2026-09-09 02:23:00 +08:00。依据为当前62个可达源子图、IR明确的运动/方向/pivot/rotate函数及现有Actor集中输入provider；前两段实际闭环分别24m与16m，本段优先比较C++函数、source数据与调用阶段，再只修复证实差异。GroundedMovement只核对Standing直接依赖的共享函数，不提前展开Grounded/Crouching图；原始曲线与已完成ragdoll维持边界。完成focused/static闭环后单独登记真实Standing运动Gate，不能把旧Actor输出或插件provider声明当作UE事实。
  - 源状态写入闭包（2026-09-09 01:53:00 +08:00）：fresh UAlsLinkedAnimationInstance明确把Standing方法委托给父动画实例；UAlsAnimationInstance::SetHipsDirection/ResetPivot直接写共享属性，而当前target两者只转发Pose。实际setter→属性getter→curve采集小图在修正测试pin名后稳定FAIL（LeftBackward写入后仍0，应3）；补通用实例属性存储与读取，调用计数留在实例层，避免状态初始化错误重置CallFunction的Counter。UE CallFunction在Source.Update前执行OnUpdate/OnBecomeRelevant，计数只在整个实例初始化时重置。接下来验证重入/预览隔离并接入Reset/Initialize/Sprint的实际调用；Settings仅沿已证实Parent.Settings依赖读取父AB的配置引用和AIS数据，不展开父图实现。
  - fixture纠正与focused结果（2026-09-09 02:30:00 +08:00）：上述setter采集还遗漏了ModifyCurve Alpha=1，早先0值不能单独证明写入失败；补正后在禁用新setter写入的受控路径中再次稳定FAIL 0对3（27.658s），恢复写入后首次/连续/间隔重入/实例重建得到3/0/3/3，27.112s PASS。Actor另外两项修改前FAIL分别为布尔字段读到非Bool，以及高度scale取4而非上轴3；修复后与实际图七帧Standing分支测试共同PASS，后者验证闲置不推进、重入重置、pending update跳过冲刺加速、初始化覆盖旧pivot请求及同步预览不重复提交。禁用Refresh的消融保留旧0计时；五个转换器自测通过，未新增依赖或修改底层动画图库。
  - 配置事实与后续：父AB的配置引用读取71.851s、AIS读取33.881s通过，确认速度150/350/600cm/s、pivot阈值200cm/s、运动阈值150cm/s及0.1/0.2s半衰期。当前只接通图内InitializeStandingMovement、RefreshStandingMovement的计时/pivot、ResetPivot、SetHipsDirection，以及已证实Bool/上轴错误；其余曲线基转换、完整运动输入和转身/停步回调仍未验收。本轮正在生成、同入口编译与静态收口，不能据此宣称全部Actor功能已通过。


  - 超时复盘与拆分（2026-09-09 02:44:00 +08:00）：开始01:28、预估02:23、实际本轮结束02:44，偏差+21m。原“Actor输入与生命周期”总目标未完成，已暂停新实现/runtime启动回读本项、62个源子图和当前证据；漂移来自将provider字符串误当成调用合同、setter fixture两次采集错误，以及末轮发现旧Gate的轴/Bool断言沿用了待修实现。当前完成范围明确拆为本项标题所列写入/计时/输入类型；剩余不搁置，下一轮按原55m窗口的1.5倍登记开始02:44、预估04:06:30，并先拆为下列两个独立验收段。
  - 本项拆分后的Gate：公开uasset转换281.967s、unsupported 0，原owner的108文件仅三张图改变；331个als-ext与47个Standing相关测试PASS（最终project回归19.985s），同入口target_game/play编译128.543s、23个Pose图spec加载3.629s，108文件编辑器修复/复查零改动，静态流程5.548s通过源闭包/位置/运行资产边界。源函数通过真实节点更新实例属性，旧Actor的影子状态不再决定Hips写入、Pivot重置与SprintTime；未增加依赖或修改动画图库。BF102.38的Standing轴边界已按源坐标证据显式复开并通过回归；改动不涉及相邻图。尚未启动任何Standing真实runtime Gate。
  - 下一轮前全程复盘：20:50至今取证揭示的是执行语义缺失：先有Actor代替资产时钟，再有曲线/初始化遗漏，现在确认source函数被Pose转发掩盖。继续增加provider标签不能解决问题；后续每一条剩余函数先验证源数值或实际调用副作用，复用已建立的实例属性与图调用计数，不再添加第二套Actor状态推算。已通过的时钟、曲线采样、缓存和实例写入不重新探索。
- [x] BF115.01h Standing曲线数值与坐标输入：本轮开始：2026-09-09 02:44:00 +08:00；预估结束：2026-09-09 03:34:00 +08:00。原Actor轮次重估拆分第一段；以已核实AIS中的五条stride/yaw曲线、UAlsAnimationInstance::RefreshPose/GetAccelerationAmount/RefreshVelocityBlend/RefreshStandingMovement及实际输入链为边界。先取当次原曲线并验证现有采样、速度/加速度/高度/角度语义，只修已证实差异，按规则生成独立target数据并完成focused/static Gate；不研究Grounded/Crouching图。
  - 数值闭环（2026-09-09 03:10:00 +08:00）：五条fresh原曲线已读取；初次通用CurveFloat合同只支持Constant，正确拒绝yaw的CycleWithOffset，未放宽该旧合同。复用native reader与既有RichCurve字段解析，以AIS uasset为公开入口的独立工具220.431s重新生成5条target曲线，保留33个原key、cubic切线与周期外推，stride输入转换为m/s，yaw输入轴和值均离线换为runtime符号。原始导出和旧表保持不变；删除Standing三条旧曲线异步加载，运行时复用AlsFloatCurve。
  - 失败与回归：原stride key得到0.550018而非0.55；source yaw 60° key得到+58.125°而runtime应-60°；三维加速度得到前向1而非0.8；完整速度被丢弃上轴；PoseGrounded=5e-8时错误跳过除法。五项修改前共同稳定FAIL（32.030s），底层物理快照单测另FAIL（24.723s）；修复后8项focused PASS（40.216s），235个als-core和297个项目回归PASS、1个既有ignored（25.523s）。源Size2D仅约束speed；yaw/gait/真实场景方向检查仍明确投影水平分量，不把完整三维快照误用于方向对齐。BF104.02/BF102.37已仅按各自最小输入边界显式复开。旧重采样数据与三维截断即失败对照；未新增曲线采样器或依赖。
  - 当前未通过边界：本段正在同入口编译和静态收口，未启动Standing runtime；停步/转身回调仍归01i。fresh RefreshLocomotion还明确“停速保留velocity yaw”和相对旋转基座对previous velocity的补偿，当前provider/Standing差分尚未表达；这两条已证明的运动输入生命周期在完整Standing Gate前继续核对，不以本次数值PASS宣称全部Actor输入闭合，也不提前展开相邻图。
  - 本段Gate完成：实际结束2026-09-09 03:13:00 +08:00，偏差-21m。同入口target_game/play双example编译128.002s；首次spec未传绝对asset root、首次manifest命令把Python开关放在脚本位，均明确失败，修正调用后23个Pose图spec加载1.866s、完整静态流程3.458s通过，108文件编辑器修复/复查零改动，源编译闭包/位置/运行资产边界均PASS。五条新数据来自完整public uasset工具运行且已纳入当次6份native证据；图RON未改变，复用01g同次图编译。该完成仅覆盖上述曲线与三维数值，完整Standing runtime仍待输入生命周期和回调收口。
  - 剩余拆分：原01i回调段尚未开始，先按fresh source新证实的两个输入生命周期边界拆成下列01i与01j；继续原重估轮次02:44→04:06:30，分别独立Gate，不丢弃停步/转身目标。
- [x] BF115.01i Standing运动输入生命周期：开始：2026-09-09 03:13:00 +08:00；预估结束：2026-09-09 03:38:00 +08:00。依赖01h；以AAlsCharacter::RefreshLocomotion保留velocity yaw、UAlsAnimationInstance::RefreshLocomotionOnGameThread的previous velocity基座旋转补偿为闭包，核对实际Tnua基座身份/姿态和项目已有运动快照。先最小复现停速、重新移动、持续基座旋转与换基座边界，复用现有状态和公开物理数据；不展开Locomotion/Head图。估时依据为两处已证明差异、现有base entity数据与本轮25.523s回归记录。
  - 源守卫纠正（2026-09-09 03:23:00 +08:00）：依赖提交后实际开始03:14，保留原预估03:38。重新沿基座条件读取CMC构造、UE SaveBaseLocation以及fresh B_Als_Character CDO（38.517s），确认实际CharMoveComp的bIgnoreBaseRotation=True；UE据此SaveRelativeBasedMovement(..., false)，动画实例bHasRelativeRotation为false，故previous velocity旋转补偿在当前角色不会执行。此前把存在的分支当成有效差异不成立；原“持续旋转/换基座”受控路径不再新增，不改基座或物理代码。此结论只覆盖当前已证实的角色配置，不扩展到其他配置或相邻图。
  - 有效失败与最小实现：五帧实际PostUpdate快照→source provider角度用例在停止帧稳定FAIL（28.988s），应保留45°却从零速度重算0；修复后45/45/135/0/0 PASS（24.468s），覆盖停速后转视角、低于阈值、达到1cm/s重新移动。复用AlsActualMotion新增一个world yaw标量，保留实际world velocity零值；provider移除影子AlsMovementDirectionState读取，Standing同源消费者使用该角度。旧的从当前velocity每帧重算即失败消融；未新增缓存层/基座状态/依赖。当前正在项目回归、同入口预构建和静态Gate收口，完整Standing runtime仍未执行。
  - 本段Gate完成：实际结束2026-09-09 03:29:00 +08:00，相对预估03:38偏差-9m。298个项目回归PASS（17.136s），同入口target_game/play双example编译63.146s、23个Pose图加载2.597s、完整静态流程3.452s通过；108文件编辑器修复/复查零改动，源闭包/位置/运行资产边界通过，Character CDO仅用于实际基座旋转守卫且已纳入fresh证据。只有停速yaw保留进入实现，基座补偿设计因源守卫排除而删除。下一步01j实际图回调闭包，尚未启动Standing真实runtime Gate。
- [x] BF115.01j Standing状态回调与停止语义：计划开始：2026-09-09 03:38:00 +08:00；预估结束：2026-09-09 04:06:30 +08:00。原Actor轮次回调段由01i顺延至本项，依赖新的01i；以已登记的Initialize/RefreshTurnInPlace、RefreshRotateInPlace、RefreshDynamicTransitions及PlayStop/StopTransition状态回调为边界，对比真实图状态、源触发/结束阶段与现有动作通道。当前只登记，不提前实现；本段完成后再登记完整真实Standing Gate，仍不得转入Grounded。
  - 依赖提交后实际开始：2026-09-09 03:31:00 +08:00；预估结束：2026-09-09 04:06:30 +08:00。提前完成01i后复用原重估轮次终点，以当前已登记的实际图回调及既有动作通道为剩余闭包；先核实调用副作用，不沿用provider名称即功能通过的旧判断。

  - 源闭包与有效失败：当次Standing IR的28处函数依赖中，四个Lock/Plant状态为PlayStopLeft/Right的StateEntryFunction，Move entry与Idle exit调用StopTransitionAndTurnInPlaceAnimations；UE SetState明确先exit、再切状态/初始化、后entry。原State Result只保存名字并转发Pose，Actor只对影子Plant状态补发停步且重复发quick-stop。复用实际FSM/缓存/同步预览fixture：首次遗漏StateKey导入的编译失败已纠正，禁用新增通知的有效对照32.144s稳定得到空列表，而应为Exit/Entry；恢复后21.572s通过，pose逐帧输入也保持正确。
  - 实现与消融：FSM直接读取已生成State Result上的函数元数据，在实际选中状态时写入既有通知列表；项目既有dispatcher消费Left/Right/StopAll，并按UE NativePostUpdate顺序保留被停止请求阻挡的待播放项，下一帧再消费。删除影子停止侧、旧quick-stop注入及相关状态/旧断言；未新增回调注册框架或修改依赖。两条Stop sequence当次直接读取uasset时长2.333333s，与现有播放资产一致；源Blueprint参数保持0.4起点、1.5倍速、0.2s blend。新增队列测试与既有通知场景reader共4 PASS（49.378s）。
  - 共享停止边界：fresh StopMontagesWithSlot保留Montage.BlendModeOut，UE Stop对Inertialization立即置零权重并发原时长请求；旧turn辅助函数仍淡出，最小测试21.066s FAIL。已显式复开BF111.04的共享停止边界，改为移除尚未停止的条目并复用现有惯性请求通道，保留已在普通blend-out中的条目；两项修复验证22.368s PASS。另以实际arena重建复现通知编号2→2冲突（30.492s FAIL），会被现有cursor吞掉新回调；现用标准原子计数分配独立通知标识，继续复用原去重结构，预览只消耗不承载语义的标识。
  - 本轮结束与复盘（2026-09-09 04:30:00 +08:00）：实际开始03:31、预估04:06:30，状态回调子闭包Gate实际完成04:27:18，偏差+20m48s，原整体回调目标未完成。超过容差后已暂停新实现/runtime启动，回读本项、28处source依赖与三类失败；主要漂移是把state函数名当成已执行、未核对Montage停止模式，以及跨实例去重漏检，而非图连接或新增物理问题。完成范围明确缩为本项标题，余下native刷新函数另列01k自动继续；完整Standing目标不变。
  - 本项拆分后Gate：331个als-ext和298个项目回归PASS（41.335s），lifecycle self-test PASS；同入口target_game/play双example编译118.740s，23个Pose图加载3.139s，静态流程5.557s通过108文件编辑器修复/复查零改动、62个源子图及位置/运行资产边界。图RON没有改变；state函数provider标签已指向实际dispatcher。尚未启动Standing真实runtime Gate。
- [x] BF115.01k Standing原生刷新函数执行：本轮开始：2026-09-09 04:30:00 +08:00；预估结束：2026-09-09 05:23:15 +08:00。依赖01j；按上一轮实际估时窗口35m30s的1.5倍重估剩余工作，以已读取的Initialize/RefreshTurnInPlace、RefreshRotateInPlace、RefreshDynamicTransitions及其排队montage消费为唯一闭包。先用真实图分支/帧序测试核对调用阶段、bUpdatedThisFrame、ActivationDelay、FrameDelay、pending update、PlayRate与曲线缩放，再修对应通用映射和Actor消费者。复用实例属性、当前slot/通知通道和AIS配置；不新增第二套影子FSM，不展开相邻图。该闭包通过后登记完整Standing startup与运动Gate。
  - 当前源/实现闭包（2026-09-09 05:11:00 +08:00）：真实RefreshRotate小图先29.252s FAIL false/1.0，执行节点后56.808s PASS true/3.0。共享Rotate函数退出时清零、Turn函数176°选Right各有24.492s/22.121s有效失败，M5.4/M5.3只按数学边界复开。函数节点复用实例属性和帧号；InitializeTurn重入清零，三个native刷新只在实际分支且每帧一次执行，Actor的Standing独立检测已撤出。
  - 必要consumer链：native排队播放及既有PlayStop/QuickStop都指向root Transition/Turn slot，故直接核对两条辅助通路。Stop被旧slot活跃过滤器排除，修正旧测试预期后稳定FAIL；turn slot只给Pose却没有源RotationYawSpeed，不能据“已播放”宣称转身。AIS与当次10条sequence uasset确认Stop、Standing Transition、Standing Dynamic各两条为Stand Pose f0的mesh-space additive，四条Turn为普通pose；公共uasset工具0.567s生成独立target曲线/clip/时长数据，不修改原始导出。复用既有sample_pose、mesh difference和slot context直接采样，避免再建隐藏采样图；站立转身的curve scale与1.2倍clip clock分开，QuickStop资产按AIS修正。当前仍在帧序/消费fixture与编译，尚未生成新Standing树或启动真实runtime。

  - 本段Gate完成：开始04:30、预估05:23:15，Gate实际结束2026-09-09 05:43:14（偏差+19m59s）。236个als-core、331个als-ext、302个项目测试PASS（29.987s）；帧守卫禁用后24.737s稳定失败，恢复后的四项实际图/16资产测试35.815s PASS，实际arena重建也验证实例标识不会复用。隐藏turn sampler移除后直接复用已有clip采样，两个slot独立输出pose及按同一权重归一化的完整曲线；16资产包括native函数在stance变化时直接选择的六条Crouching资产，不包含Crouching图实现。
  - 生成与边界：公开uasset转换291.776s、unsupported 0；首次隔离namespace参数误为ab_als，未写真实assets，随后用同次source/IR和既有materializer按现有generated namespace重建2.671s。只允许原AB_Als owner的108文件安装，仅Standing根图及Idle图变化。target_game/play双example编译134.782s，23个Pose图spec加载3.408s，声明式Gate5.269s PASS，编辑器修复/复查零改动，62个源子图与源节点/边、位置及运行资产边界通过；montage生成和native映射自测通过。原始导出不变。完整Standing真实startup、运动与native动作Gate仍待执行。

- [x] BF115.01l Standing真实startup与方向Gate：开始：2026-09-09 05:56:00 +08:00；预估结束：2026-09-09 06:36:00 +08:00。依赖已提交的01k；以当前62个Standing源子图、23个Pose图、原生函数及16条直接montage资产闭包为边界。估时依据为此前实际入口单场景数分钟、当前同入口预构建已完成，以及首次完整生成Standing运行尚未验证的事实；先公共无驱动startup，再standing_direction。首个失败必须读取report/runner和截图，按实际状态、源输入、clip/curve与pre-rig/final骨架定位，只修证实根因。两项完成后继续Standing冲刺与停步/转向/动态过渡等实际分支；尚不进入Grounded。
  - 运行前摘要：01k的source/math/实际图fixture、生成/安装、编辑器、声明式静态与target_game/play二进制Gate均通过。未进行过本批Standing真实runtime；运行保留原Playground地形、环境、角色和相机，沿run_manual.bat启动，使用公共固定进展watchdog，不放宽视觉或性能阈值。
  - startup实际结束2026-09-09 05:57:35：公共入口19.439s PASS、6张截图，首尾人工检查为原场地正常站立姿态。18个要求骨骼全部观测，frame p95 13.274ms/max 28.525ms、eval p95 0.0717ms，每render恰1个physics step；graph_errors为空、primary_failure为空且总pass=true，watchdog未触发。下一步standing_direction，尚未把startup当作运动Gate。
  - 方向输入闭环（2026-09-09 06:11:22）：运行前逐项比较当次UE RefreshInput/RefreshLocomotion与真实场景发现，角色运动使用AlsInputOverride，但生成图provider只读取普通输入，导致覆盖移动时bHasInput/bMovingSmooth错误。最小实际provider函数测试60.917s稳定FAIL；改为与角色相同的覆盖优先选择后，六项provider测试21.032s PASS，旧选择即失败消融。未新增状态或修改运动阈值；声明式静态Gate6.607s PASS，既有图/资产/spec不变，target_game/play实际入口72.357s预构建PASS。以下开始公共方向场景。

  - 方向首次Gate失败（2026-09-09 06:12:27）：42.362s结束，manual exit91，runner已审计report；无graph error但视觉FAIL。首先在后退开始时出现local scale 1→2、层级末端512倍，首个pose链分歧位于Movement Details；另有后退转右窗口实际RightForward而预期RightBackward，留待首个骨架根因闭合后核对。性能p95 14.48ms/max27.2833ms，watchdog未触发。
  - 有限复开01d（2026-09-09 06:16:25）：当次四条Acceleration uasset均为AAT_LocalSpaceBase、Lean Pose第0帧参考，而当前16个SequencePlayer实例均additive_reference=None。01d新增curves字段打断了转换器按clip紧接additive_reference匹配的旧格式；旧静态审计也复用同一遗漏，故错误通过。只复开曲线字段与additive生成组合边界，先最小工具反例，再重新生成及真实姿态验收；不撤销曲线时钟/直接初始化的既有证据，不修改原始导出。

  - 静态修复进展（2026-09-09 06:23:00）：转换器新增含真实curves布局的引用发现/替换/幂等自测，旧匹配0.303s FAIL，兼容可选curves字段后0.305s PASS。并行核对方向场景前提发现未写aim_held；真实aim系统fixture36.042s复现Aiming请求退回ViewDirection，持续held输入后31项场景测试22.940s PASS，覆盖判定新增实际rotation mode检查。公开转换最初因隔离输出目录准备遗漏被拒绝，已补目录后重新开始；原生读取正在推进，尚未安装或重启runtime。

  - 重跑前静态Gate（2026-09-09 06:25:36）：公开转换305.718s PASS，16个SequencePlayer和5个Clip的参考增量全部转换且原始scale为1；安装仍限既有108文件，仅四个Movement Details图改变。target_game/play双example65.210s、23图spec3.107s、编辑器108文件零修复0.572s、声明式Gate3.689s全部通过，加入additive与curves组合自测。startup既有证据保留；本次只修运动additive资产和方向场景输入，以下重跑standing_direction以关闭01d有限复开及01l运动Gate。

  - 01d有限复开关闭：开始2026-09-09 06:16:25，沿用01l预估06:36，实际Gate结束06:26:39（相对窗口-9m21s）；41.105s真实重跑中视觉pass=true、faults/graph_errors/temporal_anomalies均为空，最大local scale1.0000005，后退截图姿态正常。p95 13.9674ms/max22.7051ms、eval p95 0.0757ms、每render1个physics step。additive组合根因已关闭；manual仍因RightBackward预期FAIL，01l保持未完成，下一步核对当次UE T15的自动转移条件。

  - 中性髋部自动边闭环（2026-09-09 06:35:00）：fresh Standing T15明确abs(HipsDirectionLockAmount)<0.5、FeetCrossingAmount<=0且RightBackward state weight>=1时转RightForward；当前Default曲线前提满足，实际最终RightForward与UE一致。此前把RightBackward当长期稳定态的场景预期错误，新增同源预期先25.514s FAIL，再修为RightBackward→RightForward，与已存在的LeftBackward自动边一致。稳定相位签名按源前提改为四种；同时新增从实际FSM及pose-ready读取的六态覆盖，保留左右Backward分支进入证明。数值/几何/性能阈值不变，尚待本轮场景回归和入口编译。

  - 方向最终重跑前（2026-09-09 06:37:22）：中性髋部修正后的31项场景回归21.071s、静态Gate3.462s、target_game/play入口64.565s全部PASS；只更新源前提与实际六态覆盖，生成图和运行算法未再改动。以下运行公共standing_direction，同时检查六态求值、四个稳定签名和完整视觉报告。

  - 六态reader失败与修复（2026-09-09 06:47:18）：41.458s上一轮视觉pass=true、所有方向窗口通过，仅新六态覆盖失败；读数在HlTransition时丢弃参与混合的状态。fresh T14/T15均以真实state weight为条件，BF113.03c按这一reader边界有限复开；最小过渡权重投影22.915s先FAIL（0而非两态），改读FSM已记录权重后305项项目测试21.827s PASS、静态Gate3.434s PASS。删除冗余稳定态参数，复用现有权重；报告追加actual mask。未改动画执行或资产；target_game/play入口61.756s于06:48:15构建PASS，以下运行过渡权重读数版方向Gate。

  - 权重Gate与活动链纠正（2026-09-09 06:52:20）：41.327s运行中manual exit0、全部方向覆盖、实际六态mask111111、visual pass=true且无fault/temporal异常。公共审计仍拒绝运动窗口的Stop FSM不可用；fresh Move子图只消费Movement Details，Stop FSM仅属于Stop子图，故这是未执行旁支被误列为必需证据。当前Standing运动fixture测试先18.720s FAIL，删除这一证据点后回归/入口构建中；StopQuick等已有过滤链不包含该点，其边界不变。下一步同一公共方向Gate。

  - 估时边界前复盘（2026-09-09 06:55:00）：本轮开始05:56、预估06:36，已接近20m容差，最终Gate预计越过边界，故暂停新的runtime启动并复读source/失败链。已完成输入覆盖、curves与additive组合、瞄准场景输入、中性髋部自动边和实际FSM混合权重；最新运行manual与几何均PASS，仅活动链误列Stop。删除旁支后的第二处旧长度断言也已纠正，31项回归20.051s PASS。漂移来自旧Gate自身漏检与错误前提，未扩展物理或相邻动画功能；剩余仅入口编译、公共九节点方向Gate及提交。按上一40m窗口的1.5倍登记下一轮开始06:55:00、预估结束07:55:00，立即继续同一任务，不以该窗口增加实现范围。

  - 复盘后运行前摘要（2026-09-09 06:55:45）：target_game/play入口65.183s、声明式静态Gate3.425s PASS。source Move实际九节点链、31项场景回归、已有六态权重/四向签名/几何结果均闭合；以下执行公共完整方向Gate。

  - 本项完成：第一轮05:56→预估06:36，06:55复盘时仍未完成（相对原预估+19m）；第二轮06:55→预估07:55，实际结束2026-09-09 06:57:04（偏差-57m56s）。公共方向Gate40.999s PASS、manual/validation均exit0；11窗口、19截图、九节点活动链、六态mask111111及四个同步相位签名全部通过，截图人工检查姿态正常。faults/graph_errors/temporal_anomalies均为空，frame p95 14.2605ms/max26.9682ms、eval p95 0.0793ms、每render1步物理，watchdog未触发。startup和方向闭环完成；冲刺加速混合、停步/转身/旋转及动态过渡的完整Standing动作Gate继续下一项，尚不进入Grounded。

- [x] BF115.01m Standing冲刺启动与稳定分支Gate：开始：2026-09-09 07:02:51 +08:00；预估结束：2026-09-09 07:37:51 +08:00。依赖已提交01l；当次本地RefreshStandingMovement与Standing图确认SprintTime<0.5使用前向加速度、pending update直接置0.5、停止冲刺清零，两条Sprint sequence按同一Movement同步组混合。以这两条实际播放器、同源输入和既有公共sprint场景为闭包，补齐启动/稳定分支及curve/pose观测后再运行。估时依据为现有7帧native函数fixture、已验证时钟/曲线/资产、当前入口增量编译约一分钟及方向Gate每场约41秒；不展开Grounded、相邻图或物理调整。

  - 冲刺source/观测闭环（2026-09-09 07:22:20）：fresh本地CPP确认0.5边界，native Standing两处ModifyCurve的实际输入pin分别把PoseStanding/PoseMoving写1。启动窗口只读取本帧SourceInstanceProperties及实际Movement同步相位，稳定窗口复用首次启动相位，均沿指定SequencePlayer和当前九节点移动链观察；fixture先静置1.5秒通过公共warmup。100ms采样对短源窗口的最小反例29.519s FAIL，窗口级逐帧采样后通过，稳定窗口沿用普通间隔；未改变动画、物理或图资产。编译时一处ResMut解引用错误已修正；公共视觉工具self-test0.570s PASS，正在收口项目回归与入口构建。

  - 首次冲刺运行前（2026-09-09 07:24:49）：307项项目测试24.239s、公共视觉工具自测0.570s、声明式Gate3.733s、target_game/play入口63.850s全部PASS。图与资产无变化；用实际native属性决定启动/稳定窗口，以启动时实际同步相位对齐两份姿态签名，启动窗口逐帧采样；以下运行公共sprint。

  - 首次冲刺失败与静态定位（2026-09-09 07:52:37）：18.400s首次运行manual全PASS，但pre-rig/final时序失败；首次局部跳变在约2.94秒，未见骨骼scale/segment异常。源TwoWayBlend对加速度作0..0.25→0..1映射（raw1得到4）、20/4插值后限制alpha，因此native加速度归零不等于Sprint播放器已接管；原窗口还有方向/parent元数据遗漏，已按实际参与权重修正。两条当次源sequence均0.6秒、标记Left0.016667/Right0.316667，与target一致，不修改资产。
  - 最小实际图与下一证据边界：两条真实glTF的单独播放及相互混合180帧三路43.998s PASS，最大角速率分别20.544/29.126/29.126rad/s，未复现运行中的突变；不能据此修改播放器或插值。复用既有scope数据公开真实同步玩家快照，新增读取测试41.287s及308项目回归29.744s PASS；当前静态Gate3.720s PASS。下一次有限公共运行只补上首次报告缺失的实际play rate、stride、gait、玩家weight和pose采样时间，以定位完整Actor输入中的首个source差异；动画/物理算法仍未改动。

  - 补证据运行前（2026-09-09 07:54:23）：同步snapshot读取已通过真实scope测试，308项目回归和静态Gate通过，target_game/play入口110.205s构建PASS。以下同一公共sprint仅修正窗口到真实参与玩家，并记录源公式所需的实际输入与采样时间；不把静态三路连续性当作完整Actor行为已通过。

  - 超时复盘（2026-09-09 07:57:56）：本轮07:02:51开始、预估07:37:51，至07:57:56仍未通过（偏差+20m05s），已暂停新的实现/runtime并回读上述source闭包与两次报告。投入包含窗口/短时采样、真实同步reader、两条实际clip三路连续性和完整输入采集；纯播放器/混合连续，完整场景仍有跳变。第二次18.419s报告已排除旧元数据遗漏，暴露实际play rate首次3.0→约1.08、原生观测在约0.25秒内缺失，以及后续速度降到2.4/1.8后重新冲刺，B玩家尚未清零时已减速，故不能伪称稳定分支通过。漂移来自原场景未约束实际运动前提、首份报告缺少输入时间线，以及对纯clip问题的未证实猜测；没有source依据不改播放器、Rig或物理。按35m窗口1.5倍，登记下一轮开始07:57:56、预估结束08:50:26，沿同一问题自动继续：先定位观测缺口对应的实际Actor/FSM状态，再逐字段对照源公式与消费者；不扩大至相邻图。

  - 缺口取证边界修复（2026-09-09 08:20:38）：发现原trace同时以当前Gait和native写入帧过滤，恰好删掉PoseMoving变0的区间；旧“Grounded”文字来自PreUpdate，不能用于证明同帧实际状态。最小reader反例31.522s FAIL（旧值被丢弃），现保留旧值并标明updated=false，窗口仍只接受本帧值；新增实际PostUpdate locomotion/gait/standing/grounded状态、actor高度和竖直速度。33项场景回归22.648s、静态Gate3.715s PASS。场地同一点静态核对只覆盖原20x20 Floor，未找到该位置的障碍物，不因猜测改地形/物理；入口构建后只补齐这个缺失区间。

  - 完整状态取证运行前（2026-09-09 08:21:39）：同入口64.837s构建PASS，以上focused/静态闭环通过。以下公共sprint保留所有观测帧及真实状态，动画算法和场地未改；先以缺口对应状态判定剩余source链，再进行任何功能修复。

  - 第二轮超时/拆分复盘（2026-09-09 09:14:19）：第二轮开始07:57:56、预估08:50:26，至09:14:19仍未完成（偏差+23m53s）。完整状态证明首次跳变始终Grounded/Move、actor高度约0.924，纯clip与完整Standalone Standing树均通过，故撤销离地、方向基和播放器算法猜测。fresh UE LinkedInputPose复制Curve/Attributes的事实与双输入fixture33.434s失败闭合了真正差异：target事后合并未选输入元数据。修复后22.156s通过；两次借用编译错误已纠正。已暂停新的实现/runtime并回读全过程；剩余拆为下列两项，按上一52m30s窗口的1.5倍推导总终点10:33:04，单项均不超过60分钟。Grounded算法、Rig、物理不展开；并消融移出已完成定位而无失败边界的两组探索性实际clip/整树测试，只保留旧实现会失败的输入选择/混合回归。
  - [x] BF115.01m1 共享linked-input context回归：开始09:14:19、预估结束09:44:19、实际结束09:26:54（偏差-17m25s）。IO在实际Pose输入求值后同步复制其context，SourceLinkedInputPose沿该边持有曲线、相关资产和同步玩家，复用现有node state；旧事后输入合并保留兼容。双输入选择/混合/重入fixture先FAIL后PASS，移除新桥接即失败；als-ext 332项1.353s、项目308项44.417s、声明式静态3.732s、同入口target_game/play构建110.532s均PASS。没有改变UE图、物理或运动公式；BF111.05有限复开随01m2真实姿态Gate关闭。
  - [x] BF115.01m2 原场地连续冲刺验收：开始09:28:00、预估结束10:16:45。依赖01m1；提前开始并保留复盘后的48m45s窗口。实际原路径后段进入坡道、速度降为Walking/Running，不能完成稳定玩家收敛。先按原地图Floor与障碍bounds验证完整平地路径，修正唯一fixture前提，再公共sprint验证启动/稳定、相位签名、curve/pose和性能；不改地形或运动算法。
    - 平地前提闭环（2026-09-09 09:33:00）：按原地图bounds与既有ue_point_to_bevy基转换检查完整55.25m行程和0.35m胶囊余量，原路线28.990s稳定FAIL于BP_SimpleObject27；修改Sprint唯一入口起点为原场地(-45,-26)、朝+X，34项场景回归21.304s PASS。复用现有旋转响应复位与显式fixture时序边界；地图、Actor运动公式、动画资产不变。以下同入口构建与声明式Gate后先startup，再公共sprint。
    - 运行前摘要（2026-09-09 09:34:30）：场景回归与linked-input调用方闭环全部通过，声明式静态3.707s、target_game/play入口构建通过。此次通用context修复后先经公共startup排除静止首帧回归，再运行平地sprint；保留原地形、角色和相机，所有几何/时序/性能阈值不变。
    - startup完成2026-09-09 09:35:13：20.184s PASS，manual/validation exit0、6截图、18骨骼，faults/graph_errors/temporal_anomalies为空；frame p95 14.5016ms/max26.2835ms、eval p95 0.0838ms、每render1步物理。以下同一入口二进制运行公共平地sprint，冲刺尚待验收。
    - 平地冲刺失败2026-09-09 09:35:58：17.878s结束，manual exit0、启动native(0.0914,0.999835)/稳定(2.93966,0)、两条窗口均进入，3秒后持续6.5m/s，原路线前提已修复。视觉exit1：启动附近phase elapsed1.9186至2.1477仍丢失PoseMoving/同步context，随后多处pre-rig/final跳变；actor始终Grounded/Move且高度约0.924，p95 14.5508ms，无图错误。m1双输入bug已由反例证明，但它尚未消除完整链的这个缺口，不能关闭BF111.05或父项；下一步只读取现有报告/实际调用链定位剩余首个context差异，禁止猜测Rig或重复runtime。
    - 根图缺口静态取证（2026-09-09 09:47:00）：固定同源输入的完整实际AB_Als树300帧23.665s PASS；首次fixture缺少locomotion_action，补齐实际Actor六种Tag前提后通过，不能把该结果当作Actor slot输入已验证。现有报告只在同步相位可用时记录pose链，缺口内缺少各root节点与Slot实际权重；新增这些直接node-state/Actor读取，34项场景回归23.164s PASS（一次缓存字段编译错误已纠正）。没有更改算法或资产；静态/同入口构建后只补这个已证实缺失的证据边界。
    - 根图补证据运行前2026-09-09 09:48:43：同入口63.939s、声明式静态3.724s PASS，已有固定输入实际根图与34项场景回归通过；公共平地sprint只新增各root节点的当帧PoseMoving/sync玩家和Slot权重/实际动作，不改变执行行为或放宽门槛。
    - 首个根图边界与最小反例（2026-09-09 10:00:13）：18.668s补证据报告确认缺口从Standing输出开始，所有后续节点照传空context，两处Slot权重始终0、实际transition playback为空；排除Slot/相邻root consumer。fresh UE AnimNode_StateMachine明确标准过渡按1-alpha/alpha混合Curve及Attributes，惯性过渡求值NextState；target ProfiledBlendNode只输出Pose/显式curve_names，生成过渡curve_names为空且FSM input只取Pose。既有实际A→B→A fixture补同值曲线后19.751s稳定FAIL于frame2（None而非1）；首次fixture pin拼写错误的82.849s结果不计入行为证据。BF113.03c按这个已证实的通用FSM过渡context边界有限复开：沿实际input pin传递动态context，保留中断snapshot的对应context，普通混合沿alpha、惯性过渡沿目标；不修改图拓扑、时钟或既有状态重置策略。
    - FSM静态修复/消融（2026-09-09 10:16:44）：按input pin复用既有context通路，ProfiledBlend使用实际alpha混合动态曲线/同步玩家，惯性过渡携带目标context，中断时Pose与context一同缓存；无独立影子状态机。原反例34.153s PASS，扩展为两种过渡/两个中断时点/三种重置策略及不同值曲线后25.496s PASS。合并输入身份17.930s FAIL（0.75而非0.25），去掉snapshot context 16.584s FAIL（0而非1），两者恢复；修正过一次getter类型、测试sync字段及把当前state weight误作历史Pose权重的测试预期。删除无失败边界的154行固定输入根图探索测试和临时root/Slot读数，只保留实际会拒绝旧实现的回归；下一步调用方回归、同入口构建/静态和公共sprint。
    - 调用方回归闭环2026-09-09 10:18:49：恢复消融后的als-ext 332项16.862s、项目309项65.429s、声明式静态3.723s全部PASS；生成图和资产不变。正在同一target_game/play入口构建，之后只执行startup与当前sprint，01m2及两处有限复开尚待真实姿态Gate。
    - FSM修复运行前2026-09-09 10:22:00：同入口target_game/play构建35.526s PASS（等待artifact锁后完成依赖检查），已完成332/309回归、输入身份/中断快照消融与声明式Gate。以下公共startup→sprint复查实际Pose/Curve/sync链，所有既有阈值保持。
    - 修复后startup完成2026-09-09 10:23:00：18.893s PASS、manual/validation exit0、6截图正常原场地站姿，无fault/graph error/temporal anomaly；frame p95 14.4323ms/max17.1932ms、eval p95 0.0832ms、每render1步物理。以下同一二进制公共sprint验证启动过渡与稳定分支。
    - 修复后sprint结果2026-09-09 10:24:08：17.868s结束，manual exit0，两分支/两同步相位签名与十节点链均PASS；phase1.75秒后无PoseMoving/sync缺口，3秒后持续6.5m/s，所有pre-rig时序异常消失，证明FSM context根因修复。frame p95 14.1191ms/max26.6785ms、eval p95 0.0852ms、每render1步物理。视觉仍exit1：仅final右calf/foot/thigh三次局部旋转突变（3.0938/3.1744秒）；尚不能关闭完整01m2。接下来只核对这段source/target脚部输入与最终写回链，不因敏感性猜测改Rig或阈值。
    - 第四轮前全过程复盘（2026-09-09 10:33:00）：当前m2开始09:28、预估10:16:45，到本次复盘尚未通过（偏差+16m15s），剩余source/focused/编译/runtime预计越过20分钟容差，故先暂停实现和新runtime，回读三轮全部证据。前期漂移主要来自把单独clip/固定输入整树的通过误作实际启动覆盖，以及未在第一份trace保留空context帧；完整链追踪最终分离了已修复的linked input、FSM动态context和原场地路线三个边界。最新真实报告已消除曲线/sync/pre-rig故障，剩余仅final右腿三次突变。无健康持久Editor，fresh本地CR_Als native读取75.826s完成；ApplyFootIk中CalculatePoleVector→膝位置加0.4m pole→DamperExact(0.05)→BasicIK(PoleVectorKind Location)，本地AlsRigUnits.cpp用Hierarchy.GetGlobalTransform，属于Rig组件空间；target SourcePoleState却缓存世界位置，角色移动被混入平滑。只有限复开BF111.03的此Pole空间边界，不展开其余Rig或物理。按上一m2估时窗口1.5倍拆为以下独立Gate，自动继续同一冲刺目标；保留原阈值。
      - [x] BF115.01m2-R4a Pole组件空间闭环：开始2026-09-09 10:33:00、预估结束11:08:00、实际结束10:41:29（偏差-26m31s）。真实缓存的平移/旋转/缩放反例31.271s FAIL于frame1；现用既有World↔Armature坐标工具在组件空间缓存/阻尼，输出时回到世界空间，12项foot-IK回归23.668s、310项目回归1.875s PASS，旧世界缓存即失败消融。删除重复pole方向查询，直接使用当帧已记录世界pole位置。fresh native Rig 75.826s；旧全Rig审计仅因新导出器新增2198处嵌套fields而拒绝旧审计资料，当次离线重生成审计profile后133 nodes/1407 pins/160 links审计0.536s PASS，实际runtime profile逐字段不变、原始导出不改；声明式Gate加入该直接依赖并4.237s PASS。尚待R4b真实姿态复验。
      - [x] BF115.01m2-R4b 冲刺最终真实Gate：开始2026-09-09 10:41:29、预估结束11:19:36，提前开始并保留上一拆分终段窗口。依赖已通过R4a，同入口编译、声明式Gate和公共冲刺，检查剩余final异常与已通过的全部动态合同；完成后关闭父项及本轮有限复开并提交。
        - 运行前2026-09-09 10:42:47：R4a已提交，target_game/play实际入口66.972s构建PASS、声明式4.237s及310项目测试通过。既有startup与pre-rig/context/两分支合同保留；以下原场地公共sprint只验证Pole坐标修复是否消除最后三处final突变，未修改动画/物理/性能阈值。

        - 最终完成2026-09-09 10:43:42：公共sprint 18.093s PASS、manual/validation exit0、issues为空；224启动样本/50稳定样本、两canonical相位距0/0.009953、完整十节点链通过，9张截图中启动/稳定姿态人工检查正常。PoseMoving/sync缺口为0，faults/graph_errors/temporal_anomalies均为空；frame p95 14.3662ms/max27.9883ms、eval p95 0.0895ms、每render1步物理，watchdog未触发。R4b实际比11:19:36预估早35m54s，R4a已单独提交；BF111.03 Pole空间、BF111.05 linked input和BF113.03c FSM context有限复开均关闭。父01m第一轮07:02:51→07:37:51未完成，两次超时复盘及第四轮前拆分已记录；最终10:43:42完成（对首轮预估+3h05m51s）。主要改进是先验证源动态context和正确坐标空间，而非仅看播放器时间/静态姿态；Standing其他动作仍未宣称全部完成。

- [x] BF115.01n 方向切换动态混合定位：用户2026-09-09明确选为Pole修复后的下一项，优先于尚未开展的Standing停步/转身Gate。开始2026-09-09 10:44:00、预估结束11:34:00（Pole提前完成后立即开始）。用户实测前后左右切换时视觉近乎静止、混合不自然，手工版症状较弱；现有截图与窗口姿态跨度Gate未能覆盖切换期间连续时间推进。按BlendSpace播放/混合权重、六向FSM及按键到图输入三条链记录连续frame/pose/phase/weight/input，手工版只作现象对照，唯一正确性依据仍为fresh UE source。估时依据为已有62个源子图、6状态/6向BlendSpace合同及公共方向场景，先动态最小复现并定位首个source差异，再决定必要修复；此登记不提前展开实现或复开其他完成项。
  - 三链source/观测闭环2026-09-09 11:05:00：当次Standing native37.874s及6个BlendSpace native全部完成；六向资产均为零步幅静态Pose/完整步幅周期四点，X/Y Cubic窗口0.2/0.4、额外weight interpolation为0、循环开启，与现有目标参数相同。fresh Input_OnMove按视角基组合XY并ClampMagnitude012D；实际键资源→EnhancedInput→AlsInputCommand保持/连按/四向/释放fixture27.623s PASS，尚未证明Actor运动或动画混合无问题。公共方向场景新增不以settled过滤的逐帧input/override/keys、速度、源标量、实际FSM权重/中断snapshot、参与BlendSpace位置/phase/sample-time及root/子姿态腿部变换；reader与35场景回归38.129s PASS（首次JSON宏数组表达式错误已修正）。未改输入、FSM或播放算法；以下同入口构建/静态后只运行一次完整动态取证。
  - 动态取证运行前2026-09-09 11:07:49：真实target_game/play入口143.107s、声明式7.411s PASS；输入/reader反例和源六向静态前提已闭合。以下公共standing_direction记录整个切换区间，已有startup/pole Gate保留，不把旧稳定窗口的通过当作本问题已解决。
  - 动态根因与01b有限复开2026-09-09 11:21:40：42.878s取证包含2455帧，所有输入/六态/骨骼与时序判定通过，公共Gate仅因重型trace的一帧95.676ms未过（p95 15.550ms）；未放宽阈值。用户症状在后退转右/后退转左/左右切换确切复现：角色3.6–3.75m/s且BS输入(1,1)，目标BS权重已0.98，采样时间却连续停在0.292959，腿部角速度可降至约0.07rad/s，而稳定时中位约5rad/s。按键保持测试排除输入丢失，静态端点排除，停滞伴随FSM中断快照。fresh AnimSync.cpp137–148/238–243明确保存并延续同步组自己的上帧结束位置；target scope把含旧snapshot phase的pose context当下帧group clock。既有FSM fixture增加真实sync scope后26.947s稳定FAIL：frame5组相位0.35，应0.4。01b只按“中断Pose context与组时钟反馈的组合”有限复开；旧独立时钟测试未覆盖此组合。先在既有scope输出中保留本帧已解析组相位，复用状态，不改输入、资产、六态选择或阈值。
  - 修复静态闭环2026-09-09 11:31:58：source scope现把已解析的本帧组相位写入既有输出context，阻止旧Pose快照相位成为下一帧时钟；无新状态、图节点或Actor补偿。13组合的实际FSM/sync反例20.572s PASS（删除新增写回即回到0.35失败），332底层28.367s、311项目45.910s PASS。移除已完成定位的逐BlendSpace骨骼副本，保留每帧root腿部姿态、实际采样时间、输入与权重，降低报告复制开销；新增当次Standing及六BS证据。以下同入口构建/静态与一次连续方向复验，尚未把静态通过当作用户症状已解决。
  - 修复后运行前2026-09-09 11:35:14：target_game/play入口114.766s、声明式4.748s PASS；已有组合反例、332/311回归与fresh六向source闭环通过。以下公共standing_direction连续复验同一输入/状态序列，比较冻结段的实际sample-time推进和腿部运动，全部原有时序/骨架/性能阈值保留。
  - 本项及01b有限复开完成：实际结束2026-09-09 11:36:24，相对11:34预估+2m24s。42.886s公共方向Gate PASS、manual/validation exit0、2379帧连续trace、11窗口/19截图，issues/faults/graph_errors/temporal_anomalies为空；frame p95 16.0954ms/max69.5903ms、eval p95 0.0919ms、每render1步物理。四个典型切换首秒的主导播放器停滞帧由15/17/36/22变为全部0，root腿部角速度中位分别由0.921/1.293/0.776/1.213升至3.535/3.867/2.223/4.205rad/s；这项连续证据补足截图无法证明的播放推进。已确认实际按键/BS参数/六态选择保持源行为，仅修组相位反馈；移除重复子姿态日志后性能门槛也通过。下一步恢复Standing尚未开展的停步/转身/旋转/动态过渡Gate，不把本项当作整个Standing已完成。

- [x] BF115.01o 连续换向与停步恢复：开始2026-09-09 11:40:00、预估结束12:20:00。用户随后要求覆盖“左跑两秒立刻反向、持续WASD循环、松键后未恢复正常站姿”，本项优先复现该连续输入再核对Standing停步/quick-stop直接闭包。01n只证明同步时钟停滞已消除，不能作为所有滑步/延迟/停后姿态均已解决的结论。以当前62个Standing源子图、已闭合的回调/原生刷新/16条montage资产和现有公共场景为基础，记录实际state/clip/weight/lifecycle/Pose，再按首个source差异修复；不启动Crouching或其余图工作。估时仍按现有source/场景框架、增量构建与单场记录，出现额外根因则按规则复盘拆分。
  - 新复现入口静态准备2026-09-09 12:00:00：按用户节奏新增公共standing_response：静置1.5秒、左2秒、无释放间隔切右2秒、W/A/S/D每0.25秒循环三轮、松键4秒，使用默认ViewDirection并保留原Playground。逐帧复用现有input/BS/FSM/root-Pose trace，追加Stop FSM、实际transition与脚部目标；末段单独采集idle-after姿态，并要求真实Standing Idle、PoseMoving=0及transition inactive。fresh PlayQuickStopAnimation确认非VelocityDirection固定左分支，源原生动作/图算法尚未改动；原35场景回归31.843s PASS，新增时序fixture验证2秒反转与释放阶段中。
  - 时序fixture与36场景回归2026-09-09 12:03:00通过（24.490s）；末段idle-after独立窗口确保记录等待结束的最终姿态。当前只新增复现与观测，未按猜测调整FSM中断或停止算法；同入口构建与静态后执行一次用户时序场景。
  - 用户时序运行前2026-09-09 12:05:04：target_game/play入口68.583s、声明式4.525s PASS，36项场景/时序fixture通过。以下公共standing_response完整记录2秒反转、快速循环和松键恢复，不使用此前3秒稳定方向窗口替代本次问题。
  - 用户时序首次结果2026-09-09 12:05:55：21.034s结束，838帧；快速循环期间多次中断snapshot，松键后真实Standing已Idle且速度0/PoseMoving0，但StandingDynamic从约9.1秒至结束反复回到clip_time约0.02，右脚未恢复正常落地；9.738秒有pre-rig calf_r突变。原response_idle OR式累计恰好撞到短暂inactive帧而误报，已改末段每帧持续满足；因此本问题未通过。fresh源FrameDelay=2、AllowTransitions全权重门控与资产负一增量曲线均已核对，尚未证实是曲线链还是执行阶段；追加本帧root曲线、provider门控、slot曲线/weight及native delay/queue frame，36项回归27.375s、静态4.246s PASS。使用已有UE TraceScriptDriver在原地图运行同一输入CSV与0.05秒骨骼记录，固定150/60秒watchdog；该source对照与随后target运行顺序执行，避免同时渲染干扰性能。
  - 补边界运行前2026-09-09 12:24:11：target同入口66.057s构建、36项回归及声明式Gate通过；UE同输入原场地对照已自动结束。以下target standing_response只补齐现有重播区间的门控/帧序，未改变动画行为；末段持续Idle判定已强化。
  - 首轮到界前复盘2026-09-09 12:34:00：开始11:40、预估12:20，目前未完成（偏差+14分钟）；剩余修复/回归及快速循环问题预计越过20分钟容差，暂停实现与新runtime并回读证据。21.320s补证据运行准确拒绝idle恢复；slot负一曲线与全权重门控正常，实际动画在约0.02秒后被request_stop，权重淡出结束再重启。逐一追踪所有request_stop调用后确认crouching_layer_writer在Standing且无CrouchingLayerRuntime时仍逐帧停止共享standing动画；fresh Crouching native35.395s表明源仅在Idle退出/Move进入事件调用停止，未有不相关时逐帧停止。先有限复开BF111.04的跨stance清理边界，保留已闭合curve/同步证据；再处理快速循环的连续姿态，不把一处时钟修复等同于全部响应完成。投入主要为补足用户实际节奏、纠正瞬时idle假通过、区分曲线门控与真正stop调用，未改物理/阈值。按上一估时窗口1.5倍拆为下面两项并自动继续。
    - [x] BF115.01o1 站立动画被Crouching清理打断：开始2026-09-09 12:34:00、预估结束13:04:00、实际结束12:45:36（偏差-18m24s）。最小清理事件反例、仅退出时清理、消融/回归/真实停后恢复Gate通过；不展开Crouching动作图。
    - [x] BF115.01o2 快速反向/循环连续姿态：开始2026-09-09 12:48:00、预估结束13:18:00。o1提前结束后按原终段窗口提前开始；沿用户2秒反转和0.25秒循环记录，对比source中断混合与实际root姿态推进，补足必要修复与动态Gate；正常停步/quick-stop剩余边界保持在此Standing闭包内。
    - o1 source/最小修复闭环2026-09-09 12:39:43：fresh Crouching35.395s明确Idle StateExit/Move StateEntry两处停止调用。实际清理路径fixture27.340s先FAIL“inactive crouching stopped a standing montage”（首次测试调用私有构造函数的编译错误已改用公开SourceTurnRequest）；现在仅曾有Crouching runtime的退出执行共享停止/移除，保持Standing动态动画和turn stack，19项Crouching回归17.125s PASS，去掉事件守卫即复现失败。未改变Crouching图、停止时长、蒙太奇或输入；正在完整项目回归与同入口静态/编译后复验停后恢复。
    - o1运行前2026-09-09 12:44:44：313项目回归1.882s、声明式4.236s、实际target_game/play构建66.290s PASS。UE同输入对照60.217s自动结束，其有效移动记录在松键后速度0/PoseMoving0，末段两足组件高度约13.4cm恢复平齐；target旧记录右足仍抬起。以下同一standing_response验证清理事件修复和末段持续idle，不改其余行为。
    - o1最终结果2026-09-09 12:45:36：21.327s公共standing_response PASS，manual/validation exit0，末段持续idle与无活动transition通过；动态动画完整推进至1.533333秒并结束，不再约0.02秒中止/重启。末段双足世界高度0.13796/0.13840m并已人工确认正常落地站姿；faults/graph_errors/temporal_anomalies为空，frame p95 16.1489ms/max31.3982ms、eval p95 0.0857ms。BF111.04清理事件有限复开关闭；快速循环中的主观响应问题仍保留给o2，不以本次停止恢复通过替代。
    - o2源中断姿态差异2026-09-09 12:57:49：fresh本地AnimNode_StateMachine.cpp988–1020逐帧遍历ActiveTransitionArray，1045–1058将本帧前一活动过渡的输出作为下一过渡输入；target activate_transition却缓存last_pose并在随后Source输入返回历史Pose。既有13组合fixture给两侧加入同一同步线性位移动画后32.899s稳定FAIL：frame5组相位正确0.4，但Fsm输出位置仍0.7，当前两侧动画均应0.8（首次漏导入宏的9.249s编译错误不计行为证据）。因此01b组时钟修复保持关闭，BF113.03c仅按“中断后活动源姿态的持续求值”有限复开；需要保留source活动过渡/状态时间及profile行为，不能用减小混合时间、Actor补偿或丢弃旧状态掩盖。

    - o2活动过渡静态闭环2026-09-09 13:14:35：复用interrupted_sources按源顺序逐帧求值，Pose/context及状态权重一起推进，并按UE604–611删除已结束过渡及其更早历史；删除旧冻结Pose时间分支和权重快照。原13组合反例29.013s PASS；新增同边重入后27.255s稳定FAIL于frame6曲线0.65而非0.49，现仅对同时存活实例使用可复用求值槽，26.795s PASS。扩大至两种混合、26次快速切换的15组合，验证当前Pose/曲线、旧状态重置策略、历史长度、求值槽数量恒定及结束清理；332项als-ext回归17.592s PASS。长程fixture的累积f32误差约1e-6米，使用1e-5米数值比较精度，未改变任何runtime Gate阈值。移除历史清理20.519s稳定FAIL于历史长度，已恢复；调用方/真实Gate尚待完成。

    - o2本轮复盘与重估2026-09-09 13:33:00：本轮开始12:48、预估13:18，实际到本次复盘13:33尚未通过完整Gate（偏差+15分钟）；已完成当前帧活动源、状态权重、同边重入和历史回收修复，645项调用方回归65.054s、声明式4.210s通过，去掉活动源权重37.527s稳定FAIL于0.75而非0.35，恢复后15组合26.462s通过。继续核对源入口发现同一直接链路尚有时间差异：UE constructor56–91按目标已有权重和Hermite逆alpha缩短时长，Update125–143及527–529在创建当帧推进；target仍固定timed并在本帧从0开始。自动remaining-time还在850–854记录越界时间并从原时长扣除。现暂停实现与新runtime，回读本项及上述完整创建→更新→求值→清理闭包；原修复不是无效，但旧fixture时长预期不足以证明源时间语义。下一轮开始13:33、预估结束14:18（按上一窗口1.5倍），剩余为源时间最小反例、标准过渡有效时长/同帧推进、适度回归和同入口真实Gate；自动继续当前Standing任务，不改输入、Actor、动画资产或验收阈值。

    - o2同一时间/惯性闭包补充2026-09-09 13:40:00：源时间反例19.162s稳定FAIL，frame2标准过渡曲线仍0.25而非UE当帧推进后的0.35。fresh Standing还确认Movement Details的Run→First Pivot、First/Second Pivot→Run与Switch Pivot均直接使用惯性过渡；UE70/1374–1396将FSM过渡时长置零并向最近Inertialization发原始时长请求。target生成器却将它映射为duration/4半衰期混合，生成的两处InertializationPassthrough因request_epoch_key为空直接透传，因此仅修标准时间不足以关闭用户换向症状。本轮只扩入上述已证实的FSM→惯性请求→现有惯性节点链及其必要context传播，不研究其他图行为；保留此前正常turn请求通路，复用现有source_inertial_float算法。

    - o2完整时间/请求静态闭环2026-09-09 14:28:00：当前62源子图闭包补全UE创建→同帧规则循环→时钟推进→活动源求值→清理→最近惯性节点链。标准过渡使用目标既有权重、Hermite逆alpha、自动触发超出量和当前帧delta；自动规则改读当前状态自身播放器，实际组合17.321s先FAIL于frame3的0.45而非0.25。惯性FSM立即结束，原时长请求进入当前帧context并由最近惯性节点消费；Pose请求20.204s、曲线21.293s、反复中断deficit19.494s分别先FAIL后修复，331项扩展回归25.672s通过。三态同帧链另证明结束清理不能消耗新过渡上限（32.865s FAIL），0.5/0.2/0.4同帧请求须取0.2（19.846s FAIL，位置0.46835935而非0.48125），现已修正并在最终调用方回归中验证。移除旧指数残差/半衰期与对应两项非源测试，新增源时长和请求合并检查。生成器无半衰期反例0.300s FAIL；以当前生成器重生成23处惯性过渡，并更新6处旧图同一构造参数，逐字差异只允许移除该废弃字段；其他图仅作共享节点schema兼容，不展开其行为。直接重渲整个Standing根图会遗漏既有additive后处理，差异守卫在写入前拒绝，故删除这一步，只重生成受影响过渡；0.550s迁移PASS。原始导出/动画资源/输入/Actor和runtime阈值均未改，仍待同入口编译、fresh spec/声明式及真实Gate。

    - 第四轮前全过程复盘2026-09-09 14:37:00：上一轮13:33→14:18到本次复盘仍未完整验收（偏差+19分钟），继续runtime与差异审查将越过20分钟容差，先暂停实现和新启动，回读o全程、当前source闭包、diff及实际工具报告。第一轮定位并修复跨stance共享停止，o1真实恢复已通过；第二轮真实位移动画暴露冻结历史Pose；第三轮追到完整FSM创建/更新与惯性consumer，消除了原先沿用目标固定时长、指数混合与无请求透传的假前提。漂移主因是最初把“同步时间前进”当作姿态连续性的充分条件，以及没有一次覆盖创建→求值→消费全生命周期；纠正后15组合三态动态fixture验证真实几何、曲线、重置、自动资产、同帧上限/清理与最短请求。当前645项回归70.378s、target_game/play两个实际入口构建123.838s、fresh spec2.389s、声明式4.753s均PASS；spec首次3.436s仅因未注入绝对asset root失败，纠正启动参数后通过，未修改资产路径。没有新的未定位静态故障；下一步只按现有原场地公共Gate与连续trace验收，失败时先读primary/source链，禁止以阈值/补偿掩盖。按上一窗口1.5倍重构为以下两个可独立验收的剩余项，自动继续同一完整任务。
      - [x] BF115.01o2-R4a 启动及用户节奏真实验收：开始2026-09-09 14:37:00、预估结束15:10:00。依赖全部通过的source/static/编译，执行startup与standing_response；检查原场地可见姿态、2秒立即反向、0.25秒循环全帧运动、松键持续Idle及严格temporal/性能Guard。通过后提交此次实现。
      - [x] BF115.01o2-R4b 同一改动的方向/冲刺回归与闭环：计划开始2026-09-09 15:10:00、预估结束15:44:30。R4a通过后同一二进制执行既有方向/冲刺场景并审查差异，关闭o2及有限复开，记录实际时间；不把它作为Standing剩余转身/停步动作已完成的证据。
    - R4a运行前2026-09-09 14:37:00：62源子图及FSM→最近惯性节点闭包已静态闭合，fresh spec和声明式通过，真实入口二进制已完成；以下先无驱动startup，再同一用户时序standing_response，所有既有运行阈值不变。

    - R4a startup结果/response运行前2026-09-09 14:40:51：startup manual/validation exit0、faults/graph_errors/temporal_anomalies为空，6张截图保留正常原场地站姿；frame p95 14.9788ms/max27.5956ms、eval p95 0.0870ms。以下同一二进制standing_response完整复验用户输入节奏和持续idle，继续检查所有帧而非仅稳定截图。

    - R4a完成2026-09-09 14:44:20：开始14:37、预估15:10、实际结束14:44:20（偏差-25m40s）。startup及standing_response均manual/validation exit0；response完整902帧，反向/循环/持续idle全部覆盖，faults/graph_errors/temporal_anomalies为空。快速循环区间6腿局部角速度RMS中位1.9408→4.3374rad/s、P10 0.4095→2.6418rad/s，相同0.3rad/s观测口径的低运动采样9/190→0/175；这用于连续运动对比，不改变任何Gate阈值。立即反向区间中位4.3534→4.5616rad/s；末段两足目标高度0.137941/0.137945m，动态动画完整结束，已查看循环与最终站姿截图。frame p95 17.8863ms/max33.1968ms、eval p95 0.0917ms。实现通过本项Gate并单独提交；R4b提前开始14:44:20、预估结束15:18:50，剩余既有方向/冲刺的同一二进制回归。

    - R4b运行前2026-09-09 14:47:40：R4a实现已提交7888369，startup及用户连续输入/停止恢复通过；同一target_game/play二进制与fresh spec保持不变。以下既有standing_direction检查六向及不同view方向，随后sprint检查惯性入口、同步相位与稳定冲刺；静态Source闭包、645回归及所有阈值保持。

    - R4b方向首验2026-09-09 14:50:39：manual exit0、六态mask111111与全部方向窗口通过，2370帧无fault/graph error/temporal anomaly；视觉exit1仅因max_frame75.3451ms超过75ms，p95 16.3754ms、CPU max71.3819ms、eval max0.1934ms/p95 0.0942ms。已读取分阶段统计，现有聚合证据不足以给该帧归因，未据此复开物理/Rig或修改阈值。先完成独立sprint回归，再在同一二进制/阈值下受控复核方向一次；若再次失败，保留失败并补齐该帧的实际阶段证据。

    - R4b sprint通过/方向复核前2026-09-09 14:54:33：sprint manual/validation exit0，启动/稳定两分支、canonical phase及实际Pose链全部通过，faults/graph_errors/temporal_anomalies为空；frame p95 14.7219ms/max27.8914ms，eval p95 0.0937ms/max0.1370ms。前次方向的单帧超限没有对应持续动画求值升高，短进程采样也不能追溯归因；以下只做已声明的一次同二进制、同场景和同75ms阈值方向复核，保留首份失败记录。

    - R4b及o完成2026-09-09 15:03:48：方向受控复核manual/validation exit0，2338帧、11窗口/19截图、六态mask111111；frame p95 16.5986ms/max71.5817ms、eval p95 0.0895ms，faults/graph_errors/temporal_anomalies为空。旋转view 90°的跑姿已人工检查；sprint两分支211/50样本、canonical phase距离0/0.002966，无源链失败。首份方向75.3451ms超限保留，未找出该单帧确切原因，也未通过修改算法、阈值或排除样本获得本次通过。R4b实际开始14:44:20、预估15:18:50、实际结束15:03:48（偏差-15m02s）；o2开始12:48、首估13:18、最终偏差+1h45m48s，o开始11:40、首估12:20、最终偏差+2h43m48s，中间复盘/拆分均保留。用户连续换向及停后恢复已完成本项验收，BF113.03c本轮活动源/时间/惯性请求有限复开关闭；实现为7888369。下一步仅补Standing仍未完整验收的正常停步、quick-stop、turn/rotate和动态过渡实际分支，尚不能开始Grounded。

- [x] BF115.01p Standing正常停步与quick-stop实际分支：开始2026-09-09 15:07:45、预估结束15:57:45。o已完成后继续原Standing未验收范围；估时依据为已有62源子图、四个Lock/Plant回调、两条Stop和两条Standing Transition资产、已构建真实入口及既有transition_notify场景。重新读取当前Standing/设置与PlayQuickStop相关源事实，比较真实条件/通知、左右资产选择、起点/倍率/混合/结束和Slot实际Pose；沿源FootPlanted与输入产生左右分支，不手工发通知或强制montage。先静态闭环和最小实际读数/驱动检查，再原Playground公共Gate；不展开Turn/Rotate或下一模块，也不把早先provider/纯采样测试当作这些完整动作已验收。

  - p源差异及有限复开2026-09-09 15:30:00：fresh Standing78.755s/AIS34.255s通过；Stop分支按FootPlantedAmount的-0.5/0/0.5分区选Lock/Plant，四个entry调用左右Stop，参数0.4起点/1.5倍/0.2混合；QuickStop设置0.1/0.2混合、1.75–3.0倍率、0.3起点。fresh CPP1575–1610在无输入时使用TargetYawAngle，并按source yaw左右及175°反向修正规则选择；实际quick_stop_notify_system的唯一选择器却在无输入时使用身体yaw，且将Bevy yaw直接当source yaw，导致转角归零/左右反向。旧01k只测试无输入且目标与身体同向及资产采样，未覆盖真实保留目标；01k仅按此QuickStop消费者/必要旋转输入边界复开，先建立真实消息→播放反例。复用既有RotationResponseState、目标坐标基转换和core选择函数，不添加影子朝向。

  - p最小失败/配置闭环2026-09-09 15:39:34：实际消息→quick_stop_notify_system→AlsTransitionPlayback fixture在已松键、Bevy目标yaw=-90°时2.671s稳定FAIL（index0而应1）；包含仍按键优先、左右与176°的其余组合将随修复验证。core选择函数新增176°源反例125.018s FAIL于倍率截断：源remap后-184°，无clamp的Lerp应3.0277777而非3。首次组合cargo在core失败后未继续项目测试，已单独执行实际消息反例。fresh CS58.429s确认DesiredVelocity旋转=True、继承基座旋转=False、静止ViewDirection自动旋转=True。另沿必要输入看到UE静止VelocityDirection仍使用已保留目标/DesiredVelocityYaw，当前rotation_response_system静止分支回到current_yaw；下一步只核实该直接producer并建立最小测试，再合并调用已有选择与坐标基工具。此时尚未修改生产算法或启动本项runtime。

  - p修复/static闭环2026-09-09 15:57:30：必要producer的真实rotation_response_system反例43.041s FAIL于松键停速后目标丢失；现保留既有target_yaw，删除重复quick-stop公式并按UE↔Bevy符号调用core选择器，移除源不存在的倍率clamp。最小消息/选择回归58.650s PASS，完整als-core238项21.386s PASS；旧实现即上述失败消融，未增加旋转状态。fresh源确认Move满权重且!bMovingSmooth优先Move→Stop，否则Move→Idle发QuickStop；停止内部按FootPlanted四分支。新增standing_stop公共场景用实际Idle/Move与曲线释放输入、读真实Stop FSM/消息产生的播放/Slot，验证4正常+3quick分支和完整结束，不强制动画。项目测试首次仅因observer ResMut赋值层级编译失败，已修正；最终focused/声明式Gate及同入口构建尚待通过，尚未启动runtime。当前源闭包仍62子图/23Pose图，直接consumer仅QuickStop及必要目标朝向。

  - p runtime准备2026-09-09 15:59:30：项目315项31.079s PASS，新增真实分支/完整播放/Idle完成判定focused24.826s PASS，移除分支或播放观测即失败消融；同次Standing native重新审计源闭包/当前安装树及其余声明命令4.260s PASS（fresh导出只读，旧pin临时ID不同由既有合同审计处理）。场景没有新增影子FSM；当前7秒单分支超时复用公共进展watchdog。初次预构建误用了--bin，0.787s被cargo拒绝而未构建；按run_manual.bat实际--example manual_playground、target_game/play、jobs2已纠正，等待同入口构建后startup→standing_stop。

  - p真实Gate前2026-09-09 16:03:50：同入口构建145.781s PASS；编译期间补齐公共collector要求的settled末态标记后再次增量构建62.853s PASS，当前target_game/play与源码一致。全部source/static和适用focused已通过，现在按公共顺序执行无驱动startup，再standing_stop，7分支各自7秒固定上限；保留原Playground和25ms p95/75ms max阈值，尚未宣告停止功能完成。

  - p startup通过/停止Gate前2026-09-09 16:04:30：startup manual/validator exit0，6幅原Playground截图、root Pose与公共Guard通过。已检查静止姿态，接着同一二进制运行standing_stop；首次真实动作结果仍待出具，未开展后续Turn/Rotate。

  - p首个runtime失败及01j有限复开2026-09-09 16:08:00：standing_stop在首分支7秒固定上限退出，manual91/validator1；575帧证明1.522s进入实际Stop/Lock Left Foot，但全程无Stop playback，随后只有StandingDynamic，故非释放相位问题。无fault/graph_error/temporal异常，frame p95 13.683ms/max22.3797ms。fresh StateResult.StateEntryFunction绑定PlayStopLeftTransitionAnimation；converter已正确归一为OnStateEntry，runtime state_function_notifications却仅匹配StateEntryFunction/StateExitFunction。旧01j实际FSM fixture同样用了旧名，漏检生成契约；仅该通用状态回调匹配有限复开，先把既有实际FSM测试改为生成名确认失败，再统一消费者。安装树13处入口/退出均使用OnStateEntry/OnStateExit，无旧命名资产；相邻图仅承受同一事件契约修复，不展开功能调研。另新场景的末态观察缺少公共collector要求的requested字段，按已有真实生成状态补齐，不改变视觉阈值。

  - p状态事件静态闭环2026-09-09 16:11:30：当次UE AnimNode_StateMachine.cpp SetState先调用旧StateExit、初始化目标后调用StateEntry；生成图的OnStateEntry/OnStateExit为正确target规范。既有实际FSM fixture改用生成名称后36.574s FAIL，通知为空；runtime三处统一名称后als-ext332+项目315项32.944s PASS，source/static4.476s PASS，旧命名即失败消融。无新事件类型或兼容分支，使用生成器已有规范；公共视觉末态按真实稳定FSM填写requested/actual并包含settled。正在同入口重建，接着startup及停止完整分支复验。

  - p状态事件修复runtime前2026-09-09 16:13:35：同入口构建115.245s PASS；当前修复及末态报告字段已完整编入target_game/play。按公开startup→standing_stop重新验证，首轮失败保留，7秒上限与视觉/性能阈值不变。

  - p回调修复startup通过/停止Gate前2026-09-09 16:14:10：startup manual0/validator0、6图，faults/graph_errors/temporal为空，frame p95 13.720ms/max26.5818ms；同一二进制继续standing_stop，尚不关闭01j/01k有限复开。

  - p第二次runtime/超时复盘2026-09-09 16:17:45：回调修复后四种正常Stop均在对应实际状态启动左右动画并播放至2.333333、回到Idle，default QuickStop也完成；manual仍91/validator1，quick_left被仅1帧上一案例的完成读数错误勾选，quick_right实际RotationMode一直ViewDirection而选Left1.75，因此不接受左右分支通过。trace证明driver在begin_phase后同帧observer读新index+旧probe，另direct RotationMode/request被aim_rotation_mode_system按aim_held/LastFree覆盖；此前实际direction fixture已有对应输入范式，新的场景未复用。无fault/graph_error/temporal，frame p95 14.0431ms/max41.4442ms。初轮开始15:07:45、预估15:57:45、实际复盘16:17:45（偏差+20m，未完成），耗时包含native3资产、选择/目标/入口事件三处根因修复及两次真实Gate；剩余仅场景case交接/真实模式输入和复验，当前暂停新的runtime与生产实现，按原50分钟窗口的1.5倍拆分下一轮。
    - [x] BF115.01p-R2a 停止场景真实输入与交接：开始2026-09-09 16:17:45、预估结束16:52:45。以实际报告的pending-entry旧probe和当前input_state_request_system/aim_rotation_mode_system为边界，复用既有输入命令/模式切换，先最小失败fixture再修驱动与完成读取。只修场景，不修改动画/物理；focused/static及同入口构建通过后独立完成。
    - [x] BF115.01p-R2b 停止完整验收与回调回归：计划开始2026-09-09 16:52:45、预估结束17:32:45。依赖R2a，提前完成则顺延提前开始；沿刚通过startup的同一生产实现复验7分支standing_stop及受回调影响的standing_response，审查全部视觉/生命周期/性能证据，完成后关闭01j/01k有限复开和p并提交。该拆分不缩小原Standing目标，不提前转入Turn或Grounded。

  - p-R2a完成/真实Gate前2026-09-09 16:22:55：两处场景最小反例24.287s FAIL（pending-entry被当活动案例、实际模式View而非Aim）；输入命令补aim_held和现有rotation_mode_pressed、删direct模式赋值/请求，observer只读本case完成entry后的帧，并要求实际模式与case一致。三项真实输入/交接/足相fixture20.549s PASS，旧输入/去掉entry边界即失败消融，声明式4.223s和同入口构建65.305s PASS。R2a开始16:17:45、预估16:52:45、实际结束16:22:55（偏差-29m50s）；R2b提前实际开始16:22:55、预估结束17:02:55，保留终段窗口。生产运动/动画代码自16:14 startup通过后未改，复用该无驱动Gate；现在只重新运行standing_stop，然后standing_response回调回归。

  - p-R2b运行边界复盘2026-09-09 16:27:20：input修复后的真实报告4正常+默认/左Quick完整通过，右Quick确实选Right且2.529132倍播放到约1.35s，但被StandingDynamic接替而未完整结束，故manual91/validator1；全部公共姿态/性能Guard通过（frame p95 14.4133/max46.6613ms）。右case初始仍无输入时已在0.067s、0.454s排入Dynamic；之前左case结束body yaw约+0.781，driver随后强制重置为-0.785，而新case首实际读数为-0.331。fresh RefreshDynamicTransitions仅在AllowTransitions且足锁/目标距离超过source阈值时排队，不能禁用它或放宽完成判定。当前删除案例之间非必要的角色/旋转状态重置，只在首case定位原场地；后续保持自然停止姿态，把view参考对齐当前实际body yaw以生成相对左右输入。此为fixture重置步骤消融，不修改任何动画、Dynamic、足锁或物理算法；三项fixture与同入口构建后复验，旧失败保留。

  - p-R2b连续姿态Gate前2026-09-09 16:30:00：删除后续case重复定位/朝向重置后，三项输入/交接fixture20.575s、同入口构建65.669s PASS；该改动只影响standing_stop驱动，沿用已通过的生产source/static/startup。现在以原场地连续角色生命周期复验7分支，仍要求每个自身正确资产/倍率、真实Slot姿态、完整结束及Idle；未禁用Dynamic或提高任何阈值。

  - p-R2b停止Gate通过/回调回归前2026-09-09 16:31:15：连续角色standing_stop manual0/validator0，1367帧、7窗口、14截图；4个Lock/Plant与默认/左右Quick全部实际播放完成并回到Idle，左右Quick2.354919/2.359717倍率、0.8s canonical距离0.010749/0.009504，方向签名及source pose链通过。frame p95 13.8828ms/max16.7462ms，eval p95 0.0821ms，faults/graph_errors/temporal为空；左右截图已检查。删除后续案例重置即保留全部目标信号并消除夹入的Dynamic，故永久删除该工具步骤，没有改Runtime Dynamic。现在同一二进制执行既有standing_response，验证刚恢复的状态回调对用户2秒反向/0.25秒循环及持续Idle的影响；该回归完成后关闭p。

  - p-R2b回调回归失败/01o有限复开2026-09-09 16:39:40：standing_response manual0但validator1，2秒反转/循环/持续Idle均覆盖，phase3.681712（render4.667710）pre-rig thigh_r单帧1.226375rad、86.55643rad/s并引发final手/脚异常；frame p95 17.6115/max31.8561ms，无graph_error。该帧没有任何Stop/Quick montage，不能归因新回调。actual Run→FirstPivot在3.654296触发；fresh Standing该边明确0.1s惯性，UE AnimNode_Inertialization.cpp905–918将RotationAngle Unwind并改轴号，以PrevQ.GetTwistAngle和再次Unwind算速度；当前rotation_difference直接采用0..2π轴角、scaled-axis投影和未wrap角速度，存在反号四元数长路径差异。01o/o2仅按该实际惯性旋转边界复开；旧单骨骼fixture只有正号90°、FSM运动fixture主要平移，未覆盖q/-q和±π速度边界，前次真实通过没有触及该表示。BlendSpace的Pose.linear_add已做同半球累加，排除后不改它，不展开IK/物理或相邻图。
      - [x] BF115.01p-R2b1 惯性旋转最短角与twist速度：开始2026-09-09 16:39:40、预估结束16:59:40。以当次source的两处旋转公式、现有惯性fixture和这条真实Pivot输入为边界，建立反号/跨π/mixed-axis最小失败，修复共享rotation_difference后focused/static及同入口构建，再同源startup/response与必要停止回归，保留原Guard。估时依据为已完整定位的单函数差异、约半分钟测试和2分钟同入口构建，以及当前现成真实场景；不以该子项复开整个运动架构。

  - p-R2b1旋转静态闭环2026-09-09 16:48:40：旋转反号/角速度最小fixture33.152s FAIL于等价历史的非零速度，局部函数按source Unwind→正角/轴反向→TwistAngle→速度Unwind修复；四项惯性测试19.529s、333ext+317项目37.967s PASS，原实现即失败消融。尝试把两个Rust反例加入authoring verification_commands被其仅允许Python/tools脚本的合同拒绝（0.269s，随后main的缺pass触发KeyError）；已删除这两个额外步骤，反例仍由现有cargo测试与本项Gate执行，不新增包装器或修改静态工具。重跑既有声明式Gate后再同入口构建，尚未启动新的runtime。

  - p-R2b1真实Gate前2026-09-09 16:51:40：声明式4.527s、同入口target_game/play构建117.027s PASS，650回归此前通过；现在startup后优先standing_response复验phase3.681712的Pivot跳变，再停止实际分支回归。除共享惯性旋转的源差异外，未更改任何图/clip/FSM状态规则、IK/物理或Guard阈值。

  - p-R2b1 startup通过/反向Gate前2026-09-09 16:53:35：startup manual0/validator0、6图，frame p95 13.5591ms/max23.5086ms，fault/graph/temporal为空；过程中记录4次Turn slot惯性请求，Turn实际分支尚未验收，不把本项视作Turn完成。现在同一二进制执行standing_response，优先检查原Pivot旋转跳变。

  - p-R2b1反向通过/停止回归前2026-09-09 16:55:45：short_arc standing_response manual0/validator0，716帧，反向/循环/持续Idle全部通过；faults/graph_errors/temporal为空，frame p95 20.1415ms/max33.8209ms。源最短角/twist修复已通过真实同节奏反例；继续同一二进制standing_stop及既有方向/冲刺，覆盖共享惯性函数的直接调用范围，不展开新模块。

  - p-R2b1完成/方向回归前2026-09-09 16:57:45：惯性修复后的standing_stop再次manual0/validator0，4正常+3Quick全部完整播放/Idle、源pose链和方向签名通过，fault/graph/temporal为空。R2b1开始16:39:40、预估16:59:40、实际结束16:57:45（偏差-1m55s）；650回归与声明式、startup/response/stop共同闭合最短角差异。R2b继续同一二进制既有standing_direction及sprint回归，保留所有此前失败；不重复或改变已经通过的姿态阈值。

  - p最终方向性能边界/用户后续选项2026-09-09 17:05:10：short_arc standing_direction manual0、六态111111及全部姿态/源链通过，validator1仅因frame p95 27.7571>25ms（max45.4107）；CPU p95 23.1177ms、eval p95 0.1465ms，物理区间/ControlRig主要占用，步数恒1；短系统采样见浏览器负载但不能追溯归因。停止/反向已通过，先既有独立sprint，再做且仅做一次相同二进制/场景/阈值的受控方向复核，保留此次失败，不猜测优化。用户随后明确指出连续原地转身的胯部/腿部平衡不自然，并提出montage速率可能跟不上快速转身；作为p收口后的下一选定Standing任务，按source核对实际montage速率、RotationYawSpeed倍率、Actor角速度、骨盆与足锁，不先提高速率或加平滑。

  - p-R2b冲刺失败/测量时钟闭环2026-09-09 17:15:45：sprint manual0、启动/稳定分支通过，validator1保留max84.1566ms性能失败，以及pre-rig脚/腿速率40.3434/40.2748rad/s。后者所在两帧phase elapsed5.2565503→5.3419995（动画步长85.4492ms），collector wall elapsed6.3194023→6.3384139（19.0115ms）；实际Guard分母取后者。fresh本地Bevy time_system从render time receiver更新Time，generated_ab_als_provider以Time.delta_secs写animation.delta_seconds；收集时刻受CPU阶段位置影响，不能作为该姿态的动画步长。已有恒定dt Guard fixture漏检该时间偏移，01o已有限复开的验收边界扩至此测量consumer；不改变动画时钟、速率或性能阈值。新增已记录0.766989/85.4492ms应正常、此前真实惯性1.226375/14.8859ms仍应拒绝的同一Guard反例，先确认失败再修正采样。
      - [x] BF115.01p-R2b2 公共Guard姿态时间：开始2026-09-09 17:15:45、预估结束17:21:45。只在已有pre-rig快照携带实际Time.delta_secs，用于pre/final速率；墙钟frame耗时继续独立执行25/75ms预算。沿真实Guard做反例/消融及回归、同入口构建，然后既有sprint和一次方向复核；估时依据为单个采样字段/消费者、已读取时钟源码、约20秒focused及1分钟增量构建。

  - p-R2b2静态通过/冲刺复核前2026-09-09 17:20:05：原墙钟分母fixture39.761s FAIL误报；已有pre-rig快照携带Time.delta_secs，pre/final Guard使用该同帧步长，删除无关的wall参数，异常JSON另记animation_delta_seconds。33项视觉回归21.619s、证据工具自测0.527s、同入口构建65.429s PASS；原真实惯性跳变仍拒绝。生产图/动画/物理不变，已有startup/response/stop继续有效；现在sprint复核及首次且唯一受控方向性能复核，25/75ms性能门槛不变。

  - p-R2b2冲刺通过/方向受控复核前2026-09-09 17:21:20：pose_clock sprint manual0/validator0，启动/稳定、源curve/pose/sync全部通过，fault/graph/temporal为空；frame p95 14.1017ms/max20.3638ms，eval p95 0.0828ms。先前84.1566ms性能失败保留且未改25/75预算。现在只做已声明的一次相同生产实现、相同方向场景/阈值的受控复核；测量修复仅使姿态速率使用真实动画时间，墙钟性能测量不变。

  - p完成2026-09-09 17:25:45：方向受控复核manual0/validator0，六态111111、11窗口完整，frame p95 15.8884ms/max70.0931ms、eval p95 0.0860ms，fault/graph/temporal为空；最新方向与冲刺截图已人工检查。该Gate实际结束17:22:17（runner落盘），早于R2b估时17:02:55+20分钟界线；最终证据复核完成17:25:45。R2b2开始17:15:45/预估17:21:45/实际结束17:25:45（+4m）；R2b实际开始16:22:55/预估17:02:55/实际结束17:25:45（+22m50s，Gate在容差内通过）；p开始15:07:45/原预估15:57:45/实际结束17:25:45（+1h28m）。重估、拆分与失败均保留，额外时间源于真实状态事件漏接、惯性四元数语义及Guard时钟三项旧Gate漏检和新场景驱动问题。01j/01k/01o的本轮有限复开关闭；累计core238、ext333、项目317及新增33视觉回归/工具self-test，source/static、同入口编译、startup/response/7-stop/direction/sprint均完成适用Gate。首次方向p95超限和Sprint单帧84.1566ms保留，未证明外部原因，未改性能预算或动画算法进行性能补偿。Standing仍未全部完成：下一项为用户已指定的连续原地转身胯腿协调与montage速率，继续default Turn/Aiming Rotate及动态过渡实际分支，尚不进入Grounded。

- [x] BF115.01q 连续原地转身的胯腿协调与播放速率：用户2026-09-09 17:05/17:06明确选择，指出快速连续原地转身平衡感不足，并提出montage速率可能跟不上；仅作为待证假设，唯一标准仍为UE ALS-Refactored。以Standing现有62子图/23Pose图、4个Turn/2个Rotate资产、已闭合原生函数和Slot通道为起点，同时核对默认ViewDirection Turn与Aiming Rotate；不先加速、补偿或平滑，不提前进入Grounded。依据上一停止任务原50分钟实际138分钟且暴露多个旧Gate漏检，本轮先拆可独立验收的source/Actor静态与真实动作两段，不以一次截图或旧Actor输出验收。
  - [x] BF115.01q1 转身源合同与Actor/Slot静态闭环：开始2026-09-09 17:29:00、预估结束18:09:00。重新提取Standing/AIS/相关Actor source，逐帧阶段比较RefreshTurnInPlace/RefreshRotateInPlace、montage选择/倍率/起点/重启与混合、RotationYawSpeed缩放、Actor朝向、骨盆/足锁坐标和输入生命周期。只修fresh source证实且能到达用户症状的差异；先最小反例、再复用已有Actor/Slot/转换器扩展，完成消融、focused/static与同入口编译后交q2实际验收。
  - [x] BF115.01q2 连续转身与平衡真实Gate：计划开始2026-09-09 18:09:00、预估结束18:59:00。依赖q1；沿原Playground真实鼠标/瞄准输入，覆盖左右90/180转身、连续快速转向和瞄准原地旋转，记录实际资产/clip时间/播放速率、曲线倍率、Actor角速度、骨盆运动与足锁，并检查pre-rig/final/temporal及性能。源合同闭合后才设计最小场景，已有Gate能力优先复用；仍保留Standing其余动态过渡完整分支的后续验收边界。

  - q1源闭包/有限复开2026-09-09 17:37:00：MCP健康检查明确连接拒绝，fresh native Standing43.721s/AIS34.278s/CS34.899s通过。AIS四个Standing Turn均1.2倍、90/180度缩放开启；Rotate为50度触发、180–460度/秒映射1.15–3.0倍，与当前配置相同。UE Montage_PlayInternal以新BlendIn停止旧group，故现有接替线性混合没有证实差异。fresh Actor从当前身体yaw积分RotationYawSpeed并重置target/smooth target，静止松键ViewDirection随后保留该目标；当前共享rotation_response_system从旧smooth target累加且松键追新view，直接可脱离转身pose。BF102.11只按该消费者有限复开，旧Gate缺少身体/目标分离与松键持续转动视角反例；接着真实system最小失败，不改montage倍率或足锁。Standing闭包仍62源子图/23Pose，尚未运行q的runtime。

  - q1静态闭环/startup前2026-09-09 17:46:10：真实rotation_response_system反例34.299s FAIL，零曲线/无输入仍产生0.009951rad旋转；按source先从身体积分，再重置目标平滑初值并执行该模式的目标选择，删除原曲线专用输出分支。正反曲线、blocked/unblocked、结束后视角变化及重新输入共享一个实际system fixture，旧实现为失败消融；最终239core（1ignored）16.138s、318项目44.889s、fresh闭包/声明式6.078s、target_game/play同入口构建140.968s全部通过。未新增运行时状态、倍率参数或足锁补偿，现有4Turn/2Rotate源资产与曲线绑定无新差异。现在同一二进制执行无驱动startup；q2仍需实际连续动作验收，BF102.11有限复开尚未关闭。

  - q1完成/q2开始2026-09-09 17:48:00：同一二进制startup manual0/validator0，6图、842帧，frame p95 14.4894ms/max27.1573ms，faults/graph_errors/temporal_anomalies为空，原场地静止截图已检查。q1开始17:29、预估18:09、实际结束17:48（偏差-21m）；source/static、最小失败消融、239core/318项目及同入口构建完成，Actor静态修复单独提交，BF102.11仍待q2完整动作Gate。q2提前实际开始17:48、预估结束18:38，沿当前源闭包补齐四种Turn与两侧Rotate及快速连续输入，先场景focused检查，再同入口构建和一次完整公共Gate。

  - q2场景静态闭环2026-09-09 18:04:10：standing_turn沿真实look_delta/aim_held驱动4Turn、2Rotate和连续多次左右转向，只在首case定位原场地，后续保持身体/足锁连续。observer读取实际Turn stack和Rotate Sequence Player的已求值节点时钟、真实Standing FSM、曲线倍率、骨盆/腿Pose与足锁/膝点；沿实际Slot/Sequence→Standing→当前父入口补公共source Pose链。41项场景focused40.797s、声明式4.705s PASS；30/120FPS真实输入一致，移除实际播放、完整结束或Idle读数均不能通过。设计消融删除未经source要求的“必须中途接替”条件及其状态，UE AllowTransitions会限制重触发；复用既有variant选择和窗口trace，删重复Crouch命名映射。正在同入口增量构建，生产Actor仍为7f818a5，可沿用已通过startup；接着首次完整q2公共Gate，未修改曲线、montage倍率、IK或任何Guard阈值。

  - q2真实Gate前2026-09-09 18:09:15：场景首次构建66.595s通过后，source短弧复算发现+140→-140可继续同向，故连续fixture改为六次±75度/1.2秒切换再回中，仍要求至少三次实际montage、左右均出现和末次自然结束；独立±160输入仍验证180资产。更正后的两项实际输入/完成focused23.716s、同入口构建64.777s PASS；同一生产实现的startup已通过，当前首次启动standing_turn公共Gate，每case12秒固定上限，保留25ms p95/75ms max与公共姿态阈值。未因夹角歧义修改source旋转算法或放宽真实分支要求。

  - q2首轮结果/报告边界2026-09-09 18:16:50：manual91/validator1，1823帧中7个动作case全部完成，source Pose链/方向签名/temporal通过，frame p95 14.0748ms/max20.7055ms、eval p95 0.0888ms，faults/graph_errors/temporal为空。失败来自新场景误用只在速度>0.5的Move中置位的standing_graph标记，以及末帧空montage覆盖了现有drive保留的clip/rate/variant；实际末态trace最小反例21.123s FAIL于variant=None。接着按实际Standing状态置位，并保留真实停止drive读数、inactive权重0；不改validator。Rotate右末态仍有约50.9度view差，需同时等待实际Rotate flags关闭，不能只据某帧Idle结束。
  - [x] BF115.01q2a 用户补充的Aiming持续极限测试：开始2026-09-09 18:17:00、预估结束18:37:00。用户明确反馈已有好转但测试强度不足；沿同次Rotate设置/源消费闭包，在原场地保持Aim，连续720度/秒正转4秒、反转4秒，再每0.25秒反向4秒，之后自然收稳。720超过source的460度/秒倍率映射上限，验证实际3倍动画、左右多圈Actor运动、所有过渡帧的胯腿/足锁连续性和性能；source倍率与阈值不变。复用当前输入、节点时钟与公共Guard，只补必要的累计转角和分段截图证据；静态/focused闭环后执行完整standing_turn，再必要的standing_response回归。

  - q2a静态闭环/极限Gate前2026-09-09 18:26:30：末态variant反例修复复用现有drive保留字段，移除None覆盖，inactive权重明确0；Standing标记只由本case实际状态/Slot已求值置位，Rotate结束同时要求实际flags关闭。极限输入在30/120FPS真实View/Aim系统各累积超过11圈/方向；真实Gate另要求Actor至少6圈/方向、实际倍率>2.9、左右Sequence均出现并自然回Idle。42项focused24.705s、fresh静态4.487s、同入口构建63.568s PASS；新增累计转角的低值反例拒绝通过。当前沿已通过startup的同一生产实现运行完整8-case standing_turn，极限段20秒固定上限；正向/反向/快速反转三段各留source窗口和截图，独立Temporal Guard覆盖所有帧，源倍率/视觉/性能阈值不变。

  - q2a极限结果/公共观测修复2026-09-09 18:41:35：首份极限报告manual0，Aiming实际1048帧、左右累计10.2876/10.2650圈、倍率最大2.999999；全场frame p95 13.9629ms/max23.5403ms、eval p95 0.0844ms，全部8-case与胯腿/骨段/graph Guard通过，已检查高速/反转截图。validator1剩普通Rotate末帧旧Actor曲线与当前inactive权重的时序错配，以及实际连续+190.4509度被固定baseline最短角误判反向。BF111.04仅其公共signed yaw观测有限复开；多圈/真反向最小反例23.915s FAIL，保留逐帧最短角累计（新增一个累计标量、复用原reference）后323项24.981s与validator self-test0.522s PASS，真反向仍拒绝。收稳还要求实际曲线归零及view差已低于对应45/50度源阈值，避免待触发Turn被计为结束。删除起播ID列表，改用已有完成epoch要求连续Turn三次完整结束，4项focused20.622s、最终同入口构建63.596s PASS；现在复验同样8-case极限Gate，不改任何生产动画/IK/倍率或Guard阈值。

  - q2a极限复验通过/回归前2026-09-09 18:44:30：完整standing_turn manual0/validator0，8-case全部覆盖、10个source窗口/17图；frame p95 13.7092ms/max16.3997ms、eval p95 0.0822ms，faults/graph_errors/temporal为空。Aiming实际1009帧、左右累计10.1564/10.1239圈、倍率最大2.999999，结束曲线0且view差-11.69度（低于50度源阈值）；普通连续Turn等待实际完整结束与45度阈值后10.835s完成。最新高速右转与反向截图已检查。公共signed累计及真正静止末态通过，同样极限输入/阈值未改；现在同一二进制执行standing_response，覆盖Actor旋转修复直接影响的移动→停后恢复，不重复整套方向/冲刺。

  - q完成2026-09-09 18:48:20：停止恢复standing_response同一二进制manual0/validator0，反向/快速循环/持续Idle全部覆盖，frame p95 17.5678ms/max35.005ms、eval p95 0.0859ms，fault/graph/temporal为空；末段站姿截图已检查。q2实际开始17:48、预估18:38、实际结束18:48:20（+10m20s）；新增q2a开始18:17、预估18:37、实际结束18:48:20（+11m20s），均在容差内完成，额外工作为用户指定的持续极限输入和旧公共yaw/末态观测漏检修复。四种Turn、左右Rotate、三次完整连续Turn、720度/秒Aiming持续多圈/快速反向及停止恢复全通过；BF102.11 Actor和BF111.04公共signed观测的本轮有限复开关闭。Actor修复7f818a5，最终保留source的1.2倍Turn与1.15–3.0倍Rotate，没有新增速率补偿或IK平滑。Standing仍剩动态过渡左右完整分支验收，随后才进入Grounded；原完整移植目标继续。

- [x] BF115.01r Standing左右动态过渡完整分支：开始2026-09-09 18:54:00、预估结束19:29:00。q的正常/持续Turn和Aiming极限已完成，继续Standing最后尚未完整验收的Standing Dynamic Left/Right；原完整移植顺序保持。估时依据为当前62源子图/23Pose闭包、已有RefreshDynamicTransitions/native队列/两条montage采样和真实Slot入口，参考p的7停止动作与q的8转身动作实际记录；复用现有场景/报告后，仅核对两分支的源触发、选择优先级、倍率/起点/混合/完整结束及足锁恢复。先重新提取Standing/AIS及必要CPP事实，核对FootLock与当前足目标坐标/生命周期；只扩展source直接证明的producer或consumer，已完成IK/图不因猜测复开。用原Playground实际输入产生左右动态过渡，不直接发通知或强制montage，先focused/static及同入口构建，再公共Gate。该项完成前不开始Grounded。

  - r源闭包2026-09-09 19:02:00：MCP连接拒绝后fresh Standing47.808s/AIS33.746s通过，62可达源子图/23Pose闭包保持；Idle的RefreshDynamicTransitions直接读取世界空间足锁/足目标，source阈值8cm、FrameDelay2、倍率1.5、起点0、混合0.2。目标native选择/队列/Slot与该设置一致；原极限Aiming报告两侧Dynamic均出现但约0.5秒被Rotate打断，不能充当完整结束证明。新增原场地输入场景以持续Aiming转动产生源队列，观察目标侧排队后松Aim并切换VelocityDirection，保留实际身体/足锁连续，等待真实Slot完整结束和该脚重锁；不改动画、足锁或阈值。当前先完成输入/完成判定focused与fresh静态，尚未启动本项runtime。
  - r静态闭环2026-09-09 19:09:00：两条Dynamic uasset由既有工具0.303s重读，生成记录与当前target逐项一致（1.533333秒、Stand Pose首帧mesh-space additive）；所选脚FootLock在0.466667–0.5秒为-1的additive曲线，0.533333恢复0，AllowTransitions同段-1，故检查解锁→重锁及8cm恢复边界。324项目测试31.053s和fresh静态4.521s PASS；禁用场景的RotationMode切换后focused21.126s稳定FAIL于ViewDirection而应为VelocityDirection，证实仅松Aim不能让该完整动作在正确前提下结束。恢复后正在focused复验/同入口target_game/play构建；源consumer仅延伸到CPP明确的AB_Als根Transition Slot，不展开其余根图。
  - r运行前2026-09-09 19:12:25：输入消融恢复focused17.960s PASS，同入口target_game/play构建66.207s通过；canonical截图时间按源解锁阶段定为0.4秒，增量构建63.870s通过。native源闭包/编辑器/位置/运行资产边界及全部声明命令已通过，生产动画/Actor/IK未改。即将沿run_manual.bat执行无驱动startup及standing_dynamic两分支，要求源队列选择、实际Slot完整播放、Idle和所选脚解锁→重锁/距离恢复，12秒单分支固定上限，公共temporal/性能Guard不变。
  - r startup完成2026-09-09 19:13:30：manual0/validator0，809帧/6截图，frame p95 17.1416ms/max27.2793ms、eval p95 0.0779ms，fault/graph/temporal为空；原地站姿截图已检查。继续同一二进制的左右dynamic完整动作Gate。
  - r首轮真实失败2026-09-09 19:15:00：manual91/validator1在首case12秒上限退出，1095帧、frame p95 13.147ms/max34.8196ms、eval p95 0.0818ms，fault/graph/temporal为空。16次实际源队列均选择Left，触发时Left距离平方约0.06–0.085m²大于Right，选择符合source；场景只按旋转方向等待Right，未到松Aim步骤。旧极限报告中的首个Right来自不同足锁历史，不能据旋转方向指定脚侧。去掉此fixture假设，输入改为每2秒反向、源选择目标侧后才松Aim；30/120FPS检查覆盖两个方向累计转角，保留12秒上限与所有完成/视觉要求。无生产实现或已完成条目复开。
  - r复验前2026-09-09 19:18:15：交替输入focused26.390s PASS（30/120FPS每方向四圈），同入口构建64.673s通过；源/图/生产实现保持刚通过startup的版本，复用本轮fresh静态。继续standing_dynamic，同一12秒上限、完整播放/重锁及公共Guard，无阈值放宽。
  - r完成2026-09-09 19:20:00：交替输入真实Gate manual0/validator0，683帧、2个完整source窗口/9截图，frame p95 13.5144ms/max16.3948ms、eval p95 0.0827ms，fault/graph/temporal为空。Right/Left分别4.0608/4.0741秒完成，实际montage均自然推进到1.533333秒、无stop请求；所选脚LockAmount均降到0再回1，末态偏差约2.64/2.59mm，实际Idle/Slot结束，0.4秒canonical误差0.003623/0.002965秒，左右pre-rig签名和根Transition Slot运动通过，截图已检查。开始18:54、预估19:29、实际结束19:20（偏差-9m），结果完成，无拆分；本项仅新增公共输入/观测场景，未修改source对应算法、曲线或阈值。
  - Standing模块闭合：01a–01r已覆盖62个可达source子图/23个Pose图，完成转换器通用执行/缓存/惯性、Actor方向/曲线/停止/转向、六向Walk/Run、Sprint、正常/quick-stop、左右90/180 Turn、Rotate/720度每秒持续Aiming及左右Dynamic的适用静态/真实Gate。原source记录保持不变；此前失败与旧Gate有限复开已分别闭合。此处结束Standing工作，按用户顺序进入AB_Als_Grounded，不提前展开Crouching或其余模块。

- [x] BF115.02a Grounded源闭包与转换/Actor差异：开始2026-09-09 19:22:00、预估结束20:02:00。依赖Standing全部Gate及提交b523e0e；原顺序继续AB_Als_Grounded。估时依据为已归档同图20个可达source子图（更早导出120个raw graph）、当前约三十个生成图/FSM/条件资产、旧生成路径由GroundedLayer写入state label而需重新核对实际FSM执行，以及Standing本轮source/运行时修复暴露的缓存/生命周期漏检；先用40分钟可验收段重新提取本地Grounded、相关stance/roll设置和必要CPP，按当前公开uasset转换路径审计完整可达闭包、实际路由/惯性/片段/事件及对应Actor输入，建立首个差异最小反例或完成无差异合同。该段不承诺全部Grounded runtime；具体实现与真实Grounded Gate在剩余边界明确后各列独立不超过60分钟的子项。Standing已完成内容不猜测复开，Crouching只作为Grounded直接linked-input边界，不提前调研其内部移动图。

  - 02a源/差异Gate完成2026-09-09 19:40:00：MCP连接拒绝（2.395s）后通过公开uasset转换80.039s完成fresh Grounded/CF_Als_StanceChange，另fresh AIS33.434s；完整闭包120 raw/22可达graph、112可达node、6 Pose图/1 FSM/13条件图、5实际状态加2 alias/1 conduit、12可见边与8函数依赖。AIS速度混合半衰期0.1、Lean半衰期0.2；C++ InitializeGrounded要求重新初始化同一VelocityBlend，RefreshGrounded同时刷新VelocityBlend/Lean，两stance共用；ResetGroundedEntryMode清空tag，Roll的StateExit调用PlayTransitionRightAnimation，0.2/0.2/1.5/0.2、bFromStandingIdleOnly=false并按执行时actual stance选资产。
  - 当前安装反例：首个spec因未传实际asset root而在2.996s明确失败，修正参数后6图fresh spec2.131s PASS；同次源闭包审计0.800s稳定FAIL于6个已安装RON。三条Sequence缺少当前源曲线，Grounded边缺少source bAlwaysResetOnEntry=false策略，其余为同步字段显式默认值；原始导出不变，当前生成仅写隔离staging。native Initialize/Refresh/Reset仍为Pose透传，Actor独立GroundedLayerRuntime推算重置/退出；真实观测仍加载旧手工Grounded图handle，不能据其label回显验收当前生成树。这些源调用/消费者差异保留为下一修复边界，不把转换report ready当行为通过。
  - 本段开始19:22、预估20:02、实际结束19:40（偏差-22m），结果为完整源闭包与可重复安装差异，未执行Grounded runtime；拆分为下一段生成数据迁移，随后分别闭合原生共享刷新、实际状态回调/观测和真实Gate。Standing已完成功能仅在必要共享输入有source反例时有限复开，Crouching内部移动图不在本段。

- [x] BF115.02b Grounded生成曲线与状态初始化策略迁移：开始2026-09-09 19:40:00、预估结束20:00:00。依赖02a；估时依据为公开转换已在隔离区得到30资产、仅6个安装差异，以及fresh spec约2秒/源审计约1秒和既有source Sequence/可中断FSM回归。仅通过既有converter/materializer安装本次Grounded所属数据，保留两条站蹲片段及Roll曲线、source目标状态重入策略，建立旧数据不能通过的focused合同和消融，完成fresh spec、声明式/编辑器/位置/资产边界及适用测试。该段只完成生成数据静态接入；native Grounded刷新、Reset/Roll实际回调和当前图观测尚未闭合，全部静态闭环前不启动Grounded真实场景。

  - 2026-09-10 15:55:00有限复开：用户追加全部同类来源常量审计后，已安装Roll evaluator仍为frame0；重新读取本地Grounded45.945s，确认同一节点可见pin与Node均为frame45。旧Gate检查了曲线、Roll montage和退出生命周期，遗漏静态参考帧的独立source比较。仅复开02b的该生成常量及其真实回归，执行/估时见05b2；其余算法、回调和已完成窗口保持，不按猜测重开。

  - 02b完成2026-09-09 19:54:00：复用现有安装器和AB_Als owner记录，只接入30个Grounded所属生成资产，实际diff为6个RON；没有修改原始导出或其它模块。旧安装数据在同次源审计0.800s失败作为移除/旧数据对照；新安装0.566s、30文件编辑器修复/复查零改动0.298s、6图fresh spec2.106s与新声明式Gate2.113s全部PASS，22源子图/24 Pose节点/16 Pose边/13条件完整匹配，位置与运行资产边界通过。12项SourceSequence回归39.563s、真实共享缓存/嵌套FSM初始化fixture1.310s PASS；沿用既有转换器能力，无新抽象/运行时分支。开始19:40、预估20:00、实际结束19:54（偏差-6m），结果为生成数据静态迁移完成、无拆分；未启动Grounded runtime，原生刷新和实际事件/观测仍是明确开放边界。

  - 02b生成来源补充：c9c9e283未手工修改RON。曲线生成来自560d2856（pose_renderer的Sequence Player/Evaluator source_curves），状态重入策略来自b113e3c（fsm_contract/materializer/pose_renderer）。本次19:24:05通过公开ue_animgraph_converter.py的uasset入口重新读取并生成，80.039s完成；六个提交RON均与target/bf115_grounded/conversion内当次输出内容完全一致，安装器仅按现有owner接入生成结果。用户再次明确所有生成文件都须先修转换器再重新生成，后续保持该路径。

- [x] BF115.02c Grounded真实状态回调与观测：开始2026-09-09 19:54:00、预估结束20:34:00。依赖02b；以本轮8处函数依赖中的ResetGroundedEntryMode（3处OnBecomeRelevant）、两处stance-change entry Stop和Roll exit PlayTransitionRight为范围。先最小真实graph/dispatcher反例验证阶段、次数和执行时actual stance，复用实例属性/现有状态通知与Transition Slot，移除GroundedLayerRuntime对这些事件的独立推算；观测改读当前生成Grounded FSM，不以写入label回显。AIS明确指向的Standing/Crouching Right Transition仅作为该函数直接消费资产纳入，不展开Crouching内部图。估时依据为Standing已打通的公共状态通知/队列及现成Slot采样，仅补3种Grounded回调、两条右侧资产分支和当前graph reader；静态/消融/focused通过后提交，再闭合Grounded共享VelocityBlend/Lean原生刷新，全部静态完成后才真实Grounded Gate。

  - 02c有效失败2026-09-09 20:01:00：实际分发器对PlayRollToGroundedTransitionAnimation返回None，且观测器handle未指向当前根图引用的Grounded；新增两个最小断言在34.569s同轮测试稳定FAIL，另22项相关测试通过。转换器ResetGroundedEntryMode映射反例0.302s FAIL：输出仍为无作用Pose透传，源要求OnBecomeRelevant清空entry tag。旧Gate只验证手工状态推算、函数名和常量node title，未约束真实生成图引用/实际分发。接着复用SourcePropertyWrite帧标记和现有dispatcher，删除该Actor独立事件路径；不会手工修补生成RON。

  - 02c静态进展2026-09-09 20:26:00：314项目测试40.224s PASS，Reset帧号消融22.431s稳定FAIL于提前/过期帧消费，已恢复当前帧条件。公开Grounded uasset转换86.155s零unsupported完成，新生成仅3个State的Reset节点变为SourcePropertyWrite；经原owner安装0.551s，不手改RON。CrouchingRight Transition按fresh AIS/CPP的直接资产由既有native montage生成器0.575s重新生成17条记录，原16条内容保持；新资产2.333333秒、源参考为Stand Pose首帧，遵守实际源设置。BF111.05只按已复现的新生成回调/reader边界有限复开，保留旧物理/desired stance边界；同入口构建和fresh静态正在收口，Grounded共享原生刷新与完整runtime仍未通过。

  - 02c完成2026-09-09 20:30:00：Reset帧号恢复focused20.036s PASS；target_game/play双example构建75.060s、30文件编辑器修复/复查零改动0.293s、6图fresh spec3.133s、完整声明式/22源子图审计2.645s PASS。共314项目回归、两个旧入口失败及帧号消融均保留；生成器先改再公开uasset重生成，3个State的Reset节点已接入，CrouchingRight资产由源工具生成。GroundedLayer只保留当前生成图的实际观测，删除未被生成图消费的独立状态/时钟/曲线/回调推算及其镜像测试；实际回调统一走现有通知/队列，旧手工图与Roll物理/desired stance未改。开始19:54、预估20:34、实际结束20:30（偏差-4m），结果为回调/观测静态接入完成、无拆分。BF111.05本轮有限复开仍等待Grounded完整真实Gate，不因单元测试或source report ready提前关闭。

- [x] BF115.02d Grounded共享原生刷新与Actor输入：开始2026-09-09 20:30:00、预估结束21:20:00。依赖02c；按本轮C++/AIS已证明的InitializeGrounded→RefreshVelocityBlend/RefreshGroundedLean闭包，先核对Actor实际速度/加速度、坐标基、最大加速度/制动值、pending update和相关性重入阶段，建立站立/下蹲共享半衰期与离开再进入的最小失败。复用现有AlsSourceRefreshNode、SourcePropertyWriteNode和实例状态实现原生调用，输入来自真实Actor数据；再处理已证明重复的发布器/直接consumer，Standing或ControlRig完成范围只有出现具体source反例才有限复开，不扩展Crouching内部图。估时依据为2个原生函数、既有标量/向量数学与实例帧序fixture，参考Standing原生函数及本轮callbacks的实际记录；完成消融、focused/适用回归、公开uasset生成、同入口编译和声明式Gate后，再登记完整Grounded真实动作验收。

  - 02d源/失败闭环2026-09-09 20:38:00：fresh C++ RefreshLocomotion按上一帧实际world velocity差分，底座未更换且使用relative rotation时先旋转旧速度；pending update或Delta<=1e-8时加速度归零。GetAccelerationAmount按acceleration·velocity选实时加速/制动上限，完整三维clamp后再投影。UE FVector Normalize的平方阈值1e-8 cm²换算为target 1e-12 m²，现值正确，保留；DamperExactAlpha实际使用InvExpApprox，复用项目已有同公式。共享stance反例23.935s FAIL于下蹲改用另一份raw blend，converter初始化映射0.260s FAIL于Pose透传；旧测试只验证独立Actor公式。直接GetControlRigInput源边读取同一GroundedState的forward/backward，现独立逐帧blend缺少relevance/reentry合同，纳入此输入消费边界；不展开CR算法或Crouching内部图。下一步完成真实图fixture、共享刷新与Actor输入，全部静态之前不启动runtime。

  - 02d阶段性结果2026-09-09 21:06:00：Grounded原生实例fixture覆盖两stance、两帧不相关、重入只初始化VelocityBlend、加速/制动、pending Lean及关闭refresh/initialize消融；314项目测试36.375s PASS，公开uasset生成81.553s、安装0.566s、编辑器零修复0.539s，同入口双example构建122.422s。随后用户指出RON里的bevy_als为项目名，已以0.300s失败映射自测复现该不必要应用依赖；沿现有als-ext→als-core依赖迁移节点与原有fixture/纯数学，不新增依赖或函数注册框架。该源函数类型引用只出现在Grounded根及Standing根/Idle共3个RON；此处有限迁移其目标类型归属，Standing动作算法与全部已完成运动Gate保持原边界，必须由修改后的公开converter重生成两scope，不能手改类型字符串。
  - 02d底座前提纠正：fresh B_Als_Character默认实例38.092s确认bIgnoreBaseRotation=True、bStayBasedInAir=False；UE SaveBaseLocation因此以bRelativeRotation=False保存底座，RefreshLocomotion的旋转补偿分支在当前ALS配置不执行。先前base旋转反例只验证通用条件，未验证该角色配置，不能当作当前source合同。以实际默认值重写同一fixture后22.163s FAIL于不应出现的旋转加速度；已删除新增底座缓存/旋转分支，只保留实际world velocity的跨帧差分和pending/零Delta边界。原始导出未改；后续回归以此配置约束为准，尚未执行runtime。

  - 02d静态完成2026-09-09 21:24:00：迁入als-ext后的4项原生图fixture31.612s、310项目回归68.130s PASS，真实速度差分/忽略底座旋转/零Delta最终fixture27.410s PASS；C++数学原样归入als-core供Actor和节点共同使用，无新增依赖。公开Grounded/Standing重生成83.504s/292.961s，138文件逐一对照确认本次类型迁移仅3个RON变化，安装1.351s；生成树已无bevy_als::引用。实际target_game/play双example142.104s、29图独立spec加载3.168s、编辑器255文件零修复0.574s、Grounded/Standing声明式及22/62源闭包5.491s/5.574s全部PASS。旧CR审计要求独立组件，已改为共享实例字段读数，并由自测拒绝raw global/旧组件；源姿态算法未改。删除不适用的底座状态、重复Actor blend和3个项目注册点，撤掉无必要的manifest图列表扩张；原生refresh/initialize消融仍保留。开始20:30、预估21:20、实际结束21:24（偏差+4m），结果为共享刷新/Actor输入静态接入完成；本段未启动runtime，BF111.05与BF109.10有限复开仍待下面真实Gate。

- [x] BF115.02e Grounded真实分支与生命周期Gate（后置至AB_Als_Crouching之后）：原准备开始2026-09-09 21:24:00、原预估结束22:14:00。依赖02d及Crouching模块Gate；恢复时重新登记开始/预估结束，不沿用本轮时间点。以当前22可达source graph、6 Pose图、Standing/Crouching/两条stance-change/Roll、Reset/Stop/Roll-exit事件和共享VelocityBlend/Lean为完整Grounded闭包。先核对既有grounded公共场景是否读取当前生成FSM、实际实例及source片段，修复场景中已证明过时的mirror读数；用最小fixture验证驱动/覆盖后，经静态审计与实际入口预构建运行startup和一次连贯Grounded场景。必须覆盖站蹲往返、移动条件、Roll退出按执行时stance选择右侧片段、重置/停止次数及最终稳定姿态，并记录root/局部/final pose、共享读数、曲线/clip/weight和时间守卫。估时依据为已闭合的源/静态合同、既有grounded场景和source窗口/trace/watchdog，参考Standing动作Gate的增量构建及场景收口记录；未通过时只修首个source链差异，不把Crouching内部运动或CR其它算法提前并入。
  - 用户新增真实症状2026-09-10：下蹲过程中角色高度短暂抖动；下蹲结束后出现一帧站立姿态，站立结束后也出现一帧下蹲姿态。这是待定位的视觉现象，尚不能推断actual stance或物理状态确实反跳。沿用户既定顺序，在03e收口后立即纳入本项站蹲联合闭包；需逐帧同时记录capsule/Actor高度、实际stance、生成Grounded状态/权重以及pre-rig/root/final pose，先用fresh source和最小反例确认首个差异。
  - 用户追加真实症状2026-09-10：下蹲后进入瞄准，角色朝向未跟随瞄准方向。02e补充真实键鼠的“下蹲完成→按住瞄准→持续转动视角”链，联合记录actual stance/rotation mode、实际Crouching Rotate状态/权重、源RotationYawSpeed及Actor yaw；先定位切换链与输入消费，不能仅因症状复开03d已通过的独立Rotate算法。
  - 用户进一步给出最小操作顺序2026-09-10：先按住瞄准，再按住前进，然后旋转视角；Standing与Crouching都没有正确跟随。02e以这条完整真实输入顺序作为共享Actor朝向回归，两种stance均需对照UE AAlsCharacter实际grounded rotation分支、输入/视角/速度/target yaw与最终body yaw；不把故障预归因于Crouching Rotate图。已完成Standing范围须在fresh source与真实调用链共同证明根因后才有限复开。

  - 新段前复盘：19:22起四段分别完成源闭包、生成数据、实际回调/观测与共享原生刷新；旧Gate主要漏检生成图实际执行与Actor镜像分离，另两项错误前提已由真实角色默认值和用户的包归属反馈纠正。当前剩余是现有场景能否证明同一生成图中的源分支与事件，不再新增状态机或运行时补偿；先闭合驱动/读数再真实运行，避免用反复试跑探索算法。

  - 用户顺序调整2026-09-09 21:28:00：Grounded真实Playground验收移到AB_Als_Crouching之后。当前只完成现有driver/reader入口定位，未修改场景或启动runtime；Grounded静态结果1598c52保留，BF111.05及BF109.10有限复开仍未关闭。原准备开始21:24、原预估22:14、本轮准备结束21:28（偏差-46m），结果为用户要求的依赖后置，Gate仍待验收；接着进入Crouching，之后再恢复本项。

  - 02e恢复/拆分2026-09-10 04:45:21：03e已提交d6127a8，恢复用户指定的Grounded联合闭包。新增症状包含共享移动瞄准与站蹲切换两条不同真实链，不能沿用原只做runtime的估时；先完成以下共享朝向source/static段，再按证实的剩余切换边界登记后续段，最后统一真实Gate。三个用户操作复现及原Roll/站蹲往返/移动/回调目标全部保留。
  - [x] BF115.02e1 共享移动瞄准朝向source/static闭环：开始2026-09-10 04:45:21、预估结束2026-09-10 05:30:21。先公开重读22个Grounded可达source子图、核对本地AAlsCharacter::RefreshGroundedRotation/RefreshGroundedAimingRotation与实际输入/视角/速度/rotation_response执行路径，按用户“按住瞄准→按住前进→持续转视角”的两stance最小失败定位首个差异。只有source/target差异证明根因后才有限复开对应已完成Actor边界，复用既有输入和角度数学完成修复/消融/focused静态；不靠手工图、增加平滑或放宽跟随阈值。估时依据为当前已定位的两处源函数和单一目标rotation_response、既有输入系统fixture、Grounded22图公开转换约82秒及本轮入口构建约63秒；该段不单独启动runtime，联合场景仍由02e总Gate收口。
    - 02e1源差异2026-09-10 04:57:00：MCP健康检查2.661s明确拒绝连接；公开Grounded转换115.119s PASS（首次仅缺少隔离安装目录，在原流程要求处建目录后执行），原始导出保持独立。fresh CPP确认Moving Aiming先SetTargetYawAngleSmooth(view,1000°/s)→DamperExactAngle(0.1)→Constrain→必要时smooth target=view，target顺序相反且错误覆盖view目标。沿唯一rotation_response与角度helper调用链有限复开BF102.11，最小fixture覆盖实际aim/input状态系统、Standing/Crouching、小偏角追随和大偏角限制。相同函数的静止/空中Aiming目标重置与限制顺序、175°反向约定和InvExpApprox数学是直接共享边界，予以核对；不展开Air动画图、Roll或其它角色生命周期。接下来先确认旧实现稳定失败，再修改该单一共享消费者。
    - 02e1完成2026-09-10 05:32:13：初次fixture缺Overlay/StateRequest，修正实际输入系统前提后17.836s稳定FAIL，Standing/Crouching两角色30°输入的body/target均为0。现保留view目标，先按source推进/阻尼Actor再限角，发生约束时重置smooth target；静止/空中Aiming直接设置view目标，空中不重复积分grounded动画曲线。沿本次CPP/AlsRotation/AlsMath与UE InvExpApprox事实复用已有逆指数及175°反向约定，替换旧精确指数预期为源实算样本；没有新状态、参数、依赖或动画资产修改。六项逐一消融均在12–13s稳定失败（取消额外平滑、空中目标、曲线阶段、反向约定、原生阻尼、零速语义），恢复后240 core/290项目测试46.080s PASS，保留1项既有ignored；target_game/play构建134.601s、6图fresh spec3.441s、30文件编辑器零修复0.299s、完整声明式2.112s PASS，22源子图/30生成RON与新转换逐文件一致。开始04:45:21、预估05:30:21、实际05:32:13（偏差+1m52s），结果为Aiming调用顺序与数学的source/static闭环；BF102.11仍等待02e联合runtime关闭。
    - 02e其余已知边界：本轮fresh CS_Als_Default另确认Character MovingSpeedThreshold=50cm/s，Actor当前moving判定却只用speed>0.05m/s；完整source条件为(has_input且speed>=1cm/s)或speed>50cm/s。该低速/松键条件不同于本段已修复的持续输入目标覆盖问题，必须在02e后续共享状态/切换闭包核对，不能据本段测试宣称已解决；动画实例的同名moving及其150cm/s设置须按实际CPP消费分别取证。用户三条切换/移动瞄准症状、原Grounded所有分支与事件仍未通过联合Gate。
  - [x] BF115.02e2 站蹲切换时序、高度与共享低速状态闭环：开始2026-09-10 05:34:45、预估结束2026-09-10 06:24:45。依赖已提交02e1（ffc50a0）；沿fresh Grounded22图/两条stance-change sequence/CF_Als_StanceChange、AAlsCharacter→ACharacter/CMC的Crouch/UnCrouch调用和当前capsule/mesh写回，分别核对末帧转移、实际stance更新、base/mesh高度补偿及低速moving条件，先留下用户闪帧/高度症状的最小失败或可重复逐帧观测。只修已证实差异，转换相关修复仍经公开uasset重新生成；原始导出、原Playground和物理时钟不改。估时依据为现成22图静态闭包、两条短sequence、既有双角色capsule事务fixture及约一分钟入口构建；source/static与必要消融完成后，再登记完整grounded公共场景接入/运行段，最终包含已修复的两stance移动瞄准与原Roll/回调目标。
    - 02e2源/最小失败闭环2026-09-10 06:25:12：fresh Grounded两条站蹲序列bLoopAnimation=False且终端auto rule交叉时间0，已安装目标却为True；源简写结构省略False，而完整反射字段保留False。最小合同0.302s稳定FAIL，按CPF_Edit补回省略字段、排除运行时字段后0.297s PASS，converter0.551s/renderer0.278s自测PASS。公开Grounded重读74.812s仅改变两张状态图的loop字段；同一读取器影响Standing四个启动/pivot状态内16个相同播放器，fresh Standing265.634s证实仅这四张图loop字段变化，Crouching无受影响播放器。仅有限复开01c/02b的这六张生成图属性边界，旧Gate漏检原因是同一编译结果往返未约束省略的默认值；接下来只安装已属原owner的生成RON并复验该闭包。
    - 高度链同次取证：fresh B_Als_Character41.889s确认胶囊90/56cm、mesh相对Z=-92cm；CMC在Walking时设置bCrouchMaintainsBaseLocation=True，Crouch移动胶囊中心并同步调用ACharacter::OnStartCrouch将mesh相对位置加34cm，UnCrouch恢复默认位置。target只移动胶囊，avatar固定-0.92m；双角色反复切换最小fixture33.983s稳定FAIL于mesh世界原点-0.36m（source为-0.02m）。新增实际stance变化/新avatar时的绝对高度补偿，放在图/Transform传播前，21.054s fixture PASS，包含静止时不覆盖其它局部位移；本次未修改Tnua、物理时钟或Rig算法，后续仍需消融/回归和联合runtime。共享低速moving条件尚未处理。
    - 02e2静态推进2026-09-10 06:38:00：只安装原owner内六张生成图；29个Pose图fresh spec3.840s、Standing完整审计4.484s/Grounded2.377s与编辑器修复/复查通过。补偿/生命周期两项消融16.875s/17.072s均FAIL，分别复现34cm下沉与静止局部位移被覆盖。fresh CS重读36.659s生成独立target阈值配置(0.01,0.5)m/s；共享低速反例24.484s先FAIL，按源input/hasVelocity/moving三者关系修复后20项旋转/胶囊回归18.178s PASS、1个既有ignored。仅新增离线数字配置，复用现有native读取/单位转换/参数外的编译常量，无运行时解析或新增状态；源曲线旋转仍只在非moving时应用。单位转换消融0.301s FAIL，剩余执行消融与整合回归/入口构建继续；真实Grounded Gate尚未开始。



    - 02e2静态Gate完成2026-09-10 06:42:15：去掉输入moving支路13.723s FAIL于1cm/s边界，合并hasVelocity与moving13.690s FAIL于低速velocity yaw保留；恢复后241个als-core与291个项目测试38.392s全部PASS、1个既有ignored，target_game/play真实入口预构建134.003s PASS。源False字段/单位转换/网格补偿的必要性均有失败对照，未添加状态或更改物理/IK。开始05:34:45、预估06:24:45、实际06:42:15，偏差+17m30s；本段完成source/static，不宣称用户症状已通过真实验证。01c/02b仅循环属性的有限复开已关闭，相关历史runtime边界继续留待02e联合Gate。

- [x] BF115.02e3 Grounded联合场景与真实Gate：开始2026-09-10 06:42:30、预估结束2026-09-10 07:37:30。依赖02e1/02e2；复用当次22个源子图、6个Pose图、Standing/Crouching已通过的独立动作以及真实FSM/clip/回调读数，扩展当前只覆盖Roll退出的grounded场景，覆盖静止双向站蹲、移动时站蹲、两stance按住瞄准→前进→旋转视角，以及Roll退出时的实际stance/右侧片段与Reset/Stop回调。先修复场景/观测的最小反例并完成消融、声明式合同和同入口构建，再startup与一次完整Grounded运行；逐帧采集capsule/mesh高度、实际状态/权重、源clip时间/pose/curve、Actor yaw和公共pre-rig/final守卫。不因独立动作PASS略过用户联合复现，也不复开未证实差异的Rig/物理。估时依据为已闭合22图、旧Grounded单Roll入口、现有窗口/实际图/源码回调探针与最近134s构建/38s回归记录；本段只补同一Playground输入与可验读数，遇真实差异按首条source链修复后继续。
    - 02e3 source准备2026-09-10 06:52:37：fresh AlsAnimationInstance.cpp明确bMoving复制Actor值，bMovingSmooth另用AIS150cm/s；当前provider将两者都写为后者。沿Grounded切换条件的直接输入链，仅复开01g/02e2的bMoving发布边界；旧Gate只测Actor旋转和有输入场景，未验证0.5–1.5m/s无输入区间的图字段。先留下该绑定反例，再复用已生成的Actor阈值，moving_smooth维持源语义。fresh PlayTransitionRightAnimation还确认按回调实际Stance选资产；旧Roll观察器只接受Standing的反例22.908s FAIL，后续读实际请求/片段与source回调，不能固定初始姿态。
    - 02e3静态/场景闭环2026-09-10 07:12:40：bMoving/bMovingSmooth绑定反例20.041s FAIL后统一Actor/图输入判定；Roll探针按真实回调保存的Stance核对片段与slot，并补Stop调用计数。grounded公开入口增加静止双向、移动双向和双姿态移动瞄准六窗口；预备条件来自实际FSM/速度，单次蹲键、先aim后前进再45度连续鼠标输入，不重置动作间的身体/物理。逐帧记录两切换序列时钟、actual state/weights、mesh/capsule高度、输入/yaw、Reset/Stop/Roll请求与clip生命周期。高度/时钟/aim顺序消融18.438/18.111/17.858s均FAIL，恢复14项17.354s PASS；完整241个als-core和294个项目测试28.205s PASS，1个既有ignored。同入口重建中，未启动runtime；下一步完整静态后startup与整条Grounded链。
    - 02e3真实Gate前2026-09-10 07:14:00：target_game/play入口129.213s PASS；Standing/Grounded完整声明式4.779s/2.387s PASS，29图spec与六张生成修复资产无漂移，静态闭包仍为62/22源子图。当前开始无driver startup，随后同一原Playground执行Roll退出+六个联合窗口；保留固定watchdog与公共时序守卫，首轮若失败先读现有报告定位。
    - 02e3 startup通过2026-09-10 07:15:14：公开runner19.740s、6张截图，root pose/图求值/公共时序无错误；现按同一构建继续Roll与六个Grounded联合窗口。
    - 02e3首轮真实结果与边界2026-09-10 07:17:19：runner48.414s完成，四个站蹲窗口时钟/高度/终态已覆盖；Roll回调未出现，Standing移动Aim因进入台阶后停速/Air而超时，final calf_l在该台阶上触发1.080rad突变，未放宽守卫或修改Rig。截图、pose样本和actor高度证明旧roll_start加持续前进路径进入楼梯；改用既有SPRINT_START原场内走廊并核对后续转向区域，15项含源地形路径/输入测试28.148s PASS。
    - Roll首个source消费者差异：fresh AB_Als native40.368s及本地AnimNode_Slot.cpp确认两个根Slot的bAlwaysUpdateSourcePose=False；UE在普通slot完全覆盖时不更新Source，目标SourceSlotNode无条件先求Source，故Grounded状态机始终相关，FromRoll入口无法在slot退场后重新初始化。现有FSM已实现跳帧后的source重入规则，无需新增影子状态。当前闭包仅延伸到Roll明确依赖的AB_Als PostLocomotion Slot更新边界；Transition同类节点沿同一通用实现，但不展开其它AB_Als功能。旧源/曲线Gate只测混合权重，没有检查完全覆盖时Source是否仍更新；先用真实图cache更新位留下失败，再修复通用节点并复验。
    - 02e3超时复盘2026-09-10 07:57:59：第一轮开始06:42:30、预估07:37:30、实际复盘07:57:59（偏差+20m29s），完整Grounded Gate尚未通过。已暂停新实现/runtime并重审22个Grounded源子图、首轮实际报告与8图Root Slot fresh合同；四项站蹲高度/不循环/终态已覆盖，漂移为旧输入路线进入楼梯，以及全权重Slot未停止Source更新导致Roll重入漏触发。不得据台阶上的final故障修改Rig或降低守卫。真实图cache更新反例41.611s FAIL；复用现有Slot与FSM相关性后四项Slot测试25.797s、241 core/337 ext/295项目回归49.256s PASS（1个既有ignored），target_game/play双example114.976s PASS。公开Root uasset转换仍在其有界watchdog内，未安装未核验输出。
    - 第二轮纠正与拆分：按上一55分钟窗口1.5倍登记开始2026-09-10 07:57:59、预估结束09:20:29；拆为02e3a Slot源默认值/生成合同静态收口（07:57:59–08:37:59）与02e3b完整Grounded联合真实Gate及有限复开关闭（计划08:37:59–09:20:29）。前段只补AlwaysUpdateSourcePose两值fixture、消融重复场景验证、同次公开转换/fresh spec/声明式合同；后段保留Roll和六个联合窗口、全部公共守卫与用户复现。目标不缩减，静态闭合后按实际完成时间提前进入后段；未经source证明不展开其它Root功能或后续模块。

    - 02e3a静态收口2026-09-10 08:10:00：Slot两策略真实图fixture37.881s、合并后的15项Grounded/路径检查43.312s PASS；合同/渲染/转换自测0.317/0.321/0.538s PASS。新渲染fixture初次漏填既有provider字段导致失败，补齐后通过，不是生产图错误。公开Root转换在16个依赖中第16项触发600.280s总时限；保留当次15份不可变导出，续读最后一项71.899s完成9 AnimBP/256 RON闭包，unsupported 0。Standing/Crouching/Grounded输出完全一致；本项Root输出仅显式写出原默认false，沿原256文件owner仅安装该一张图，其他待办模块15处差异保持隔离。通用审计新增明确source-asset范围，仍要求完整source记录/编译就绪并逐字核验所选owner所有RON；默认全闭包行为不变，范围消融self-test0.309s通过。57个实际Pose图fresh spec3.428s、同入口双example134.775s PASS，SourceSlot新默认ABI加载已验证；source/static声明式及Root编辑器复查进行中，尚未启动第二次真实运行。

    - [x] BF115.02e3a Slot与联合场景静态Gate完成：开始2026-09-10 07:57:59、预估08:37:59、实际结束08:11:29（偏差-26m30s）。Standing/Crouching完整合同5.050/2.943s PASS；Grounded加明确Root owner源闭包、Root编辑器/位置后的最终合同4.792s PASS，Root编辑器修复0项，57图fresh spec通过。无新增运行时状态或FSM；共享moving阈值、源Slot相关性、场景逐帧检查均有旧行为会失败的最小边界。格式整理中产生的无关改动已删除，并以rustfmt规范结果一致验证未改变代码。原22/62/33源闭包保留，Root只覆盖Roll必需的1 Pose图/8源图及两Slot实现；剩余Root与其它模块不在本项完成结论内。
    - [x] BF115.02e3b Grounded联合真实Gate：依赖02e3a，实际开始2026-09-10 08:11:29、预估结束08:53:59（沿原拆分的42m30s窗口提前开始）。同入口构建、最小回归、生成与声明式均已闭合；没有外部Playground进程，以下公共startup后运行Roll与六个连续窗口。保留真实source stance选择/完整片段结束、Reset/Stop、双向站蹲canonical、移动瞄准有符号身体yaw和全部公共pre-rig/final守卫，真实通过后才关闭既有有限复开。

      - 02e3b startup通过/动作Gate前2026-09-10 08:16:00：公共startup144.744s PASS，含启动器因源文件格式还原触发的2m06s增量编译；运行6截图，frame p95 13.237ms/max25.517ms、eval p95 0.072ms，faults/graph_errors/temporal为空。末帧已核对为原Playground正常站姿。源闭包及57图spec/三模块声明式无变化，继续同一入口完整Grounded动作Gate，不能把startup当联合复现已解决。

      - 用户追加复现2026-09-10：瞄准时快速左右大幅横向甩鼠标后静止，角色面朝方向有时明显偏离。仍属当前共享Aiming链，补充真实快速反向鼠标输入与停止后的view/body yaw、source允许偏角、曲线和原生Rotate退出读数；先用当次UE公式/生命周期核对，不以视角与身体必须完全重合替代源规则，也不放宽公共守卫。
      - 第二次联合运行结果：runner39.046s，manual91/validator失败；公共pose pass=true，frame p95 15.346ms/max27.418ms、eval p95 0.082ms，faults/graph_errors/temporal均空。四项站蹲canonical/高度/时钟/稳定终态及Standing移动Aim通过，Crouching Aim实际body已随view转45°，但在场景额外蓝色mantle ledge处停止前进，未满足完成前提。旧路径fixture仅核对原始Playground JSON，遗漏同一入口额外生成的ledge；先补这一最小反例。Slot现在确实使Grounded FSM在full权重阶段不更新；恢复时源T2要求FromRoll且非bMovingSmooth/Rotate，而实际恢复时速度位于1.5m/s边界，故进入Standing并清除entry，Roll专属退出仍未覆盖。继续核对这条已命名的实际源入口与Roll速度/权重消费者，不据此修改FSM或姿态算法。

      - Roll源根因闭包：fresh AM_Als_Roll33.803s与本地UE AnimInstance/AnimMontage确认RootMotionFromMontagesOnly、0.1/0.3 HermiteCubic、默认末段自动淡出。UE开始淡出时清除RootMotionMontageInstance，后续保留Pose却停止root-motion提取；target roll_drive持续覆盖LinearVelocity到1.5/1.3终点，grounded motor也仍按整段roll.active排除，违反该控制权生命周期。BF102.04.a仅此边界有限复开；先真实系统反例，再复用现有时间与淡出常量，不增加模拟时钟或修改动画位移/scale。源T2入口与FSM条件本身无差异，不改生成状态机。

      - 本轮静态修复2026-09-10 08:46:00：真实roll_drive47.158s与grounded motor27.662s先分别FAIL于淡出后仍覆盖速度/未恢复制动；复用现有Roll时间和0.3s淡出常量区分root-motion有效期，帧内跨入淡出的最后一次提取保留，之后Pose继续而普通motor接管，两个实际系统测试28.994s PASS。无新增状态、时钟、位移缩放或姿态分支。路径反例先因World资源初始化链的调用错误18.748s编译失败，纠正后33.072s稳定拒绝漏算入口蓝色ledge的旧路径；另一候选方向被原场地楼梯静态检查拒绝，最终将Grounded起点在原平地沿Z移2m并保持原左右45°输入，原地形及实际spawn函数产生的Mesh边界均通过。15项focused21.900s、242 core/295项目回归2.126s（1个既有ignored）、完整Grounded+Root source/editor/位置声明式6.398s PASS。复用已有720°/s三角鼠标输入的4秒快速反向段，添加两stance甩动停止窗口；零移动、实际Aiming、source Rotate退出/曲线归零与50°源停转阈值共同定义终态，公共yaw幅度/pose守卫独立保留，未以身体必须对齐view改算法。实际入口重建结束后重新startup与完整九窗口（Roll+八个联合case）。

      - 修复后真实动作Gate前2026-09-10 08:48:00：target_game/play入口137.401s PASS；新生产版本startup19.795s PASS、6截图正常，frame p95 13.567ms/max17.060ms、eval p95 0.082ms，无fault/graph/temporal。完整静态闭包保持通过，下面同一入口运行Roll和八个联合输入窗口，覆盖新增快速左右甩动后的实际停转终态。

      - 九窗口真实结果/观测修正2026-09-10 09:02:00：runner42.301s中manual0、九窗口全部覆盖且pose pass=true，无fault/graph/temporal，frame p95 18.972ms/max26.655ms、eval p95 0.090ms。Roll实际请求Standing Right从0.2到2.333333完整结束，双向切换canonical0.00379/0.00513s；两移动Aim完成45°跟随，快速甩动停止后stand/crouch的body-view偏角分别约14.06°/3.03°，曲线0、Rotate已退出，满足当次source50°停转阈值。独立validator仍1，原因是把有符号Actor响应当动画曲线、Roll缺少声明终态，以及single clip canonical未采集。BF111.04公共观测有限复开；native-yaw自测0.573s和无group实际collector20.549s先FAIL，新增显式locomotion yaw来源后仍强制signed trace/实际状态/view/零curve，原animation-curve分支保持原检查；单片段canonical独立于多变体group并校验实际样本距离。自测0.575s、34视觉测试21.898s、15 Grounded1.553s PASS，四处完整结构初始化遗漏在7.694s编译报错后补默认来源。target_game/play63.253s PASS；生产算法/图未再修改，沿用已通过的19.795s startup，接着相同九窗口最终报告复验并把定时截图扩展到动作结束。

      - 用户取证偏好：后续非必要不截图；瞬时问题以逐帧数值轨迹、实际执行顺序和source/target合同定位，该偏好随后明确扩展为验收也不截图；下轮工具以零截图模式保留数值验收。
      - E3c静态推进2026-09-10 09:24:30：零截图validator反例0.559s FAIL；readiness fixture初次类型误用7.657s编译失败，纠正后28.235s稳定FAIL，修复后35项视觉测试21.100s及独立自测0.567s PASS。CaptureCount默认0，定时/动作/teleport/fault/graph-error截图全部关闭，数值样本、窗口与失败报告保持强制。fresh UE Actor/Movement/Mesh与实际Tnua依赖进一步证明Roll停止后的motor速度在当前Update已写入LinearVelocity，而动画provider仍读上一PostPhysics快照；BF104.02只按此采样阶段有限复开，复用现有AlsActualMotion，不改物理时钟或图阶段。

      - 02e3b估时边界前复盘2026-09-10 09:12:41：开始08:11:29、预估08:53:59、实际复盘09:12:41（偏差+18m42s），完整Gate尚未完成。已暂停实现和新的runtime，重读E3整条source/consumer、两次完整报告和实际调度：八个站蹲/移动Aim/快速甩动停止窗口稳定通过，姿态/性能守卫无异常；最新43.411s运行只缺Roll入口（manual91），独立观测字段错误已消除。根运动淡出后控制权修复有两实际系统反例且首轮能进入Roll，但不同帧划分下图读bMovingSmooth仍true、运行后速度已1.315m/s，FromRoll被Standing入口清除。target完整图在PrePhysics、ActualVelocity/ActualMotion采样在Physics Last之后，已有前后阶段差异；尚需比较UE montage提前更新/Actor tick/动画更新的真实顺序，不能仅根据后采集速度把前一阶段值判成Bug或改阈值。
      - 纠正与重估：按上一42m30s窗口1.5倍登记下一轮开始09:12:41、预估结束10:16:26；拆为02e3c无截图验收支持与Roll源帧序/最小边界（09:12:41–09:42:41），02e3d静态收口与完整无截图Grounded Gate（计划09:42:41–10:16:26）。用户明确取消所有截图，包括验收；后续只采集逐帧数值、真实状态/clip/曲线/pose与时序守卫，不调用截图。已完成的八窗口和所有原目标保留，未来Locomotion/Layering/Head/CR/Root模块仍按依赖顺序继续；本检查点不是停止或等待许可。

    - [x] BF115.02e3c 无截图验收与Roll帧序静态收口：开始2026-09-10 09:12:41、预估09:42:41、实际结束09:31:00（偏差-11m41s）。同帧速度交接27.397s稳定FAIL后，将现有动画快照放到Gait/Standing/Root provider之前、读取Tnua已提交的LinearVelocity；297项项目回归24.224s PASS，沿用已通过的242 core/337 ext回归。没有新增运动状态、调度器或物理时钟，旧的上一帧速度路径即失败消融；三维、停速yaw与隔离fixture保留。Grounded+Root/Standing/Crouching完整静态7.148/5.849/3.460s PASS，实际target_game/play双example69.217s PASS。零截图的正反例与失败报告合同已固化；本段只完成静态，全部有限复开仍待真实动作结果。
    - [x] BF115.02e3d 无截图Grounded联合Gate及共享速度回归：实际开始2026-09-10 09:31:00、预估10:04:45（沿原33m45s窗口提前开始）。先运行当前生产版本startup，再完整Roll+八联合窗口；同帧速度生产者变化同时用既有standing_response与crouching覆盖移动/停止/切换，所有公共pose/时序/源片段/终态阈值保留，CaptureCount=0。实际通过后关闭对应有限复开并提交；不展开后续模块来绕过本Gate。
      - 入口前复审2026-09-10 09:33:30：新增root-motion有效期守卫漏接自动落地启动函数返回的RollLifecycle；原落地/输入阻塞fixture补实际首帧速度后28.697s FAIL，保留该返回值后3项Roll测试20.088s PASS。仅修本轮新分支遗漏，未添加状态/补偿或展开其它动作；同入口增量构建进行中，尚未启动runtime。
      - 无截图动作Gate前2026-09-10 09:36:30：最终同入口构建64.152s PASS，静态修复已提交dd60386；当前生产版本公共startup15.835s PASS，screenshots/images均空且无PNG，faults/graph_errors/temporal为空。现按同一二进制完整执行Roll+八个联合窗口，保留source canonical/完整结束/回调与全部数值守卫。
      - Grounded真实通过/共享回归前2026-09-10 09:38:00：runner42.291s、manual0/validator0，九窗口与十五项覆盖全部PASS、0截图/0PNG；frame p95 15.160ms/max28.019ms、eval p95 0.070ms，faults/graph_errors/temporal为空。Roll Right真实片段完整结束，canonical误差0.000223s；双向站蹲误差0.000342/0.003905s，实际终态/回调/高度和两stance移动Aim、快速反向停止均通过。同入口继续既有standing_response/crouching，验证共享采样阶段对移动、停止和姿态分支的影响；未新增生产修改或放宽守卫。
      - 共享Standing回归/下蹲回归前2026-09-10 09:40:00：standing_response20.073s、manual0/validator0，九窗口通过且0截图，frame p95 18.058ms/max27.668ms、eval p95 0.071ms，无fault/graph/temporal。源生成闭包与生产二进制无变化，继续既有十二窗口crouching完成同帧速度改动的另一直接消费者回归。
      - 02e3d完成2026-09-10 09:42:30：crouching40.411s、manual0/validator0，十二窗口/零截图PASS，frame p95 13.922ms/max28.831ms、eval p95 0.070ms，无fault/graph/temporal。Grounded真实交接帧在speed=1.494529m/s时发布moving_smooth=false并触发一次Roll Right请求，完整片段及canonical通过；三个动作场景共三十窗口与startup全部通过。02e3d开始09:31:00、预估10:04:45、实际09:42:30（偏差-22m15s）；原E3首轮06:42:30→07:37:30到最终结束偏差+2h05m，前两轮失败、重估和拆分证据保留。02e/02e2/02e3及01g本次bMoving发布边界关闭；BF102.04/.04.a/.11、BF104.02、BF109.10、BF111.04/.05、BF114.03b仅本轮明确复开的消费者边界关闭，不代表后续CR/Root整体重验已完成。主要纠正为源Slot相关性、Roll淡出控制权、同帧已提交速度、真实场景路径与公共观测合同；未新增补偿、平滑、阈值放宽或依赖。接下来按顺序开始Locomotion源闭包。

- [x] BF115.03a Crouching源闭包与首个转换/Actor差异：开始2026-09-09 21:31:00、预估结束22:21:00。依赖已完成Standing与Grounded静态02d，按用户新顺序执行；先从本地AB_Als_Crouching uasset公开重新读取完整递归图及所需BlendSpace/sequence、C++和AIS事实，对比当前生成资产与Actor的输入、曲线、时钟、状态/回调及真实读数。只把当前源证明的差异纳入后续实现，不沿用旧手工图或Actor结论。估时依据为已归档同图30项递归合同、当前66个生成RON/2个FSM、已有共享时钟/实例节点以及Standing同类公开转换约293秒；本项先交付完整源/静态差异与可重复的最小失败，具体修复按闭包另立不超过60分钟的验收段。Crouching完成后返回02e执行Grounded真实Gate，再继续Locomotion；已通过的Standing清理/Turn/曲线和Grounded共享刷新不因猜测复开。

  - 用户补充2026-09-09 21:34:00：已确认按下蹲后Standing表现异常，并明确要求继续AB_Als_Crouching，完成后在02e一起验收。因此02e必测“下蹲→恢复站立→移动”的Standing回归，记录实际FSM/clip/phase、共享VelocityBlend/Lean与退出清理；当前只定位相关writer/driver入口，未单独展开Standing修复或运行场景，Crouching仍为当前任务。

  - 03a源/差异闭环2026-09-09 21:54:00：MCP健康检查2.661s明确拒绝连接，公开Crouching uasset转换48.195s完成、unsupported=0；147 raw graph中可达33项（12 Pose图/2 FSM/16条件/3函数）、217节点，source审计覆盖96 Pose节点/76 Pose边，66个生成RON。12图当前spec加载3.274s通过，同次源审计1.068s稳定FAIL于12个已安装Pose图：旧Actor同步相位/缺少当前曲线与初始化策略、以及Idle原生刷新仍未迁入最新通用节点；全部新输出仍在隔离目录。可达播放资产包括六条1.133333秒Crouch Walk、Walk Pose、Crouch Pose、左右1秒Rotate及共享Lean BlendSpace（5个additive样本），不把ExecuteUbergraph镜像重复计入。
  - 原生/Actor边界：fresh AIS_Als_Default34.962s和Walk曲线34.529s确认AnimatedCrouchSpeed=1.5m/s、Stride使用同一CF_Als_StrideBlendAmount_Walk；现有生成曲线逐字段0.297s一致（0/0.583/1.5m/s对应0.2/0.55/1.0），可直接复用。前两次AIS读取使用错误的资产名/位置，均exit3且无source输出；已根据实际asset_path纠正，失败未充当事实。C++ RefreshCrouchingMovement按mesh world scale归一speed、采样源曲线、play rate限1e-4..2；当前Actor使用独立采样表、角色Transform scale、私有FSM/phase/curve/turn与dynamic推算。16个函数调用点包含六处SetHipsDirection、两个Movement刷新、Move进入/Idle退出Stop、Stop进入PlayStopTransitionAnimation、StopQuick及Idle/Rotate原生调用。实际观测仍读取手工graph handle和简称；Crouching rotate bool被写为F32；PlayStopTransitionAnimation dispatcher未注册；CrouchingQuickStop仍走旧slot采样。以上为后续输入/生命周期/直接资产消费核对边界，尚未判定其中哪项导致用户的下蹲后Standing回归，不据猜测修Standing。
  - 本段开始21:31、预估22:21、实际结束21:54（偏差-27m）；结果为完整源闭包与可重复安装差异，未实现Actor或启动runtime。先接入同次converter生成数据，再闭合Actor/原生函数/实际读数；Crouching之后按用户要求执行包含下蹲后Standing回归的Grounded联合Gate。

- [x] BF115.03b Crouching生成执行数据静态接入：开始2026-09-09 21:54:00、预估结束22:24:00。依赖03a；同次公开converter已应用Standing/Grounded阶段修复，可直接生成完整66文件闭包，仅安装此scope的12个差异，保留原始导出和原AB_Als owner，不手改RON。验证同帧序列/曲线/组时钟、bAlwaysResetOnEntry和原生节点类型，建立当前Crouching生成树的声明式manifest，完成编辑器/位置/fresh spec/完整源编译一致性及适用focused Gate。估时依据为已得到的48秒公开转换输出、12个文件差异和此前Grounded资产接入记录；本段只做生成数据静态迁移，Actor发布/回调/实际观测继续下一段，真实运行仍待静态闭环。

  - 03b静态完成2026-09-09 22:00:00：既有installer按原AB_Als owner安装同次公开输出66文件，12张Pose图变化，0.558s完成；原始记录、其它图和owner内容保持。当前图已改用源组时钟、同时间曲线、实际初始化策略及als-ext原生Idle/Rotate节点；源审计对旧12图的1.068s失败作为旧数据消融，fresh 12图spec2.139s和完整声明式33源子图/66文件审计2.383s PASS，66文件编辑器零修复0.526s，位置/资产边界与相关工具自测通过。12项SourceSequence回归3.995s、实际同步权重/缓存/marker fixture1.309s PASS；本段仅资产/manifest，复用已验证的target_game/play加载器，无需再次全构建。开始21:54、预估22:24、实际结束22:00（偏差-24m），结果为生成执行数据静态迁移完成；Actor发布、回调和观测尚未闭合，未启动runtime。

- [x] BF115.03c Crouching原生调用与Actor消费者：开始2026-09-09 22:00:00、预估结束22:50:00。依赖03b；沿33源子图的16处函数调用、AIS步幅/速度、真实Root曲线和两个FSM，先用最小反例约束typed bool、mesh scale/原曲线、PlayStopTransitionAnimation与实际FSM reader，再复用als-ext实例节点和现有通知/Slot执行源调用。删除已证实重复且无生成图消费者的私有Crouching状态/phase/曲线/动态推算；对仍有消费者的字段先追踪，不以清理名义改变相邻模块。Stop/QuickStop直接选择的Crouching Left/Right Transition资产只按本轮C++/AIS核对并经现有工具生成采样配置；核对实际Root曲线驱动旋转与foot input，保留原Standing及Grounded已通过边界。估时依据为两FSM/一个movement组、既有原生/通知框架、12图静态闭合及Grounded同类Actor迁移记录；完成消融/focused/回归/重新生成及同入口静态后，再登记Crouching真实Gate。用户要求的下蹲后Standing恢复检查保留于后续Grounded联合验收。

  - 03c最小失败2026-09-09 22:06:00：当前实际writer/reader/dispatcher的三个反例同轮稳定FAIL，另26项相关测试PASS：rotate bool写为F32、Crouching telemetry handle不属于当前角色引用的生成根图、PlayStopTransitionAnimation未识别。旧测试验证了私有phase和手工图回显，未约束这些生成入口；接着按当次源函数及数据类型修复，并追踪尚存Actor发布/curve/slot消费者。未运行Standing或Grounded场景；按用户要求，修完Crouching后在Grounded联合Gate复测下蹲后的Standing恢复。

  - 03c根因反例2026-09-09 22:15:00：前三项修复后PASS；新增两个真实调用反例34.434s稳定FAIL、另29项PASS：旧步幅表在源0.583m/s关键点得到0.550018，旧turn_pin_driver在Crouching Aiming中将已由源请求排入的montage清空。修复收敛到现有GeneratedAbAlsProvider的纯输入、实际FSM通知和Root曲线消费；删除私有Crouching FSM/phase/曲线/lean及重复Turn检测，不新增runtime抽象。RefreshGroundedMovement原有真实provider已同时服务两stance，登记归属；共享驱动保留montage推进/采样，动作选择由已生成的原生回调负责。QuickStop两侧Transition直接采样所需源记录纳入本段；旧slot左右都已使用StandPose frame0，不能把该参考姿态称为Bug。

  - 03c完成2026-09-09 22:30:00：Crouching Actor删除私有FSM/marker时钟/曲线采样/Lean与动态检测，只保留真实Root曲线到脚锁/旋转的投影；纯stride/play-rate输入复用已有源Walk曲线并使用mesh world scale，动作由当前生成图的原生回调和通知分发。实际reader绑定源完整标题，读取两个FSM的当帧缓存、真实源同步相位及共享实例；场景已去除私有movement回显验收。旧Turn驱动清空源请求的反例由FAIL转PASS，删除无消费者的检测状态与镜像测试；未新增依赖或runtime层。CrouchingStop进入分发按执行stance选择Right，新增独立计数；QuickStop左右均用原始uasset经既有工具生成的直接montage记录，新增Left、其余17条重新提取不变，参考均按源设置，工具0.562s通过。
  - 验证：8项focused32.255s、288项项目回归18.821s（含18个montage真实79骨姿态与曲线采样）、lifecycle自测0.294s PASS；公开uasset47.539s重新生成，66 RON无差异，installer0.551s。target_game/play两个example67.836s、12图fresh spec1.858s、编辑器66文件零修复0.295s、完整33源图/66文件及声明式合同2.386s PASS。中间spec调用漏设资产根、editor参数误用，分别exit8/2，读报告纠正后通过，未把调用错误归为图故障。开始22:00、预估22:50、实际结束22:30（偏差-20m）；结果为Actor/原生消费静态闭合，尚未运行真实Crouching或Grounded。下一段完成实际片段/场景观测并执行Crouching公共Gate；下蹲后Standing恢复仍由后置Grounded联合Gate验收。

- [x] BF115.03d Crouching真实片段观测与Playground Gate：开始2026-09-09 22:30:00、预估结束23:20:00。依赖03c；沿既有Crouching公共场景核对六向移动、Stop/QuickStop、左右Rotate、Turn/Dynamic的source分支及真实片段/相位/生命周期读数，修复已因迁入生成执行图而失效的旧驱动观测；先最小fixture、同入口增量预构建与静态闭合，再运行startup及一次连贯可见Playground场景。使用原地形/角色/相机，证明实际FSM、根/局部/final姿态、源clip/curve/weight、带符号yaw及时间守卫；失败只沿首个已证实源链差异修复。估时依据为已闭合的33源子图、两FSM/一个Movement组、既有公共场景、67秒增量构建与Standing同类Gate记录；未引入其它模块实现，完成后恢复02e联合验收用户报告的下蹲后Standing异常。

  - 03d场景静态闭包2026-09-09 22:40:00：真实场景最小fixture24.706s稳定拒绝旧手工图路径；已把六向移动、两侧Rotate/90°Turn的观测链改接生成节点、实际Sequence时间和原生FSM退出，去掉requested镜像终态；移动改按源Movement相位0.25比较六个方向，转身/旋转按实际clip time0.5比较两侧。原场景直接写view_yaw，现改经真实look input输入。完整审读确认现有case未独立覆盖Stop/QuickStop、180°Turn和两侧Dynamic；本轮03d先验收现有12个窗口的完整真实消费链，上述剩余源分支拆入03e，Crouching整体仍需两段均通过后才恢复Grounded。拆分结果为原50分钟段仅保留已有驱动/观测的迁移，避免把新场景设计与首轮真实执行混在一起；未启动runtime。

  - 03d真实Gate前2026-09-09 22:48:00：23项Crouching focused 20.790s、33源图/66文件声明式合同2.650s、同一target_game/play入口构建63.735s全部PASS；观测链、真实片段时间和终态最小失败均已闭合。下面先运行无驱动startup，再运行现有六向移动/两侧Rotate/90°Turn的12窗口；03e补齐剩余源动作，Grounded联合回归继续后置。

  - 03d startup完成/动作Gate前2026-09-09 22:49:00：公共startup runner19.361s PASS，6张截图、18个必需骨骼齐全、frame P95=13.509ms、eval P95=0.073ms；已视觉核对原Playground地形、角色和相机，未出现图错误或参考姿态风险。继续同一生成树的Crouching动作Gate，不改阈值或场景环境。

  - 03d真实失败2026-09-09 22:52:00：动作runner72.479s FAIL/exit91，按要求审计了非零退出报告；graph_errors/faults/temporal_anomalies均为空、watchdog未触发，失败为场景超时和Crouching状态覆盖缺失。逐窗口实际trace证明全过程仍执行Standing FSM，Crouching节点完全不可用，不能据此修改Crouching姿态或放宽时间阈值。
  - 03d入口根因/02c有限复开2026-09-09 22:58:00：当前Grounded源与目标均把站蹲入口挂在两个State Alias上；InterruptibleFsm的手动direct_transition已能匹配alias成员，但source_transition_event及source_condition_transition_id仍要求源ID直接等于当前真实状态ID，因此自动条件从Standing永远跳过Crouching Alias。复开02c仅补此自动条件入口与回调；旧Gate漏检原因是alias fixture只调用手动direct_transition。先重新提取本地Grounded合同、扩充现有真实FSM求值fixture证明失败，再复用现有alias匹配修复；Standing和Grounded数值算法不动，完整Grounded联合验收仍按用户要求后置。

  - 03d alias修复静态闭环2026-09-09 23:09:00：MCP仍拒绝连接2.653s，公开Grounded uasset/曲线92.378s重新提取，30个生成RON与安装值一致；因此不改RON。扩展既有真实FSM求值fixture后82.588s稳定FAIL（alias分支没有进入/退出事件）；修复仅删除自动条件对真实状态ID的重复限制，并复用现有direct_transition的alias成员匹配。加入非成员拒绝路径，27项FSM回归61.373s、337项als-ext回归20.512s与288项项目回归39.674s PASS。另实际trace确认旧场景未持续aim_held，已修复输入并把实际Aiming加入六向验收；没有更改角色旋转算法。当前同入口重建中，完成fresh spec/声明式后再启动；02c有限复开待Crouching真实入口证明后关闭，Grounded完整Gate仍后置。

  - 03d alias修复真实Gate前2026-09-09 23:13:00：target_game/play双example110.836s、18图fresh spec3.182s、Grounded/Crouching完整声明式合同2.897s/3.149s PASS。源22/33子图、30/66 RON未变化；本次真实运行只验证已修的自动alias入口及Crouching既有12窗口，不启动独立Grounded或Standing场景。继续startup后执行Crouching，保留首轮失败证据。

  - 03d alias startup后/动作前2026-09-09 23:15:00：新二进制公共startup19.541s PASS，原场景根姿态/骨骼/截图与时间守卫通过；接着运行Crouching以验证实际alias切换和动作窗口。

  - 03d第二次真实结果/02c复开关闭2026-09-09 23:34:00：Crouching runner39.333s完成，未超时；真实Crouching节点、两侧Rotate和90°Turn的活动/完成/Idle终态全部出现，证明alias自动条件入口已修复。02c这次有限复开开始22:58、随03d预估23:20、实际收口23:34（偏差+14m），结果为自动alias条件与成员/非成员/回调Gate通过；完整Grounded依然待02e。Crouching整体未通过：两条侧后固定方向期望仍与源图不符，且Rotate/Turn链错误要求未执行的Movement Apply Additive；另一次final右腿突变发生在Rotate输入开始附近，pre-rig无异常，继续沿真实输入/脚锁链定位，不调整公共守卫阈值。源新核对明确侧后状态是通往侧前的过渡分支，需用Backward→Right/Left历史及实际混合权重验收，不能把135度输入当稳定侧后FSM状态。

  - 03d超时复盘 2026-09-09 23:42:03：原开始2026-09-09 22:30:00、预估23:20:00，本轮在此结束复盘，已超过误差上限且Gate未完成。回读当前条目、22/33源闭包、两次runtime及新fixture证据：主要漂移是必须先修Grounded自动alias入口，以及旧场景把侧后状态当稳定方向、要求非活动Additive链、直接跳变视角。alias已独立关闭；当前只余新场景输入/权重观测的真实闭环，final突变仍由公共守卫验证，后续8个源动作仍为既定闭包。方向/链路反例30.397s FAIL后，新24项Crouching测试24.201s PASS；复用既有源方向历史和连续鼠标驱动，删除旧瞬时视角工具，未改IK算法。
  - 纠正与重估：本轮暂停新的runtime并完成上述复盘；按上一窗口1.5倍规划Crouching剩余总窗口，开始2026-09-09 23:42:03、预估结束2026-09-10 00:57:03。为保持每段可独立验收，拆为03d现有窗口真实Gate（开始2026-09-09 23:42:03、预估结束2026-09-10 00:17:03）与03e剩余8个源动作（计划开始2026-09-10 00:17:03、预估结束2026-09-10 00:57:03）；通过前不进入Grounded联合Gate。当前构建结束后重跑声明式合同，再继续同一Crouching验收。

  - 03d新分段真实Gate前2026-09-09 23:43:00：源方向历史/活动状态权重/连续鼠标输入已接入，24项focused24.201s、同入口构建63.980s与完整声明式合同2.403s PASS。生产动画算法沿用已通过alias startup的版本，本次只变更场景与只读观测；复用本轮已通过startup，执行修订的12个Crouching窗口，仍检查公共final守卫和原阈值。

  - 03d新分段进展2026-09-10 00:01:00：第三次真实runner40.207s FAIL但faults/temporal_anomalies均为空，连续look输入已消除final突变；所有Rotate/90°Turn完成，剩余左侧移动窗口在固定结束时尚差一帧到达完整状态。实际clip权重证明源过渡仍在推进，现改为按实际FSM就绪后完成原采样窗口再推进case，保留总watchdog。Rotate原观测忽略有权重的退出过渡，且固定目标convergence不适用于移动视角；最小反例23.374s FAIL后，改读实际状态权重与sequence时间，完整覆盖退出混合，保留signed yaw幅度并用源角度阈值/Idle/curve0验证终态。24项Crouching回归19.455s PASS；未改生产动画/IK或公共数值守卫，构建与下一轮真实验收继续。

  - 03d窗口复验前2026-09-10 00:04:15：实际FSM就绪窗口及退出混合观测24项focused19.455s PASS，同入口target_game/play构建61.026s、完整33源图/66文件声明式2.375s PASS。沿用当前生产版本已通过的alias startup；现在执行相同12窗口的Crouching公共Gate，保留总场景超时与全部源片段/姿态/时间守卫，未新增生产分支。

  - 03d窗口结果/公共观测有限复开2026-09-10 00:09:00：runner47.004s，manual0且12窗口全部覆盖、faults/graph_errors/temporal为空，独立validator1。当前生成树只存在实际FSM读数，旧validator仍强制已删除的Actor requested镜像；BF111.04仅该报告合同有限复开，旧Gate没有覆盖无镜像的原生执行树，最小self-test0.306s稳定FAIL。实际Rotate trace另证明100ms稀疏采样跨过0.5秒canonical、已Idle但未settled的尾帧被计入活动窗口、退出后被清空的clip字段丢失终态证据；修复只复用逐帧采样、实际Sequence缓存时钟和settled边界。当前33源图/66文件合同仍PASS，不改动画、IK、姿态阈值或角色速度。

  - 03d观测修复Gate前2026-09-10 00:17:00：validator保留实际FSM强制检查，requested仅在显式存在时校验，self-test0.555s PASS；Rotate复用节点最终时钟与当帧updated标记，只有真实活动或settled帧参与窗口，Turn/Rotate使用现有逐帧canonical采样。截图复核发现旧settled恒真使Turn截图拍到动作前定位下降帧，最小fixture17.315s FAIL后改为实际活动/结束；最终24项focused18.623s、同入口构建63.113s及完整声明式2.376s PASS。源算法和生产树沿用已通过startup的版本，继续同一12窗口真实复验；不以运行内覆盖PASS替代独立报告与视觉结果。

  - 03d本轮真实失败/直接Rig边界2026-09-10 00:28:00：runner46.982s，12窗口与全部报告合同通过，frame p95 14.597ms/max33.159ms；唯一失败为CrouchingDynamic淡出→Rotate Left接入时final右脚局部旋转1.243rad/42.640rad/s，pre-rig连续。MCP2.639s连接拒绝后fresh CR_Als36.155s及本地CPP确认ApplyFootOffsetLocation→Basic IK→ApplyFootOffsetRotation→Set Rotation；target在Basic IK之前用旧calf计算脚旋转，违反同帧读数顺序。BF109.10既有有限复开追加此直接消费者，旧Gate仅检查函数存在和局部写回，没有约束完整执行顺序；先实际调用fixture验证，再移动现有计算，不新增IK平滑或阈值。两条CrouchingDynamic uasset重新读取0.303s，StandPose参考与当前记录一致，不改资产。第一次CR路径少一层挂载导致49.993s/exit3，读日志后按已有asset_path纠正，不计为source证据。

  - 03d脚旋转静态闭环/Gate前2026-09-10 00:35:30：新增source顺序审计0.554s拒绝旧实现，现将现有角度计算移到混合后的Basic IK写回后，并按source最终Set Rotation权重应用；无新增solver或平滑状态。实际ECS调用在移除当帧脚/小腿读数后24.318s稳定FAIL于半权重，恢复后13项foot focused17.316s与288项目回归28.008s PASS。fresh Rig导出经原生成器0.557s在隔离目录生成源合同，runtime配置逐字相同；旧profile源快照不重写，fresh合同下133节点/1407pin/160link/79骨审计0.565s PASS，并纳入Crouching声明式。target_game/play构建64.447s已通过；现在按生产修改重新执行startup，再复验相同Crouching输入与原公共守卫，实际突变是否消除仍待结果。

  - 03d第二轮超时复盘2026-09-10 00:37:40：第二轮开始2026-09-09 23:42:03、预估结束2026-09-10 00:17:03、实际复盘结束00:37:40（偏差+20m37s），结果未通过真实Gate，无依赖项启动。已暂停新的runtime并回读33源子图、两份完整真实报告、当次Rig/CPP事实和focused结果：报告镜像/采样/截图迁移已闭合，新增漂移是动态过渡与Rotate重叠揭露旧Rig执行顺序；现在仅余该源顺序修复的真实证明及共享Standing转向回归。startup19.769s、6图和完整声明式2.644s均PASS，原场地站姿已检查。此轮未通过不能被内部12窗口覆盖替代，原源动作范围不缩减。
  - 第三轮纠正估时：按上一03d窗口1.5倍推导，开始2026-09-10 00:37:40、预估结束01:30:10。先用当前已闭合的静态/同入口二进制复验Crouching，若出现剩余首个差异则沿现有报告完成最小反例；随后做该共享Rig边界必要的Standing转向回归，不展开其它Standing实现。03e保留原八个动作与独立Gate，计划时点随依赖移到01:30:10–02:10:10，实际开始仍按当前事实更新。现在恢复同一03d动作Gate；若第三轮仍失败，第四轮前按规则重审整个过程、拆分与验证路径。

  - 03d连续输入边界2026-09-10 00:46:00：脚顺序修复后的runner46.232s仍只有同阶段final右脚42.462rad/s失败，故不把该已证实顺序差异称为完整根因。复核完整scene driver发现每个stationary case都会直接重置Actor身体/视角和rotation response；实际trace从Rotate Right末态-2.993rad到Left首帧-0.992rad，而look input为零，脚锁仍保持前一朝向的世界位置。fresh C++的脚锁瞬移依赖实际teleport事件；场景此次同位置改yaw没有走该生命周期。最小场景fixture24.355s稳定FAIL于该重置。现删stationary重置与逐帧强制零速度，延续移动结束后的实际身体/脚锁历史；Rotate鼠标起点取实际view，身体yaw只作旋转量基线。复用原输入函数、速率、完整结束和所有守卫，不添加脚锁补偿；接着focused/static和同入口增量后再验收。

  - 03d连续场景Gate前2026-09-10 00:49:00：删去stationary三类重置后，24项Crouching focused20.491s、33源图/66文件及当次Rig顺序声明式2.629s、同入口构建62.609s PASS。source Pole/Basic IK数学复核无新差异，停止扩张；本次只改场景输入/观测，复用该生产版本已通过的19.769s startup，执行相同动作范围与全部公共守卫。

  - 03d Crouching通过/共享回归前2026-09-10 00:50:30：连续场景runner48.080s、manual0/validator0，12窗口/22截图，frame p95 13.919ms/max27.926ms、eval p95 0.081ms，faults/graph_errors/temporal为空。四个Rotate/90°Turn的canonical误差0.001331–0.004086秒，实际父FSM、完整结束和signed yaw均通过；移动、Turn和Rotate截图已核对。删除case间直接Actor重置后右脚突变消失；真实输入与身体/脚锁历史连续。下一步同一二进制执行standing_turn，覆盖共享Rig顺序修改影响的正常90/180、Rotate及持续极限输入；不扩展Standing图或算法。03d与BF111.04公共观测有限复开待这次必要回归后共同提交，BF109.10其余Grounded边界仍留02e。

  - 03d完成2026-09-10 00:53:00：Standing完整声明式6.316s、同一二进制standing_turn45.149s均PASS，8-case/10窗口/17截图，frame p95 13.162ms/max24.808ms、eval p95 0.080ms，faults/graph_errors/temporal为空；180°与高速反转截图已核对。BF111.04本次公共观测复开及BF109.10新追加的脚旋转顺序边界关闭，Grounded共享输入原有边界继续留给02e。第三轮开始00:37:40、预估01:30:10、实际结束00:53:00（偏差-37m10s）；03d从最初22:30到此完成，对首轮23:20预估偏差+1h33m，前两轮失败/拆分已记录。耗时主要来自已独立关闭的Grounded alias入口、旧镜像/稀疏采样合同迁移、源Rig顺序漏检与场景身体重置；未新增补偿、平滑、依赖或source资产修改。现有12窗口完成并单独提交，Crouching整体仍待03e八个剩余动作，之后才恢复02e联合验收。

- [x] BF115.03e Crouching剩余源动作分支Gate：实际开始2026-09-10 00:58:00、预估结束2026-09-10 01:38:00；依赖已完成并提交的03d（68c275a），按提前完成事实更新原计划时点。补齐Crouching Stop进入Right、QuickStop三种源选择路径、两侧180°Turn及两侧Dynamic，以既有原生通知、直接montage和公共观测结构实现最小输入驱动；旧源事实已包含这些C++/AIS调用与18条montage资产，新增fixture不得手工注入动画状态。静态/最小fixture/实际入口构建后，在同一原Playground证明真实片段/曲线/权重、状态及完成生命周期，按失败源边界修复。估时依据为剩余8个独立动作窗口、已经验证的源播放器和Standing同类输入驱动；通过后Crouching整体完成，恢复02e包含下蹲后Standing恢复的联合Gate。

  - 03e源闭环与超时复盘2026-09-10 04:10:36：本轮00:58开始、预估01:38、复盘时仍未完成（偏差+2h32m36s）。同次公开uasset转换41.401s、unsupported 0，33源子图/66生成RON与已安装资产完全一致；fresh AIS 34.959s及本地C++/BP证实Stop固定Right且start 0.2、QuickStop三路及start 0.3、180阈值130°/播放1.2/不缩放角度、Dynamic按两脚8cm距离选择且播放1.5。MCP连接拒绝已保留，原始导出未改。漂移来自读取与迁移已有三个场景的实际状态、完成和观测合同，当前只增加一个连续crouching_actions入口并复用现有driver/coverage，尚未编译或启动runtime；不得把未执行的fixture当Gate。暂停实现复核后，剩余工作限于该入口的实际Crouching读数/8动作选择、最小消融测试、同入口编译/声明式及公共运行；不修改相邻图或扩大Rig闭包。第二轮开始2026-09-10 04:10:36、预估结束2026-09-10 05:10:36，按上一轮40分钟估时窗口的1.5倍推导；未拆分或缩小八动作验收目标。

  - 03e静态闭环2026-09-10 04:18:00：仅新增crouching_actions连续场景，复用三种既有输入driver/完成probe/coverage，不注入动画状态；选取真实stance对应FSM，正常Stop等待Move稳定、QuickStop在实际Idle→Move过渡时释放，180°使用160°鼠标输入跨越源130°阈值，Dynamic等待源距离排队、释放Aiming后完整播放并重锁。新增实际stance reader及11组真实input/aim系统fixture，初次失败仅为观测链label误写，已改为现有refresh_rotate；强制回读Standing的消融20.481s稳定FAIL，恢复后45项场景回归18.646s PASS。已删除Turn观测重复构建同一链的临时步骤。12图fresh spec4.600s、66文件编辑器零修复0.587s、完整声明式/33源子图及66文件一致性5.532s PASS；一次错误的summary路径参数在调用层修正，未误记为审计通过。当前同入口target_game/play构建中，下一步公共startup及八动作Gate，尚无runtime结论；转换器/生成RON/原始动画与Rig算法均无新差异或改动。

  - 03e真实Gate前2026-09-10 04:20:30：target_game/play入口构建112.342s PASS，等待过artifact锁后正常完成；检测到另一手动Playground进程后复查确认其自行退出，未终止外部进程。source/static闭包仍为33子图/66RON、实际下蹲Stop/QuickStop/Turn/Dynamic及18条原生直接montage，无新增算法差异。以下只启动同一二进制公共startup和连续八动作场景；要求实际变体、canonical clip time、完整结束/Idle、180°有符号转角至少3rad、Dynamic原生距离选脚及解锁/重锁，同时保留公共pose/temporal/性能阈值。

  - 03e首轮真实结果2026-09-10 04:26:00：startup18.748s PASS（6截图已核对）；八动作runner19.539s FAIL，普通Stop/默认QuickStop已完成，左QuickStop停在等待完整结束，公共faults/graph_errors/temporal为空。逐帧记录证明QuickStop在clip1.389s、AllowTransitions恢复full且左脚距离平方0.00725m²时，被native Dynamic接替；fresh C++ RefreshDynamicTransitions/PlayQueuedTransitionAnimation明确按>0.0064m²选脚并复用同一Transition slot，属于source允许的打断，不是动画算法差异。完整播放fixture改用±0.1前向偏移（保留真实左右符号和源play-rate检查），大角度打断证据保留；不禁用Dynamic、不放宽完整结束或姿态阈值。进一步核对现有producer/consumer发现Dynamic源序列编码Standing=0/1、Crouching=2/3，新观测仍按Standing编号比较，现按实际下蹲分支解码；只修03e新fixture/observer，未改运行算法。下一步最小回归/同入口编译后复验该连续八动作场景。

  - 03e复验前2026-09-10 04:30:40：±0.1输入通过真实input/aim及实际quick_stop_selection的左右/速率回归；队列编号消融22.293s稳定FAIL（下蹲Left编码2误作side 2），恢复后45项回归22.749s PASS。target_game/play构建75.127s、声明式2.910s PASS；图/资产与无驱动路径未改变，沿用本段已通过startup。下一次公共运行只复验八动作，保留首轮全部证据与完整结束、canonical、有符号转角和公共姿态/性能条件。

  - 03e第二次运行2026-09-10 04:38:00：runner35.271s FAIL，四个Stop/QuickStop及两侧180°Turn均通过完整生命周期，公共faults/graph_errors/temporal为空；高频720°/s瞄准驱动未排出Dynamic。回读fresh Crouching树：RefreshDynamicTransitions位于Idle，Stop/Idle才明确补FootLeftLock/FootRightLock=1，Rotate序列仅提供RotationYawSpeed；实际高频样本两脚lock为0，未形成Idle+有效脚锁距离前提。删去对Crouching复用高频Aiming的无效路径，改复用首轮已有真实证据的90°QuickStop→Idle脚锁偏移链，左右分别由真实横向输入和实际Idle→Move过渡释放触发；不改运动/脚锁算法或验收阈值。该连续场景仍要求两侧源Dynamic实际排队、独立片段canonical、完整播放与重新锁脚；用户新增移动瞄准问题保留在紧随的02e共享Actor闭包，不用本场景替代其验收。

  - 03e第三次运行前2026-09-10 04:40:15：45项场景/真实输入回归24.367s、target_game/play构建63.147s PASS；新Dynamic输入仅复用既有QuickStop左右动作，测试覆盖准备/移动/释放三阶段、真实source选择与2.375播放率。对上一轮公共validator逐项核对，仅Dynamic未覆盖/截图/生命周期导致失败，六个已执行动作无其它问题，两侧180°有符号转角及canonical均满足要求。图、资产、无驱动路径和静态合同未变化，继续一次完整八动作公共复验，不扩大测试/源模块。

  - 03e完成2026-09-10 04:42:31：公共八动作runner27.105s、manual0/validator0，8窗口/15截图全部PASS；两侧Dynamic解锁→完整播放→重锁、Stop/三种QuickStop自然结束/Idle、两侧180°canonical及有符号转角通过（3.137717/3.081694rad）。frame p95 13.386ms/max27.810ms、eval p95 0.085ms，faults/graph_errors/temporal为空；两侧Dynamic与180°截图已核对。第二轮04:10:36开始、预估05:10:36、实际04:42:31（偏差-28m05s），相对首轮01:38预估总偏差+3h04m31s；前两次失败是未满足source前提的fixture和新观测编码漏配，原始动画及生产算法未改。已登记的Crouching 20个窗口（03d十二项+本项八项）闭合并单独提交；用户追加的切换/共享移动瞄准症状继续按02e联合闭包处理，不能用这些独立动作PASS替代该验收。

- [x] BF115.04a Locomotion源闭包与首个转换/Actor差异：开始2026-09-10 09:47:00、预估结束10:37:00。依赖已提交Grounded联合Gate（337dcd9）和Standing/Crouching。以本地AB_Als_Locomotion uasset公开重读、完整递归图/FSM/动画/C++输入事实为唯一标准，逐项对比当前生成树与对应Actor状态/输入/生命周期；先交付可重复的首个差异与完整闭包清单，再按source证明登记后续最小实现和无截图真实Gate。估时依据为已归档同图30项递归闭包、现有全量converter与刚完成的62/22/33图审计流程；公开提取约数分钟，剩余用于源/目标合同及失败fixture，不将旧手工图或Actor输出当标准。不得展开Layering/Head/CR/Root或未经该source依赖证明的其它动作；原始导出不可变，所有新增转换仍从公开uasset入口生成。

  - 用户指定读取路径2026-09-10：直接读取本地uasset，不再通过Editor/MCP。此前健康检查曾成功，但OpenEditor调用未完成，随后连接拒绝；不将这些调用当作源图事实。当前已按公开uasset入口启动新一轮有界native提取/转换并保留不可变原始记录，输出仅写隔离目录。

  - 04a完成2026-09-10 10:00:00：公开uasset转换43.548s、unsupported 0；147 raw graph中32可达项（10 Pose图/2 FSM/18条件/2函数）、153节点，46生成RON，Fall/Jump/Flail/Land四同步组。当前二进制10图fresh spec2.910s通过；source审计0.834s稳定FAIL于10张已安装Pose图，覆盖64源Pose节点/50边，差异为旧Actor同步时钟、缺少当前曲线/非循环/活动状态重返策略及Land原生Rotate节点。输出仍全部隔离。fresh C++还确认OnJumped→Jump请求→NativeUpdate锁存→相关Jump/Fall的RefreshInAir消费；target将bJumped写为F32、按向上Air切换猜测跳跃，并由1150行旧Actor FSM/phase/curve推算替代图执行，InitializeLean/RefreshInAir仍只是Pose转发。只把这些直接输入/函数/资产纳入下一阶段；本项完成源清单和失败证据，不宣称动画行为已通过。开始09:47:00、预估10:37:00、实际10:00:00（偏差-37m）。

- [x] BF115.04b Locomotion生成树与输入ABI静态接入：开始2026-09-10 10:00:00、预估10:25:00。依赖04a，按同次公开converter输出仅安装该owner已有范围的十张变化图；修复bJumped Bool ABI并保留源类型最小失败，完成实际46文件加载/spec、编辑器/位置、源闭包一致性与适用focused Gate。估时依据为已完成43秒转换、46文件明确diff和Standing/Crouching同类接入记录。本段不运行真实场景；后续另立源InAir数据/实际图刷新与Jump事件闭环，删除旧影子FSM/时钟后才进行完整Locomotion Gate。

  - 2026-09-10 16:17:00有限复开：用户要求并修同类来源值遗漏，公开Locomotion重新读取46.943s确认左右Run Jump播放器可见StartPosition均0.12，旧target为0。旧Gate验证了连续同步/状态及canonical中段，没有独立对比可见pin与初始化位置。46份新编译输出仅两条起播值改变；仅该数据边界取消勾选，执行与真实回归并入05b2，不重开已通过的物理模式、跳跃输入、脚锁或其它Actor算法。

  - 04b完成2026-09-10 10:13:00：bJumped类型反例32.357s FAIL（None对Some(true)），按源Bool写入后15项Locomotion回归21.340s PASS。十张生成图由公开输出安装，保持原AB_Als owner的256文件集合，未手改RON或原始导出；46文件与编译输出逐字一致。编辑器修复0项（0.295s），target_game/play双example63.832s、十图fresh spec2.583s、完整声明式2.385s均PASS，源32子图/64 Pose节点/50边/两FSM/18条件闭合。旧生成时钟/缺失属性路径即静态失败对照；未增加新状态或依赖。开始10:00:00、预估10:25:00、实际10:13:00（偏差-12m）；只完成生成/ABI接入，Jump事件、InitializeLean/RefreshInAir执行及旧Actor推算清理仍未通过，不启动runtime。

- [x] BF115.04c Locomotion源InAir设置、曲线与标量单位合同：开始2026-09-10 10:13:00、预估11:08:00。依赖04b，沿同次32图及fresh RefreshInAir/RefreshGroundPrediction/RefreshInAirLean读取父AIS的InAir/General字段、两条曲线与实际图中的速度标量链；核对单位、坐标空间、输入/输出区间和时机，先最小反例后复用离线工具生成独立target数据，必要的通用转换器修复必须保留source fixture并公开uasset重新生成。估时依据为两条曲线、四个同步组与三条已知速度消费者，复用已完成Standing的CurveFloat导入/单位合同；本段不复制旧Actor公式作为标准。图相关的刷新与真实Jump事件随后另立不超过60分钟的实现段，全部静态闭合后再做原Playground无截图Gate。

  - 04c源/单位闭包2026-09-10 10:28:00：AIS与两条CurveFloat直接native读取33.911/33.002/32.753s PASS；Lean半衰期0.2秒、速度域-4000–0cm/s的五个真实关键点及Hit.Time域0–1的两个关键点已核对。GetVelocity是角色旋转的逆变换，垂直源Z对应target Y；Jump参考600cm/s、Lean参考350cm/s，预测速度-200/-4000cm/s、扫掠150/2000cm与mesh scale和实际capsule均保留于下一原生执行段。AIS预测仅阻挡WorldStatic/WorldDynamic/Destructible，旧公共sweep尚未约束此过滤和mesh scale，记录为04d直接边界；不改其它trace消费者。本轮图确定九处速度混合输入区间和一处650cm/s比较阈值仍为原单位、实际输入已为m/s。两个最小反例0.297/0.275s FAIL，离线映射修复后0.298/0.277s PASS；原source_default/IR不变。消融删去当前闭包未使用的递归标量适配，仅保留属性绑定及直接比较转换；复用既有RichCurve导出关键点/切线，公开重新生成进行中。当前仍未完成新数据消费和Jump/RefreshInAir生命周期，无runtime启动。

  - 04c完成2026-09-10 10:37:00：公开AIS/两曲线生成109.871s、Locomotion重新生成42.411s PASS；实际九个混合区间及650→6.5阈值由七个RON安装，46文件保持原256文件owner。旧安装source审计0.830s稳定FAIL于这七项；新十图fresh spec2.868s和完整32图/46文件声明式2.628s PASS，编辑器46文件零修复0.275s。Lean原线性重采样在源关键点得到-0.4660967，最小实际调用26.687s FAIL；换为离线目标RichCurve后16项Locomotion回归23.585s PASS，删除两个旧CurveAsset加载、运行时乘100及缺失曲线回退。两条原始导出保持不变；复用既有曲线类型、转换和LazyLock，不新增runtime适配层。标量适配消融0.301s、混合0.277s、converter/materializer/共享Curve自测0.561/0.264/0.274s PASS；target_game/play双example63.891s PASS。第一次公开调用因隔离安装目录未创建而0.298s退出，读报告后补目录，不计为source失败。开始10:13、预估11:08、实际结束10:37（偏差-31m）；本段数据与单位合同完成，原生刷新、实际Jump和Actor影子逻辑仍待04d，未运行场景。

- [x] BF115.04d Locomotion原生刷新、Jump事件与Actor消费者：开始2026-09-10 10:37:00、预估11:32:00。依赖04c；沿已提取32子图、InitializeLean/RefreshInAir、OnJumped→Jump请求和AIS预测/Lean合同，复用已有共享SourceInstance、原生节点及Tnua实际动作事件，实现按实际活动节点刷新与jump请求锁存/消费。预测只纳入源证明的当前capsule、mesh scale、walkable normal和AIS过滤边界；先最小失败再修复，不改变其它trace算法。删除旧Actor FSM/phase/手工曲线和向上速度猜测；对仍有真实消费者的输出改接实际生成树。估时依据为已完成七项数据迁移、Grounded/Crouching同类原生调用约半小时的记录与现有节点/事件框架；完成focused、消融、公开重新生成和同入口静态后，再独立登记真实动作观测及无截图Playground Gate。本段不启动runtime，也不展开Default或后续模块。

  - 04d原生/Actor闭环进展2026-09-10 11:12:00：连续两次请求的共享属性反例39.288s FAIL；复用既有write_frames增加显式当帧输入，保留其余持久属性和实例隔离，不修改依赖私有arena。InitializeLean/RefreshInAir映射0.292s FAIL→0.304s PASS；真实图fixture覆盖活动/非活动、落地速度保留、两次请求、pending/零半衰期及重新相关，分别移除刷新和初始化都改变结果。fresh AlsMath证明Damper使用InvExpApprox；C++九个标量和两曲线经公开入口114.570s重新生成。原预测扫掠最小实物理fixture52.961s FAIL（mesh scale 3仍未触及地面）；新路径读取实际Collider、mesh scale和Tnua实际坡度限制，并排除源Pawn/PhysicsBody对应的角色及ragdoll collider，保留初始穿透接触。预测准备属于ECS物理读输入，只有实际图调用才提交InAirState；C++ GetControlRigInput直接证明Root/Rig需要该状态，现从图后投影既有组件，不改Rig算法。旧Locomotion私有FSM/时钟/手工曲线已删除；另确认AirJumpLayer只给旧入口pin和manual变体观测写入交替左右猜测，现改读当前源Jump同步组四个实际Sequence及权重，完整外/内FSM观测仍由下一段覆盖。首次组合测试61.037s因新fixture浮点类型及旧phase读数Option误用编译失败，修正后native与曲线focused通过；新增过滤消融fixture暂用引用临时值导致5.545s编译失败，已改为局部绑定，回归进行中。未启动runtime。

  - 04d完成2026-09-10 11:27:00：339项als-ext与285项项目回归25.230s PASS，包含真实节点调用、当帧Bool请求、零/普通/pending Lean、落地值保留、实际物理扫掠及过滤/mesh scale/capsule消融；删除旧Actor镜像测试，保留新失败边界。公开Locomotion uasset78.187s、unsupported 0，三张调用图由原owner安装，其余43文件不变；target_game/play双example120.860s、十图fresh spec3.635s、46文件编辑器零修复0.271s与完整32图/64 Pose节点/50边/两FSM/18条件声明式4.204s PASS。manifest曾将Cargo测试误列为只读命令而0.274s失败，按原schema分离为已执行的focused_tests证据，Python只读审计保持原入口；未修改审计器或增加包装器。Actor现在只准备真实物理/曲线输入，InAirState和Lean由生成节点按源位置/生命周期写入，两个手工FSM、四个时钟及虚假transition receipt全部删除；Jump变体与phase读取实际播放器。开始10:37、预估11:32、实际结束11:27（偏差-5m）；结果为生产执行静态闭合，尚未启动Locomotion真实场景，下一段补完整动作观测/fixture后再进行公共数值Gate。

- [x] BF115.04e Locomotion完整动作观测与场景静态闭环：开始2026-09-10 11:27:00、预估12:22:00。依赖04d，沿32源子图/两FSM/18条件及九个速度混合区间，为左右Jump Walk/Run、Jump Loop/Flail、Fall/Fast/Flail、预测和Land/Land Movement建立真实状态、clip/weight/phase、曲线与生命周期读数及源前提驱动。复用既有Playground场地、输入、逐帧采样、canonical和watchdog；不能用Actor猜测、固定时间推进或请求值充当实际状态。先读完整条件/分支和既有公共case，做最小场景/reader fixture、设计消融与同入口静态；完成后另立不超过60分钟的无截图startup及动作Gate。估时依据为已完成实际执行和数据闭包、Standing/Crouching场景迁移记录及现有零截图公共runner；本段只制作/验证场景与观测，不启动runtime、不展开后续模块。

  - 04e源/场景前提2026-09-10 11:52:00：同次32图/两FSM/18条件已逐项回读：脚底曲线≤0选Left、>0选Right，Jump速度2–5m/s连续混合；起跳片段完整结束后自动进入Loop，再以源恒true条件用1秒过渡到Flail，不能要求Loop长时间稳定。Fall的Fast/Fall/Flail按三条实际混合边取证，不按片段名称猜测速度。CS_Als_Default直接native重读33.960s确认默认落地7m/s翻滚、10m/s Ragdoll；高台长空中段仅验到已发生的Loop/Flail及落地预测，随后显式fixture重置，低落差单独验证Land/Land Movement和完整Grounded回调，不能把默认动作接管当动画故障或禁用其算法。重落地additive在默认翻滚阈值之前权重为0，其7.5–15m/s混合合同仍由完整源/生成数据与标量fixture验收，不伪称默认场景实际完整播放。原Playground18米高台及原平地保持，新增两个FSM/13片段/实际blend alpha只读观测，无私有动画状态机。新场景首次编译16.302s在JSON数组表达式失败、第二次11.902s在NodeId显示格式失败，均为新观测调用错误，已修正后继续focused验证；未启动runtime。

  - 04e静态收口2026-09-10 12:20:00：源脚底provider读数2.168s稳定FAIL（None对0.75），补与实际源pin相同的实例优先/全局输入回退；实际blend alpha记录消融29.879s FAIL（0对1），恢复后339 ext/289项目回归28.814s PASS。标准采样已出现但观测时长尚不足的非循环落地反例24.893s FAIL，保留canonical命中后51项场景回归19.037s PASS；12秒case进展上限、失败条件JSON与正常完成均有focused检查。原高台坐标/外侧无地板路径分别39.242/27.247s被静态fixture拒绝，复用既有橙色测试平台移至已验证平地上方30m，按1.666667秒起跳、最小1.2速率、0.2/1秒源过渡及实际gravity推导足够滞空；原地形/环境/角色/相机保持，不另建世界。其原Collider仅为可见Mesh一半，真实spawn/AABB反例19.589s FAIL，按现有Mesh的6×0.5×6尺寸修复共享测试fixture，原移动路径已按同一可见边界校验；最终52项场景回归17.216s PASS。七段物理动作登记十四窗口，四起跳与Fall/Fast分别对齐同一源相位，Flail/落地有独立时间与姿态链；高坠落达到所需状态/预测后显式重置，低落差等待真实落地回调及Grounded，默认高冲击动作规则保持。生成32子图/46RON未变化，声明式预审3.694s PASS；当前同入口预构建进行中，仍未启动runtime。

  - 04e完成2026-09-10 12:25:00：target_game/play双example114.245s、十图fresh spec3.129s、46文件编辑器零修复0.594s、公共数值验收器自测0.844s及完整32源子图/46RON声明式2.663s PASS。实际状态/权重/片段/曲线读取使用当前角色的生成上下文，新增源alpha读数只记录已求值结果；不增加生产动画分支。开始11:27、预估12:22、实际12:25（偏差+3m），结果为七段场景/十四窗口的静态闭合，尚无runtime结论；本段独立提交后开始04f公共运行。

- [x] BF115.04f Locomotion无截图startup及完整动作Gate：开始2026-09-10 12:25:00、预估13:20:00。依赖04e，使用已构建同一target_game/play入口先startup，再七段连续动作与十四数值窗口；强制实际两FSM、四Jump变体同相位、Loop/Flail、Fall/Fast/Flail、预测、普通/混合落地及移动落地、自然结束回调和完整Grounded终态，保留公共root/pre-rig/final、canonical、时间与性能守卫。高坠落按默认CS动作边界只验收空中闭包并显式重置，重落地additive完整混合范围由已通过的source/标量合同验证，不用请求状态或Actor影子数据充当执行证据。估时依据为已完成生成/Actor/场景静态、52场景回归、当前源7段输入与此前Grounded三个场景真实耗时约数分钟；预留首个源链失败的最小诊断与修复闭环。所有截图关闭，失败先读完整报告再沿已证实边界处理，不展开Default或其它后续模块绕过Gate。

  - 04f首次startup/04d有限复开2026-09-10 12:29:00：公共零截图startup5.228s失败，manual101，尚未进入首帧/图求值；runner保存缺少report事实，stderr完整指出Locomotion→Layering→Head→Ragdolling→GeneratedProvider→Locomotion的调度环。此前focused仅初始化独立节点/系统，未把实际插件顺序组合初始化，故编译/spec/静态无法发现。04d仅调度依赖边界取消勾选；回读所有Locomotion set调用点和真实writer，Layering只读stance与上一动画Root曲线、并不消费新InAir输入，旧after Locomotion边来自已删除的Actor阶段。当前只补实际插件调度反例并删除该过时顺序，不展开Layering/Head图或算法。

  - 04f调度静态修复2026-09-10 12:39:00：七个真实相邻插件组合初始化35.213s稳定FAIL于同一调度环；fresh C++ RefreshLayering及target writer确认仅读曲线/stance，stance在Update已发布，删除其过时after Locomotion约束和导入，不增加替代顺序。去边后测试19.599s暴露fixture错误：把整个Schedules移出world再初始化会触发Bevy重新插入资源；改用原生schedule_scope后291项目回归20.653s PASS。图/算法/原始资产未变，61.302s生产预构建已通过；最终test入口变更后刷新同入口和spec，再跑startup及动作，无新增runtime探索。

  - 04f重跑startup前2026-09-10 12:44:30：最终同入口62.866s、十图fresh spec2.606s和完整32源子图/46RON声明式2.684s PASS，291项目回归与实际插件顺序反例保留。源图、单位、Pose算法均未新增差异，当前仅删除已证实过时的输入阶段依赖；先重新确认应用初始化与Root数值状态，再执行完整七段动作。CaptureCount=0，未降低任何窗口/姿态/时间守卫。

  - 04f startup通过/动作Gate前2026-09-10 12:46:00：公共startup15.174s PASS，manual0/validator0、0截图/0PNG，faults/graph_errors/temporal为空；七真实插件回归与实际入口共同确认调度环消除，04d仅本轮调度依赖复开边界关闭。32图/46RON、291项目、fresh spec和声明式合同均保持通过，提交此最小修复后，同一二进制执行完整七段/十四窗口，尚无动作结论。

  - 04f首轮动作结果/出口顺序闭包2026-09-10 13:08:00：runner18.886s失败（manual91/validator1），首个Walk Left实际Jump、源连续权重及canonical通过，但1.027s落地后经Grounded Alias直接回Grounded，未出现Land或LandToGrounded回调，12秒case watchdog正常结束。公共pose pass=true，faults/graph_errors/temporal为空、0截图。回读全部源落地条件与bDisabled均确认入口true且未禁用；本地UE5.8 StateMachineBase按具体状态先汇总alias/direct出口，再调用GetTransitionList(priority sort)，IntroSort同优先级不保序；target仅对各source组按priority/pin顺序稳定排序后拼接，未保留这种混合出口次序。当前仅为取证增加native实际顺序输出，读取器构建78.050s PASS，公开uasset重新提取进行中；未据假设修改图、算法或验收目标。需先以fresh原生顺序证明首条分歧，再做转换器最小反例/修复与重生成。

  - 04f原生顺序证据/04b有限复开2026-09-10 13:25:00：公开uasset提取57.537s确认Jump/Fall的Land Conduct入口先于Grounded Alias，Land Conduct先判断Land Movement再默认Land，正好解释首轮直接回Grounded。原01源排序使用priority/pin顺序在各source组内排序，缺少UE汇总alias/direct后的实际次序；旧生成一致性Gate只证明与同一转换器一致，未覆盖引擎排序语义。04b仅FSM执行顺序取消勾选，原Bool ABI/其它迁移保持。原生读取器直接复用UE GetTransitionList与同一alias MultiFind，避免手写不稳定排序；工具fixture0.300s稳定FAIL，删除旧排序后用Python标准库拓扑排序合并各状态顺序，0.307s PASS，冲突顺序显式拒绝，未加runtime调度层。contract/renderer自测0.312/0.303s PASS。第一次隔离转换误把完整路径当namespace，输出未安装且原生证据有效；正式重新生成使用generated命名空间，并保留旧原始记录。当前尚未安装新资产或重跑runtime。

  - 04f原生顺序静态闭合/运行前2026-09-10 13:37:00：最终公开uasset重新生成44.826s，native导出保留具体状态与conduit的实际出口次序；32源子图/46RON中四项顺序差异安装，原owner和原始记录保持。旧四资产审计1.099s稳定FAIL；新十图fresh spec1.877s、编辑器46文件零修复0.299s、完整声明式12命令3.197s和同入口增量3.729s PASS，converter/materializer及冲突顺序反例通过。当前只验证已证实的Jump/Fall→Land优先次序与Jump入口次序，下一步公共startup和完整七段十四窗口，CaptureCount=0；尚无新动作结果。

  - 04f顺序修复startup通过/动作Gate前2026-09-10 13:38:00：公共startup14.703s PASS、CaptureCount=0；新四资产已通过真实入口启动，完整source/static和原姿态守卫保持。接着运行相同原Playground的七段动作/十四窗口，验证真实Land与后续分支，当前不提前关闭04b执行顺序边界。

  - 04f首轮超时复盘2026-09-10 13:40:00：本轮开始12:25、预估13:20、当前13:40（已耗75m、偏差+20m），完整Gate未通过；先暂停实现及新的runtime启动，回读32源子图、四资产顺序差异和两次运行证据。新startup14.703s通过；动作25.751s失败于case2 watchdog，普通左右Jump→Land→LandToGrounded→完整Grounded已实际通过，Run Left→Land Movement→完整Grounded也发生，fault/graph/temporal为空、0截图。native T3普通Land出口有LandToGrounded，T2移动Land出口没有通知；场景却给所有落地统一要求该回调，导致真实已完成仍等待。公共collector另显示Land片段canonical读数缺失与一次终态窗口零采样，需要先最小fixture核对时钟/采样链，不能放宽阈值或改生产图迎合错误预期。04e仅这些已证实观测边界有限复开，04b顺序边界继续待完整运行；其它闭包不复开。
  - 纠正与拆分：按上一55m估时窗口1.5倍，下一轮开始13:40:00、预估15:02:30，拆为04f1源动作验收条件与采样静态收口（13:40–14:25）和04f2完整无截图Gate（计划14:25–15:02:30）。剩余工作依据为native已证明的通知区别、一次短窗口采样与已有真实状态轨迹，先修最小观测根因，再复用现有构建/声明式流程；保留全部十四窗口、Loop/Flail/Fall/预测/两类落地目标及后续模块顺序，自动继续，不以超时停止任务。

  - [x] BF115.04f1 源动作验收条件与采样静态收口：开始2026-09-10 13:40:00、预估14:25:00。只沿已证明的移动落地无通知、Land实际clip time读取和短终态窗口采样边界补最小失败，修复真实公共观测链，完成消融、适用回归、同入口及静态合同；本段不启动runtime。
  - [x] BF115.04f2 完整无截图Locomotion Gate：实际开始2026-09-10 13:55:00、预估14:32:30（沿原37m30s窗口提前开始）。依赖04f1，保留原场景全部七段十四窗口和source/public守卫，实际start/motion通过后关闭04b/04e有限复开并提交。
  - 04f1观测根因静态闭合2026-09-10 13:52:00：实际无通知移动落地与短窗口两个反例32.529s稳定FAIL；按最近实际Land/Land Movement状态保留源通知差异，父FSM过渡字段接入已有读数，复用逐帧采样并用已有Grounded就绪时间保留终态窗口，293项目25.102s PASS。同一失败轨迹的首个blend无效帧再核对fresh左右TwoWayBlend，均有200–500cm/s映射及5/s插值、映射本身不预夹紧；原验收瞬时速度公式20.574s FAIL。现比较实际节点alpha和两片段归一权重，源插值函数以该速度变化复算，去掉插值的消融仍被拒绝，294项目23.178s PASS。未改生产运动/混合算法或放宽阈值。首次测试误用了target_game/debug而非已缓存target/debug，确认本次进程/目录后中止102.508s编译并改正；实际入口预构建仍使用target_game/play。下面完成同入口与声明式静态，再登记运行。

  - 04f1完成/04f2运行前2026-09-10 13:55:00：最终294项目23.178s、公共数值validator自测0.838s、target_game/play双example68.418s、十图fresh spec3.399s和完整32源图/46RON声明式12命令3.426s PASS，编辑器零修复合同保持。开始13:40、预估14:25、实际13:55（偏差-30m）；结果为观测静态闭合，保留14窗口和原阈值，未改生产动画算法或新增时钟。04f2提前开始，先当前版本startup，再完整七段动作，CaptureCount=0；顺序和观察的有限复开仍以最终实际结果收口。

  - 04f2 startup通过/完整动作前2026-09-10 13:56:00：公共零截图startup15.245s PASS；当前观测修复已进入实际入口，Root/公共守卫通过。32源子图、46RON与294项目回归保持闭合，开始完整七段十四窗口，不跳过上轮尚未执行的Run Right/Loop/Flail/Fall/预测/混合落地。

  - 04f2动作结果2026-09-10 13:57:00：公共motion26.319s完成但FAIL（manual91/validator1），九窗口全部public数值/状态/源chain/canonical通过，四Jump、Loop/Flail、普通及移动落地、完整Grounded和源权重PASS；frame p95 14.622ms/max26.299ms、eval p95 0.0831ms，无fault/graph/temporal、零截图。Fall高台case5在fixture重置首帧看到上一Jump→Land，未触发新动作即满足通用落地完成并跳过，04f1仅此场景触发边界有限复开；源图不得改成忽略真实落地。case6实际完成低落差Land，但InAir垂直速度在入Land前已近零，未覆盖混合重落地；下一步沿已提取RefreshInAir、实际物理速度/模式发布核对首条差异，尚未据此复开生产算法。其余已通过九窗口保持，不提前勾选全Locomotion。

  - 04f2物理模式source闭包/04d有限复开2026-09-10 14:08:00：直接回读本地AlsCharacter、AlsAnimationInstance及UE MovementComponent，bTickBeforeOwner使Movement先于Actor，Mesh又显式依赖Actor；OnMovementModeChanged立即发布模式，动画随后读取同次角色模式/速度。target的locomotion_state_sync→refresh_gait→grounded_dynamics约束在Tnua Logic之前，动画速度却采集当前motor，导致最后一个Air帧读取已修正的近零速度。04d只取消此物理模式发布边界勾选；原节点算法、数据、Jump请求与曲线合同不复开。同步回读UE CallFunction和StateMachine确认OnUpdate与先转移后求值，未发现应增加RefreshInAir条件分支的source依据，不做速度锁存补偿。真实Tnua最小物理测试进行中，用当前controller.is_airborne对照实际LocomotionState，下一步修顺序与原事件出口。高台触发反例25.887s FAIL→22.435s九项PASS，仅记录新动作触发后的Air状态，尚未重跑runtime。

  - 04f2模式发布静态修复2026-09-10 14:13:00：真实Tnua/实际Motion插件120帧fixture在第3帧Grounded对Air稳定FAIL（26.437s），改为Logic后提交物理模式并发送原StateTransitionEvent，移除gait的旧反向依赖；landing friction和既有Air消费者明确随后读取，不新增状态或快照。20.062s focused PASS；加入恰好一次模式事件/原from-to与Ragdoll请求保护，删除落地friction的冗余after Logic边，core/ext/项目完整回归68.714s PASS（243/339/295，1项既有ignored）。此前测试只分别验证Mode请求与速度，没有在同次真实controller更新后比较两者。下一步同入口构建、Locomotion及共享已完成图的静态回归，再无截图startup/完整Locomotion；图算法、物理参数、原始资产保持不变。

  - 04f1补充收口/04f2运行前2026-09-10 14:18:00：新动作触发后的Air记录反例及模式发布/事件回归均通过，core243（1既有ignored）/ext339/项目295保持PASS；同入口target_game/play136.170s、十图fresh spec2.899s、Locomotion完整32源图/46RON声明式3.157s PASS，Standing/Grounded/Crouching共享静态8.456/8.984/4.798s PASS。04f1原开始13:40、预估14:25、补充实际结束14:18（偏差-7m），本段仅观测静态有限复开关闭；04d物理模式和04b/04e运行结论继续待真实Gate。下面先startup，再完整七段十四窗口，全部CaptureCount=0，原source/public阈值保持。

  - 04f2模式修复startup通过/动作前2026-09-10 14:19:00：公共startup15.440s PASS、validator issues为空、零截图；完整实际插件与新的物理模式阶段共同通过，32源图/46RON静态与243/339/295回归保持。接着完整七段十四窗口，重点核对新触发的高台Fall与低落差混合落地，不降低其余已通过九窗口要求。

  - 04f2十三窗口通过/落地回弹边界2026-09-10 14:26:00：完整motion30.559s FAIL，仅land_mixed未完整覆盖；其余十三窗口、源连续权重/状态/完整Grounded均PASS，无fault/graph/temporal、零截图。新模式发布使低落差以-6.057m/s进入Land，Light/Heavy权重0.789/0.211正确；随后实际物理向上约1.3m/s、再次Air，Land按源规则中断，不能把短中断窗口判成完整混合落地或放宽canonical。直接回读UE MaintainHorizontalGroundVelocity/AdjustFloorHeight及当前Tnua Walk：UE消去地面竖直速度后独立调整地板间距，Tnua默认spring_dampening=1.2把竖直相对速度按120%反向boost，已明确产生回弹的算法差异。04d当前物理落地边界继续打开；沿同一个120帧真实Tnua fixture补“无输入落地后不再次Air”的失败，再用现有config约束抵消比例，不加状态、门限或动画补偿。

  - 04f2回弹最小修复2026-09-10 14:30:00：同一真实Tnua fixture新增落地后不再次Air断言，27.454s在第42帧稳定FAIL；按source消去地面竖直速度的语义，把已有spring_dampening设为1.0（恰好抵消相对速度），仅一行配置，无新状态/阈值/组件。原1.2设置即失败消融；加入8.3/16.7/33.3/50ms交替步长后21.120s PASS，243 core/339 ext/295项目回归51.687s PASS（1既有ignored）。接着同入口构建与fresh spec/static，之后完整Locomotion及共享模式的Grounded/Standing/Crouching真实回归，全部零截图。当前十三窗口通过事实保留，混合落地仍未标完成。

  - 04f2最终静态闭合/运行前2026-09-10 14:34:00：243 core/339 ext/295项目（1既有ignored）、回弹负例与变动步长正例保持PASS；同入口135.382s、十图fresh spec3.209s、完整32源图/46RON声明式12命令3.409s PASS。只使用Tnua现有竖直阻尼字段消去已证明的速度反转，未改变spring强度、cling范围、物理时钟或验收目标。启动当前版本startup及完整Locomotion，之后用既有Grounded/Standing/Crouching场景回归共享模式/地面接触边界，CaptureCount=0。

  - 04f2最终startup通过/动作前2026-09-10 14:35:00：公共startup15.508s PASS、零截图；最终物理模式与阻尼配置已通过完整入口初始化和Root数值检查。32源图/46RON、fresh spec/static与243/339/295回归保持，通过后继续完整七段十四窗口，未只挑最后失败的混合落地。

  - 04f2十四动作通过/公共时间守卫待解2026-09-10 14:37:00：完整motion30.272s manual通过，十四窗口/全部生命周期/source blends都PASS，混合Land持续1.155s、canonical误差0.0000694，原回弹消除。但validator因一次final calf_l local_rotation_spike拒绝（0.77984rad/0.0140262s，55.599rad/s，高于40），pre-rig及graph无同类异常、零截图。故整体仍未勾选。该帧位于case4高台fixture重置后0.494s的Grounded启动移动阶段，前一采样左脚solver_reference在Y22.596而当前foot在Y30.407，随后恢复同空间；先沿当次CPP脚锁teleport及target空间读数检查首个失配，未展开CR图或按猜测修改IK/阈值。此前十四动作结果保留，共享场景真实回归仍待公共守卫通过。

  - 04f2脚锁source闭包/BF102.43有限复开2026-09-10 14:43:00：fresh RefreshFootLock把上一帧FinalLocation/Rotation作为component-space缓存，新锁脚用当前ComponentTransform转成world；target的同名缓存是world-space，却仅在teleport窗口且旧lock相关时重投影。case4传送时未锁定的左脚在首个满权重帧因此锁住旧地面坐标，随后释放时引发末端突跳；常规组件位移也属于同一空间合同。BF102.43仅该缓存边界取消勾选并记录旧测试只覆盖已锁定rebase的漏检；使用既有helper和状态，在普通/传送、旋转/scale最小fixture中先复现，不更改锁定规则、source Release速度或CR求解器。

  - 04f2脚锁空间静态修复2026-09-10 14:46:00：未锁定脚在普通位移/30m传送及旋转、scale变化后首次锁定的真实helper+refresh反例31.181s FAIL；现每帧重投影上一帧final缓存，锁定点只按原teleport/相关性条件重投影，保留原world-space消费ABI和已有previous transform，不新增缓存或状态。合并左右重复块，旧条件路径即消融失败；296项目21.183s PASS，原已锁定传送/骨骼恢复测试保持。下面更新同入口及完整声明式，再执行公共Gate；BF102.43仍待真实时间守卫通过后关闭，不涉及CR图或求解器改动。

  - 04f2脚锁修复运行前2026-09-10 14:49:00：296项目21.183s、同入口63.390s、十图fresh spec2.859s、完整32源图/46RON声明式12命令3.178s PASS；当前脚锁缓存空间修复不改变锁定/释放条件、状态机或曲线。先startup再完整十四窗口，验证高台准备阶段final守卫，随后完成共享场景回归，全部零截图。

  - 04f2脚锁修复startup通过/动作前2026-09-10 14:50:00：公共startup15.263s PASS、零截图，当前脚锁缓存修复已通过真实入口；完整32源图/46RON、296项目及fresh spec/static保持。开始完整七段十四窗口，公共final时间守卫仍启用原40rad/s阈值，不增加场景重置或采样豁免。

  - 04f2完整Locomotion通过2026-09-10 14:51:26：公共motion29.956s PASS，manual0/validator0、十四窗口和全部生命周期/source blends通过，fault/graph/temporal为空、零截图；高台准备阶段脚锁final异常消除，混合落地持续播放与canonical通过。开始13:55、预估14:32:30、实际结束14:51:26（偏差+18m56s），结果为完整Locomotion动作及公共守卫Gate完成。04f父项仍保留共享模式/地面接触/脚锁消费者回归；该新增共享边界单列04f3，未将后续模块或未通过项删除。

- [x] BF115.04f3 共享模式与脚锁真实回归及提交收口：开始2026-09-10 14:53:00、预估15:23:00。依赖04f2已通过；使用同一已验证二进制，在原Playground依次跑既有grounded、standing_response、crouching公共case，检查角色模式、落地接触与上一帧final空间修复的直接消费者。三图声明式与243/339/296回归已闭合；估时依据为此前三case共三十窗口约两分钟、当前14窗口29.956s、无需再次构建/导入，保留首个source链回归失败的诊断空间。全部CaptureCount=0，守卫及旧目标保持；通过后分别关闭04b/04d/04e和BF102.43有限复开并按修复scope独立提交，再继续Default。

  - 04f3 Grounded通过/Standing运行前2026-09-10 14:55:00：公共grounded42.757s PASS，manual0/validator0，9窗口、零截图，fault/graph/temporal为空；物理模式/接触及脚锁空间修复未破坏既有Roll和切换/瞄准终态。当前源码/同入口/声明式保持，接着standing_response，再crouching，不改两者场景目标。

  - 04f3 Standing通过/Crouching运行前2026-09-10 14:57:00：公共standing_response19.928s PASS，manual0/validator0，9窗口、零截图，无fault/graph/temporal；当前脚锁component-space缓存与模式发布保持原运动响应。相同已构建入口继续crouching，全部原阈值及source/static合同保持。

  - 04f3共享Gate通过/04b关闭2026-09-10 14:59:00：公共crouching40.336s PASS，12窗口、零截图；连同grounded9窗口42.757s、standing_response9窗口19.928s，三十共享窗口全部manual0/validator0，无fault/graph/temporal。Locomotion十四窗口29.956s同样PASS，frame p95 14.9085ms/max28.3111ms、eval p95 0.0875ms。32源图/46RON、完整源码/单位/生命周期和296项目（另243 core/339 ext）证据闭合；04b本轮原生FSM排序边界关闭，先独立提交native读取器/转换器与四份生成执行数据，再分别收口物理模式、观测和脚锁修复。

  - 04d物理模式/接触有限复开关闭2026-09-10 15:01:00：当前controller模式同帧发布、原事件恰好一次、Ragdoll请求保护及无反弹落地的最小物理反例均PASS；243 core/339 ext/296项目、十四Locomotion和三十共享窗口全通过。仅改实际模式提交阶段、直接消费者顺序与现有阻尼的速度抵消比例，没有新增物理时钟、状态、动画锁存或阈值；该scope独立提交，原04d其它已通过内容保持。

  - 04e观测有限复开关闭2026-09-10 15:01:00：移动落地按实际Land Movement出口验收，短窗口逐帧采样、父FSM状态、已插值alpha与片段权重、重置后新动作触发都由最小失败约束；全部十四实际窗口和完整生命周期已通过公共验收。复用原collector、现有计时与真实图缓存，未增加Actor镜像或降低canonical/姿态/时间阈值；观测scope独立提交。

  - 04f3/Locomotion完成2026-09-10 15:02:00：最后crouching12窗口PASS（frame p95 18.1825ms/max26.3996ms、eval p95 0.0961ms）；全部44动作窗口与当前startup均通过、0截图，图错误/根Pose/空间/时间/性能守卫无遗漏。BF102.43上一帧final缓存空间的有限复开关闭并独立提交；04b/04d/04e已分别关闭。04f3开始14:53、预估15:23、实际结束15:02（偏差-21m）；04f首次开始12:25、预估13:20到最终15:02偏差+1h42，原超时/重估/拆分与每轮最小失败保留。主要纠正为native FSM次序、source观测条件、同帧物理模式、速度抵消与脚锁component-space缓存，未引入动画补偿或阈值豁免。Locomotion完整迁移闭合，随后按清单顺序进入AB_Als_Default。

- [x] BF115.05a AB_Als_Default源闭包与首个转换/Actor差异：开始2026-09-10 15:08:00、预估15:38:00。依赖已完成并提交的Locomotion。按用户最新范围，此步仅处理AB_Als_Default，其余12个overlay留待用户以后安排。本项从本地Default uasset公开重读完整图/直接动画资产、继承输入和实际Actor消费/挂接事实，比较当前生成资产和执行链，保留所有源差异及最小失败，再按证据登记实现和零截图公共Gate。估时按已定位Default两份生成RON、已归档同图公开转换与本轮native单资源33–45秒记录收敛；先前13项扩展估时撤销，尚未提取或实现其余12项。本轮不展开Layering/Head/CR，所有读取继续使用公开uasset/native commandlet。

  - 05a源闭包进展2026-09-10 15:19:00：Default公开转换77.518s、unsupported为空，闭包为AnimGraph和Overlay两图、十四个源Pose节点；直接uasset已读取Default_Poses及Idle的曲线/时长。fresh可见pin与编译CDO共同确认五个采样帧0/1/2/1/0、Idle叠加Alpha=0.75；当前生成器丢失采样帧和可见常量默认值，旧验收只看Actor模式及固定曲线，未验证分支姿势/叠加权重。GetParent绑定的Standing/Crouching/InAir/GaitWalking与GroundPrediction输入，沿fresh LinkedAnimationInstance初始化和RefreshPose核对既有provider；另只读取父AB到Default的直接挂接边，不展开父图或Layering/Head/CR内部。现固定最小失败与同步时钟消费边界，再登记实现。

  - 05a完成2026-09-10 15:21:00：两图全部节点/直接资产和五个父属性输入已核对，Default_Poses原生帧率30、4keys/0.1s；父AB只读36.987s确认默认实例Default及直连Overlay输出。对同次source与原生成RON的六项最小检查0.300s稳定FAIL，三处非零frame被抹为0、Alpha合同1对0.75；CDO验证可见pin确实是生效常量。Parent/RefreshPose的先更新再求图与现有provider相符；同步迁移只需删除Default生成组的旧Actor注入，通用legacy时钟仍有其它已存消费者，维持不动。开始15:08、预估15:38、实际结束15:21（偏差-17m），结果为source及失败闭环完成，功能尚未验收。

- [x] BF115.05b Default常量、同步和生成资产静态闭合：开始2026-09-10 15:21:00、预估16:06:00。依赖05a；按两个已证实默认值边界、既有可见pin读取helper及两份RON范围，先扩充通用contract/renderer最小反例，保留显式帧、可见Alpha及动态绑定/连接语义，复用现有运行节点字段。公开uasset重读后只安装原owner内Default两文件，移除该生成组Actor时钟注入，完成source审计、实际节点fixture、同入口编译和声明式静态；实际Default分支姿势、0.75叠加与预测插值验收另列05c零截图Gate。估时依据为本段源闭包13分钟、native读取约37–78秒、此前单边运行合同与静态收口约16–24分钟；不展开其余12 overlay或后续图。

  - 05b首次静态修复2026-09-10 15:31:00：contract/renderer反例0.302/0.261s FAIL后，复用可见pin读取与现有explicit_frame/constant_alpha字段，四个转换自测PASS；两图公开重新读取38.420s、仅覆盖原owner内Default两RON，六个来源常量检查0.272s PASS。339 als-ext实际节点/曲线回归52.666s、同入口构建126.121s PASS；旧实现即消融失败，动态Alpha连边/属性绑定和隐藏默认值对照保留。尚未跑runtime。用户要求并修全部同类来源值问题，因此先沿转换器已支持节点和元数据公共路径追加05b1，不把本次两图数值正确当成转换器全面正确。

  - [x] BF115.05b1 通用来源常量与帧率读取审计：开始2026-09-10 15:34:00、预估16:19:00。用户明确追加的转换器scope，作为05b静态Gate前置。fresh UE通用pin编译规则与Default编译CDO证明未连接可见pin覆盖旧结构值；SequenceEvaluator以Sequence.GetSamplingFrameRate().AsSeconds(Frame)求时，FFrameRate使用分子/分母。当前已发现固定30fps、叠加raw元数据仅读分子，以及Sequence起播/循环、ModifyCurve Alpha、MultiWay/Enum/Layered权重或时间的pin覆盖遗漏；逐项最小反例后复用读取helper，保留动态连边/绑定和明确unsupported边界。此项审计公共转换代码，不重开或安装其余12 overlay、已完成图及后续图。估时依据为已定位两类公共读取边界、既有解析/renderer测试和本轮13分钟源闭合及十分钟修复；完成后重新生成Default并继续原静态及零截图Gate。

  - 05b1修复/直接消费者检查2026-09-10 15:55:00：四个来源读取反例0.294/0.265/0.268/0.268s FAIL；复用pin helper处理数值/Bool、绑定/连边/ignored值，采样与叠加共享分数帧率读取，已声明参数结构读取当次可编辑反射默认值，六个工具自测PASS。反射移除消融0.314s FAIL；UE显式采样先Clamp的源边还揭示末帧回绕，现有实际节点fixture在timestamp0对1稳定FAIL32.347s，删除回绕后339回归35.240s PASS。公开Default最终读取45.999s、unsupported为空，两RON与当前安装完全一致；可见常量值均来自当次source。更宽的反射尝试暴露了链接引用节点的继承通知字段，已消融掉这两处无采样参数的扩展，保留原严格引用合同；显式Evaluator的非teleport/sync/relevancy路径明确拒绝，未伪装支持。四个已完成模块的九个已记录evaluator读数比较，仅Grounded Roll候选不符；fresh source已证实frame45对0，02b按唯一链有限复开，单列05b2处理生成数据和真实回归。

  - 05b1完成2026-09-10 15:55:00：来源pin、19处参数结构反射读取、分数帧率及明确支持边界的自测闭合；默认/隐藏/ignored/绑定/连边和60、30000/1001fps都有独立预期，末帧真实节点与339回归PASS。开始15:34、预估16:19、实际结束15:55（偏差-24m），结果为通用转换/采样合同完成，直接生成数据和实际入口分别由05b、05b2收口。仅提交通用工具与采样修复，生成Default和Grounded数据按各自Gate独立提交。

  - [x] BF115.05b2 Grounded Roll常量有限迁移与回归：开始2026-09-10 15:55:00、预估16:45:00。依赖05b1通用修复，沿fresh Grounded Roll frame45单一source链，公开重新生成Grounded22源子图/30RON，比较每项新差异并仅安装对应owner已有范围，补可复跑静态常量检查；再同入口构建/fresh spec/原声明式Gate与零截图startup、完整grounded九窗口回归。估时按native45.945s、既往完整转换约82秒、增量构建约126秒、原grounded Gate约43秒及本轮参数解析闭环，保留源反射字段差异的维护时间。Standing/Crouching/Locomotion只审计同一常量消费，不重做已通过功能；其余12 overlay和后续图仍待原顺序。本项通过并关闭02b后恢复05b/Default，未将已发现差异转为未授权待办。

  - 05b2生成/新增直接边界2026-09-10 16:17:00：Grounded重读先因当次反射的DefaultBlendProfile=None/FilteredBones空数组拒绝82.744s；fresh UE确认空默认无额外行为，最小反例0.314s FAIL后保留空容器语义、显式拒绝未实现非空profile/bone过滤，0.307/0.532s自测PASS，完整公开转换90.174s通过。30RON仅Roll frame45和同一Grounded FSM的原生transition/entry排序改变，排序来自已修复的通用native reader，原逻辑和值保持；02b范围补充这一同图排序差异。四模块另外七个未连接来源值候选检查发现两个Run Jump StartPosition遗漏，fresh Locomotion46.943s证实0.12对0，04b仅此边界复开，加入本项完整14窗口回归。

  - 05b2 Roll末帧根骨source闭包2026-09-10 16:17:00：Roll源length1.5s/frame45、ForceRootLock=True，父AB RootMotionFromMontagesOnly；UE GetAnimationPose仍按ForceRootLock执行FRootMotionReset，RefPose/AnimFirstFrame/Zero均有明确TRS语义。target原始glTF末帧root translation为2.75m，现有Source sampler直接保留原始root，旧frame0掩盖了漏读资产根锁设置。该边只属于当前Roll采样直接消费者，需在安装/运行前以最小实际节点反例修复；复用现有骨骼reference transform和root定位能力，将源锁定模式生成到target节点，不改原始导出、不写死root位置、不提前展开Root Motion驱动或后续CR图。

  - 05b超时复盘/05b2拆分2026-09-10 16:27:00：05b原开始15:21、预估16:06，第一轮实际结束16:27（偏差+21m，静态Gate未完成）；已完成源pin/反射/分数帧率和末帧修复、公开Default两图生成、Grounded/Locomotion候选检查及fresh确认。漂移来自用户追加同类来源值审计，以及Roll末帧首次暴露的ForceRootLock资产输入；不是等待构建或新增无依据模块。根锁最小fixture初次因宏/HashMap类型无法编译14.538s，不计行为失败；修正fixture后实际节点0.33对reference0.2稳定FAIL51.776s，复用骨骼reference/root定位后339回归39.613s PASS，来源/renderer五个自测通过。当前只剩三模式/additive消融、三图重新生成与声明式、真实回归和Default专用场景。暂停实现完成本复盘后，按原45分钟窗口的1.5倍建立16:27→17:34:30新轮，超过60分钟故拆成下列独立Gate。05b2原15:55→16:45窗口于16:27拆分（偏差-18m，未完成而拆分），同一Roll/两处起播值/排序/根锁目标继续由05b3、05b4收口，原目标不减。

  - [x] BF115.05b3 来源根锁与三图生成/静态收口：开始2026-09-10 16:27:00、预估17:07:00。接续上述复盘；完成源RefPose/AnimFirstFrame/Zero及additive scale的最小对照，复用runtime skeleton默认TRS，不新增位移补偿。公开重读Default/Grounded/Locomotion，按56源子图/78RON完整编译与原owner安装，核对源常量/原生排序/根锁元数据、同入口构建/fresh spec与声明式静态后独立提交；不启动真实运行。估时依据为339回归39.613s、最近三次公开读取约46/90/47秒、同入口148.312s，以及已有审计/manifest基座。

  - [x] BF115.05b4 三图静态后的共享真实回归：实际开始2026-09-10 16:47:00、预估17:14:30；05b3提前完成，沿原27分30秒窗口登记。用同一完整入口零截图startup、grounded九窗口及locomotion十四窗口，保留原全部公共守卫/曲线/事件/时钟/源pose chain，补读Roll固定末帧和Jump实际起播来源值；通过后关闭02b/04b有限复开、05b2及05b，单独提交对应数据/消费者。Default自身分支与0.75叠加/预测窗口仍由后续05c完整验收，不用共享回归替代。以上两个窗口合计来自本轮重估，不能自行顺延或删减失败窗口。

  - 05b3完成/05b4运行前2026-09-10 16:47:00：340 als-ext（含三根锁模式、additive scale、关闭锁定与实际节点）38.908s、296项目107.113s、同入口target_game/play168.075s PASS。三图公开重读53.919/81.497/58.961s、56源子图/78RON完全支持；只通过原owner更新数据，256文件清单原文不变，启用根锁的唯一当前片段为Roll，其余源资产明确未启用。19图实际spec2.411s、78文件编辑器修复/复查零变化，Default/Grounded/Locomotion声明式9/14/12命令6.591/8.438/6.855s全PASS，9个frame与7个可见常量输入复查均无差异。最初spec误用相对asset root导致查找exe旁目录3.802s失败，已改为参数注入真实asset root；Grounded清单替换源记录时的重复路径已去重，未放宽审计。开始16:27、预估17:07、实际结束16:47（偏差-20m），静态Gate完成并独立提交；下一步当前版本startup、grounded与locomotion，CaptureCount=0，源姿态及公共阈值保持。

  - 05b4 startup结果/受控复查前2026-09-10 17:05:00：16:49启动的公共startup17.047s退出，manual0/validator1，仅wall frame p95 26.404ms高于25，max72.341ms仍在75内；CPU p95 16.784ms、eval p95 0.1181ms，missing bones/graph errors/fault/temporal均为空、0截图。任务中断后已确认原进程结束并读取报告，没有重启未确认终态的进程；当前无game/UE/cargo/rustc。未找到与本次算法变化对应的持续开销证据，不改代码或阈值，以同一已构建版本做一次startup稳定性复查，之后才运行共享动作。

  - 05b4 startup通过/共享动作前2026-09-10 17:07:00：相同二进制、相同阈值的公共startup复查15.000s PASS，manual0/validator0、issues为空、0截图；未进行性能补偿或改变算法。56源子图/78RON、340/296回归与三图声明式保持闭合，开始完整grounded九窗口，然后locomotion十四窗口，验证本轮Roll末帧/根锁和Run Jump起播值。

  - 05b4 Grounded通过/Locomotion前2026-09-10 17:11:00：公共grounded42.653s PASS，manual0/validator0、全部九窗口、issues为空、0截图；Roll末帧/根锁与同图原生排序已通过真实入口回归。静态与340/296保持闭合，继续完整locomotion十四窗口，起播0.12来自fresh源pin，维持原canonical/时间/姿态要求。

  - 05b4/共享有限复开完成2026-09-10 17:12:00：公共locomotion30.518s PASS，manual0/validator0、十四窗口/全部coverage，frame p95 21.6861ms/max31.1493ms、eval p95 0.0959ms；missing bones/fault/graph/temporal为空、0截图。连同startup15.000s和grounded九窗口42.653s，当前56源子图/78RON及340/296回归全部闭合。02b仅Roll末帧/根锁/同图原生排序、04b仅两处Run Jump起播值的有限复开关闭；05b2、05b3、05b4及05b静态目标完成。05b4开始16:47、预估17:14:30、实际结束17:12（偏差-2m30s）；05b首次开始15:21、原预估16:06、最终17:12（偏差+1h6m），超时复盘和拆分记录保留。Default自身姿态/叠加/同步/预测仍未以真实窗口验收，继续下一段，不用共享结果替代。

- [x] BF115.05c Default实际分支观测与场景静态闭环：开始2026-09-10 17:15:00、预估18:05:00。依赖05b；沿已公开提取的AnimGraph/Overlay两图、五个固定采样节点、Idle源叠加和SecondaryMotion同步组、父Pose/Gait/预测输入，替换旧场景只看Actor状态与固定曲线的验收。复用现有真实图cache reader、窗口collector与原Playground，覆盖Standing/Walking/Crouching/InAir/预测回落的实际权重、采样时间、叠加及同步时钟，以最小失败证明旧零帧或错误Alpha不能通过，再做设计消融、适用测试、同入口和声明式静态。估时依据为两图十四Pose节点已全部生成、既有Locomotion完整观测及窗口工具、此前同类观测静态约30–58分钟；本段不启动runtime，完成后单列不超过60分钟的零截图startup/default_overlay公共Gate。其余12 overlay及Layering/Head/CR内部不在本段。

  - 05c观测/消融进展2026-09-10 17:34:00：两图与全部五个父输入保持fresh source闭合；本轮再逐字段读到预测插值上升20/s、回落5/s，原始Idle旋转跨度0.2124rad、Default参考姿态跨度2.2096rad。重复标题节点的公共reader按源ID读取反例36.846s稳定FAIL，现沿原选择函数增加ID匹配；Default只读当前图cache，逐个比较实际采样时间/pose、三处TwoWay与stance混合、Idle源reference delta、实际同步phase和最终additive结果。六窗口沿既有平地依次站立/行走/下蹲/两次跳跃/返回，第二次起跳验证保留状态后的预测回落，不用Actor状态回显替代源分支。初次编译12.129s及后续10.327s分别暴露新观测调用了正向边表/缺少BonePose字段、缓存返回值所有权错误，已修正；两项focused35.819s PASS。关闭pose比较的消融23.177s稳定FAIL于旧1.0叠加权重，已恢复；非零帧、缺失/非有限通道、20/5差异及12秒进展watchdog的最小检查保留。未修改生产图、Actor或源资产，当前进行299项目回归和同入口/static，尚无Default runtime结论。

  - 05c恢复/静态进展2026-09-10 17:36:00：关闭姿态比较的消融失败后，首次复制恢复保留旧文件时间，Cargo在2.143s沿用消融二进制而再次报同一预期失败；确认源码已恢复后刷新文件时间，299项目29.695s PASS，包含源ID reader与非零帧/权重/插值/watchdog反例。两条原始clip均为79骨骼/237通道，实际比较不依赖固定骨骼payload。场景复用已验收的Grounded平地与+X方向，六窗口只保留必须的进展、预测历史和一次跳跃记录，无新生产动画状态。正在实际target_game/play预构建，随后fresh spec与原两图声明式；未启动runtime。

  - 05c完成2026-09-10 17:39:00：最终299项目29.695s、公共数值validator自测0.574s、target_game/play双example67.578s、19图fresh spec3.168s及Default完整两图声明式九命令4.267s PASS，编辑器两文件零修复0.305s。第一次编辑器调用误用不存在的--repair参数，在0.312s明确退出；读usage后改用原--fix，未改审计器。源图/十四Pose节点/原始资产保持，新增观测只运行于manual case；旧曲线/Actor模式五个coverage已替换为实际六窗口及采样/叠加/预测合同。开始17:15、预估18:05、实际结束17:39（偏差-26m），结果为观测与场景静态闭合，无runtime结论；本段独立提交后进行05d。

- [x] BF115.05d Default完整无截图公共Gate：开始2026-09-10 17:39:16、预估18:09:16。依赖05c；使用同一已构建target_game/play，先startup再default_overlay六窗口，两次真实跳跃覆盖预测上升/回落，逐节点核对五个源采样帧、实际混合、Idle reference delta/同步phase与叠加输出，并保留公共Root/pre-rig/final/时间/性能和进展守卫。估时依据为三Grounded窗口稳定采样各至少0.5秒加当前Idle2.666667秒同步周期、两次原跳跃和既有startup约15秒、当前增量构建约68秒；保留首个观测或source链失败的最小诊断/修复时间。CaptureCount=0，所有原阈值保持；通过后关闭Default模块并沿原顺序进入Layering，另外12个overlay继续按用户要求保留待办，不在本轮执行。

  - 05d startup通过/Default动作前2026-09-10 17:40:00：公共startup15.520s PASS，manual0/validator0、0截图，missing bones/fault/graph/temporal为空；当前入口5150ef1、两图/十四Pose节点、299项目与完整声明式保持通过。开始原Playground六窗口Default场景，核对所有源节点采样、叠加/同步及两次跳跃的预测插值，全部公共阈值保持。

  - 05d首轮结果/05b有限复开2026-09-10 17:46:00：Default19.445s manual91/validator1，case0在12秒上限自动结束、六窗口尚未产生；公共pose pass=true，无fault/graph/temporal、0截图。实际站立frame0与0.75叠加结果均存在，唯同步reader始终无Secondary Motion；核对组名与源/生成完全相同。fresh UE FAnimNode_LinkedAnimGraph::Evaluate_AnyThread把完整FPoseContext交给linked root并原样返回；target Default AnimGraph里的LinkedAnimLayerNode只转发Pose数据，未保存子图AnimationContextState，导致十二条层曲线、同步tick等在唯一当前generated调用点被丢弃。此前测试只约束pin alias/selector与直接Root同步，没有组合linked layer和隐式context，静态spec也不验证这条执行边。05b仅此当前Default直接链接输出合同取消勾选；先为真实Linked→Root组合补最小失败，复用现有graph_output_context_state保存当前输出，不增加Actor曲线回填或时钟。Layering只作为父图已有直接consumer记录，不展开其内部算法。

  - 05d直接曲线观测边界2026-09-10 17:49:00：上述实际Linked→Root fixture34.933s稳定FAIL于context缺失，生产修复仅把子Pose输出对应的既有AnimationContextState转交到当前linked节点。进一步读本地UE GetCurveValue及ALS既有曲线getter，均明确缺失曲线返回0；Default资产只含十二条值1的层曲线，另六条是源缺失值。旧Default曲线观测要求全部十八条显式存在，属于同一当前输出链的错误验收前提；05c仅此观测条件有限复开，补十二条存在/六条缺失与必要曲线丢失的最小对照，不向生产数据注入六条假曲线。下一步恢复完整static后再startup/default，不根据一次场景失败重做其它overlay或Layering内部。

  - 05d链接/曲线修复静态进展2026-09-10 17:55:00：Linked→Root真实三帧合同34.933s FAIL→22.674s PASS，保留隐式LayerSpine、authoritative标记及0.05/0.10/0.15同步phase；实现只增加现有context的输出转交，旧无context路径即消融。Default缺失曲线反例42.669s FAIL；按源getter以0读取缺失值，仍拒绝必要层曲线缺失、错误数值及NaN，341 ext/300项目36.564s PASS。只改当前唯一generated LinkedAnimLayer调用的通用输出行为和Default观测，不改生成资产、输入ABI或时钟；当前完成同入口重建后刷新spec/static，再执行公共startup与六窗口。

  - 05d链接修复静态闭合/运行前2026-09-10 17:59:00：341 ext/300项目36.564s、同入口双example114.555s、19图fresh spec3.199s和完整Default声明式九命令4.008s PASS。05b仅Linked输出合同、05c仅源缺失曲线读取的静态有限复开关闭；完整Default运行仍待05d。当前两图数据不变，root-linked输出恢复来源曲线与同步，先startup再六窗口Default；随后复用原grounded九窗口与locomotion十四窗口检查父图接收这些层曲线的直接影响，不展开已完成模块的新source或其它overlay。全部CaptureCount=0，原曲线/pose/时间/性能守卫保持。

  - 05d链接修复startup通过/动作前2026-09-10 18:00:00：公共startup15.517s PASS，frame p95 13.8107ms/max23.4446ms、eval p95 0.0859ms，0截图且missing bones/fault/graph/temporal为空。新Linked输出在真实入口初始化通过；341/300回归、两图fresh静态保持。当前执行完整六窗口Default，随后两个已声明直接消费者回归，不只重跑旧失败的站立窗口。

  - 05d六窗口通过/公共边界2026-09-10 18:10:00：第二轮Default18.353s manual0全部十六coverage通过，六窗口/五采样帧、0.75实际叠加、十二曲线与同步、两次跳跃的预测升降全部正确，source逐帧errors为空；validator仍拒绝。其一是现有方向签名把同步phase模式统一要求FSM，而fresh Default两图无FSM；05c仅对齐元数据复开，已用同一报告在focused对照中证明，将0.35 phase等价换算为Idle源时长的clip time后原验收器全部签名通过，接着复用其既有clip-time路径，实际状态继续为空。其二是第一次从下蹲恢复后的起跳，final calf_r在0.0201296s转1.0502765rad（52.1757rad/s），pre-rig无异常；当帧脚IK权重正在回落，足锁/trace和源BasicIK写回是当前唯一待定位链。尚未证明算法差异，不复开已完成FootIK条目或放宽40rad/s；先沿当次样本与本地对应源函数比较。0截图，两个共享case仍待此Gate闭合。

  - 05d超时复盘2026-09-10 18:30:00：开始17:39:16、原预估18:09:16，本轮实际结束18:30（已耗50m44s，偏差+20m44s，Gate未完成）。暂停实现及新的runtime后，回读两图/十四Pose节点、341/300回归与两次实际运行：Linked context丢失已以5行复用转交修复，第二轮六窗口及全部Default来源值/混合/曲线/时钟/预测通过；仅同一次final calf_r突变和无FSM签名元数据未闭合。签名已复用既有clip-time能力，以实际Idle时长乘0.35对齐同一phase，不新增验收模式；旧元数据32.908s FAIL，新元数据30.802s PASS，同一保存报告的等价时间对照也通过。突变后公开CR只读79.248s得到fresh native，分析仅到当前FootIK的直接三个函数及UE BasicIK/ALS偏移CPP；部分权重写回、射线启用阈值与组件空间pole缓存已确认相符，尚未证实应改的姿态算法。已定位报告applied_weight取旧AlsFootIkRuntime平滑权重，而实际求解使用当前曲线，两者在失败帧为0.466782/0.209299；先纠正该读数并保存实际求解TRS，避免继续凭不完整数值推断。原6+9+14窗口及后续模块顺序全部保留，不复开无source证明的已完成算法。
  - 纠正与拆分：按原30分钟估时窗口1.5倍，下一轮开始18:30:00、预估19:15:00。拆成下列数据/静态及完整运行两个独立Gate；依据为Default已完成实际分支、唯一右小腿失败帧和fresh源求解输入闭包，先建立可复算数据再沿首个实际分歧修复，不能用重复运行或放宽阈值代替定位。
  - [x] BF115.05d1 实际IK读数与可复算输入静态闭环：开始2026-09-10 18:30:00、预估18:55:00。只修正当前Gate已证实的权重观测来源，复用现有telemetry记录每次实际BasicIK的输入TRS/目标/权重，保留无FSM clip-time修复；完成最小失败/消融、适用回归、同入口及source/static合同。本段不改未经证明的动画/足锁/求解器，也不启动runtime。
  - [x] BF115.05d2 完整Default与直接消费者公共Gate：实际开始2026-09-10 18:57:01、预估19:15:00（沿用原终点）。依赖05d1；用同一入口startup、完整Default六窗口及已声明grounded九/locomotion十四窗口。若仍失败，使用实际输入复算源/target首个分歧后最小修复和回归；保留全部守卫及0截图，不提前进入后续模块。

  - 05d1读数/输入静态进展2026-09-10 18:40:00：实际post-IK telemetry反例36.280s稳定FAIL（旧读数0对本次实际weight0.20929879）；移除其AlsFootIkRuntime依赖，改读实际执行返回记录，未执行时为0，301项目26.577s PASS。每次BasicIK返回同次输入的armature/thigh/calf/foot世界TRS、目标/旋转、pole、骨骼本地轴、实际长度和weight，复用现有space telemetry保留相邻两帧，未将这些记录回喂动画算法。旧平滑权重路径即消融失败；新增邻帧JSON导出/未执行null检查后完成同入口和静态。当前只改观测与证据载荷，源FootLock、Offset与BasicIK数学保持不动，仍未启动新runtime。

  - 05d1完成/05d2运行前2026-09-10 18:54:44：301项目26.577s与新增邻帧JSON导出26.855s PASS（共302项已验证），最终同入口64.959s、19图fresh spec3.208s及Default完整声明式九命令4.521s PASS。六窗口使用实际Idle时长换算的clip-time对齐，仍记录实际同步组，不伪造FSM；05c本轮元数据静态复开关闭。输入记录在现有space中每帧只保留当前/上一执行，跳过求解为null；Default原逐帧trace同时保存当前输入，异常落在稳定窗口之外也可复算。求解器、足锁、偏移、曲线与40rad/s守卫均未改。开始18:30、预估18:55、实际结束18:54:44（偏差-16s），本段静态完成并提交；接着同一入口startup和完整Default，物理异常是否重现仍是开放边界。

  - 05d2 startup通过/完整动作前2026-09-10 18:59:00：新入口1921adc公共startup17.107s PASS，frame p95 21.6546ms/max27.8623ms、eval p95 0.1416ms；fault/graph/temporal为空、0截图。301全量和新增邻帧导出、19图fresh spec及完整声明式通过；开始原六窗口Default，当前/前帧实际IK参数及逐帧输入随同原数值报告保留，不改变角色输入、姿态算法或守卫。未提前认定此前右小腿突变已修复。


  - 05d2完整动作结果2026-09-10 19:04:00：Default19.259s manual0，六窗口、五源采样帧、实际叠加/同步/曲线及预测升降全部通过，source errors逐帧为空，无FSM签名问题已消失；公共validator仍失败。第一次恢复站立后起跳的final calf_r在0.0159496s转0.9080184rad（56.9305rad/s），pre-rig无异常；已得到同次与上一帧完整求解输入，权重分别0.257388/0.544095。frame p95 26.1172ms/max193.0519ms也超预算，CPU p95 23.4558ms/max34.7292ms，eval p95 0.1165ms；0截图。当前先复算UE BasicIK与实际target参数的首个差异，不重启场景，不放宽原守卫；grounded/locomotion直接消费者仍待该边界闭合。


  - 05d2源输入时序闭包2026-09-10 19:14:34：用保存参数逐式复算本地UE BasicIK，前后帧局部差0.908025rad，与实际结果误差1.84e-6rad；因此不改solver数学。fresh AnimInstanceProxy先NativeThreadSafeUpdate、后节点Update/Evaluate，GetCurveValue读取上一完成的属性曲线；ALS RefreshFeetOnGameThread读取已有骨骼，RefreshFeet准备锁脚Final，GetControlRigInput再交给Rig。fresh CR RefreshFootOffset/RefreshFootIk都读取同一RigInput.FootLocation，Weight读取Rig当前曲线。target却在求值后重算FootLock且射线另取新IK骨，已准备的AlsControlRigInput足部字段未被消费。BF109.10只按该输入阶段/consumer边界取消勾选，旧Gate仅验存在/阶段mask和单帧数学，未验证前后两帧生产顺序。接着复用现有FootLockInput、FootLockState和RigInput，把锁脚准备移到新图之前，并让trace/solve读取同一准备值；不新增姿态平滑/阈值，不展开其余Rig模块。


  - 05d2输入阶段静态闭合/运行前2026-09-10 19:31:10：真实ECS顺序反例36.860s稳定FAIL，现将既有足锁准备移到图前、由TransformHelper读取当前组件变换下已有骨骼，保留原缓存/传送/constraint算法；26.618s PASS，并扩为两帧，确认当前图的新骨骼/曲线只供下一次准备。Standing/Crouching既有FootLockInput补全IK曲线，Rig消费同一准备位置/旋转，pelvis读取已更新的源PoseState。304项目27.649s PASS；首轮唯一失败为旧传送fixture只写GlobalTransform，按真实Transform层级补齐后原阈值通过。关闭目标一致性观测22.952s FAIL、恢复22.903s PASS。fresh CR独立生成0.567s，runtime参数逐字不变；133node/1407pin/160link/79骨静态0.584s及最终十命令6.103s PASS。同入口76.435s、19图fresh spec3.238s通过。以下先startup，再完整Default及原grounded九/locomotion十四窗口；当前只宣称静态闭合，原小腿突变和帧时仍待真实复验。


  - 05d第二轮超时复盘2026-09-10 19:35:00：本轮开始18:30、预估19:15，实际复盘19:35（耗65m，偏差+20m，未通过）。暂停实现/新的runtime，回读两图/十四Pose节点、直接三函数与UE执行阶段及完整报告：六Default窗口来源合同已通过；旧最终小腿参数复算数学相符，源差异为足锁准备阶段和Rig目标绕读。新增图前准备、同一目标消费与304回归通过，但startup5.229s在调度初始化失败、无report.json；公共runner正确拒绝进程101。原因是新增after(AlsRotationResponseSet)实际在Physics Last之后，而现有图求值在PrePhysics，形成环，旧最小fixture未带真实物理顺序边。补相同First→Last→RotationResponse边后25.820s稳定FAIL于该环；需删除这条多余顺序约束，保留在现有图更新阶段内准备。进一步按source FinalLocation=ComponentInverse(FinalWorld)确认组件空间值应在准备时固定，不能由图后变化过的组件坐标再次推算；继续复用现有RigInput足部字段保存，不新增状态。BasicIK、曲线/动画资产、物理时钟及所有守卫不改；只有当前输入链，BF109.10尚未关闭。
  - 纠正与拆分：按上一轮45分钟窗口1.5倍，下一轮开始19:35:00、预估20:42:30，拆成下列两个独立Gate，剩余估时基于25秒focused/约76秒同入口和三次主要真实case约两分钟，并保留当前组件快照反例与首个源链失败定位。此为同项第三轮预估；若仍需第四轮，先重审全过程和拆分，不沿用本轮验证路径。
  - [x] BF115.05d3a 图前组件空间足输入闭环：开始2026-09-10 19:35:00、预估20:10:00。在已证明的调度环与RigInput坐标边界上补最小跨帧失败、删多余顺序约束并复用现有输入字段；适用消融/回归、source/static及同入口闭合后单独提交。本段不启动runtime。
  - [x] BF115.05d3b 全部Default与直接消费者Gate：实际开始2026-09-10 19:49:00、预估20:42:30（保留本轮终点）。依赖05d3a；startup→完整Default六窗口→grounded九/locomotion十四窗口，保留图/Root/pre-rig/final/时间/性能与watchdog、0截图。足输入阶段修改直接影响旋转约束和站蹲writer，另用既有standing_turn与crouching完整case复验该消费者；不展开新的相邻图或后续模块。


  - 05d3a完成/05d3b运行前2026-09-10 19:49:00：保留真实物理顺序的调度环反例25.820s FAIL；删去多余after(RotationResponse)后，组件输入未准备反例36.390s FAIL，现直接把当次FinalLocation/Rotation转换并保存到已有RigInput，图后只保留这些字段，26.652s PASS。304项目22.655s PASS，原source目标一致性消融保持；同入口67.495s、19图fresh spec3.142s、十命令source/图/编辑器/当前Rig审计4.528s PASS。仅增加一个已有SystemSet阶段并移动原足锁逻辑，复用TransformHelper与原输入字段，无新逐帧缓存/求解器/平滑。开始19:35、预估20:10、实际完成19:49（偏差-21m），静态子项闭合；公共startup后再完整Default、grounded/locomotion及站蹲直接回归，仍未宣称数值突变消除。


  - 05d3b startup通过/完整Default动作前2026-09-10 19:52:48：c6f1bc8公共startup15.749s PASS，frame p95 18.5099ms/max23.6924ms、eval p95 0.0834ms；fault/graph/temporal为空、0截图。304回归、真实物理顺序fixture、19图fresh spec和十命令静态已通过；当前开始完整Default六窗口及两次跳跃，不以startup替代此前小腿故障的验收。


  - 05d3b Default复验结果2026-09-10 19:54:40：18.608s manual0，全部14 coverage/六窗口通过，新trace与solve同目标合同逐帧通过、source errors为空；frame p95 23.3812ms/max28.5083ms、eval p95 0.0819ms通过，0截图。公共时间守卫仍捕获第一次起跳final calf_r：0.79758775rad/0.016706s=47.742596rad/s，elapsed9.2916026；pre-rig无异常。输入阶段/组件字段修复已通过自身合同，但不足以关闭整个Default；保留当前/前帧实际参数，先复算余下源参数与位置限制，不重复启动或放宽40rad/s。共享case尚未开始。


  - 05d3b输入reader反例/05d3a有限复开2026-09-10 20:00:41：新失败帧复算pre-solver局部变化25.9181rad/s、给定参数BasicIK后47.7428rad/s，与实际一致；改用源初始骨长的受控对照仍47.1407rad/s，不能把该相邻差异当成根因。实际pelvis_write_delta=0且trace有负offset，追踪我本轮改动发现PoseState由provider写普通global input，而source_player_property只读原生状态/source_frame_input命名空间，导致准备好的PoseGrounded/InAir全读0。此前304回归漏了这两个输入到实际reader的正值组合；05d3a仅此读数边界取消勾选，先补实际player输入1/0与0.25/0.75/预测0.4的最小失败。目标仍读取源更新阶段PoseState，不退回当前帧曲线；不改BasicIK长度或阈值。


  - 05d3a reader有限复开关闭/05d3b再运行前2026-09-10 20:09:16：实际player输入正值反例32.643s FAIL（错误命名空间返回0）；改用已有普通图输入读取，305项目25.279s PASS，覆盖grounded=1、0.25+0.75×0.4=0.55及全空中无预测=0。旧reader即消融失败；同时删除移动函数时重复的lint属性。最终同入口68.295s、19图fresh spec3.182s、十命令静态4.522s PASS。源PoseState准备阶段与组件空间足目标合同保留，未改Rig数学、长度或守卫。本次有限复开从20:00到20:08完成，仍在原20:10静态终点之前；下面新startup再完整Default，旧47.7426rad/s尚未认定消除。


  - 05d3b pelvis reader修复startup通过/动作前2026-09-10 20:10:54：0b6abe5公共startup15.536s PASS，frame p95 17.5789ms/max24.3286ms、eval p95 0.0732ms，fault/graph/temporal为空、0截图。305回归及当前source/static保持；继续原完整Default六窗口，逐帧检查准备目标、实际采样/叠加/预测与全部公共守卫。


  - 05d3b pelvis正值恢复后的完整结果2026-09-10 20:14:10：18.652s manual0/全部六窗口及coverage通过，frame p95 21.4322ms/max25.5170ms、eval p95 0.0766ms通过，0截图、无graph/source错误；第一次起跳final calf_r仍0.7773805rad/0.0162968s=47.70142rad/s，公共validator拒绝。此前错误reader已用正值反例修正，但不是该突变的充分根因。给定实际参数的BasicIK复算与输出一致，下一步沿已有AB_Als中Default→Layering→Rig直接Pose/曲线连接核对实际输入，先只读该消费者边界；未证明的Layering内部、Head或其他Rig算法不展开，不放宽阈值或开始共享动作回归。


  - 05d3b Layering直接输入闭包2026-09-10 20:25:00：本地Layering公开只读75.821s得到fresh五图，当前只沿Default/Locomotion输入→腿部重组→骨盆骨骼mask→输出曲线两节点分析。腿/骨盆叠加Alpha=1、最终曲线Accumulate→Override与目标一致；不据此扩展其他区域。骨盆LayeredBlend的source过滤为pelvis depth0、thigh_l/r depth-1并开启mesh-space。UE AnimationRuntime的普通/ISPC路径对零权重骨保存SourceRotations，并以混合后parent逆旋转回local；target共享apply_mesh_space_layered_blend却使用混合后parent×旧local，让排除双腿继承骨盆旋转。该函数由生成SourceLayeredBoneBlendNode实际调用，位于当前Rig输入链；BF109.05仅此mesh-space零权重边界取消勾选，旧fixture从未改变未选child的parent，漏检该不变量。下一步用同一个两骨fixture覆盖父骨部分/满权重、child显式/缺省旋转，先FAIL后最小修复。

  - [x] BF115.05d3c 当前Layering零权重子骨静态闭环：开始2026-09-10 20:21:00、预估20:42:30。仅修fresh source证实的共享mesh-space零权重语义，复用已有Skeleton/rotation缓存与父级转local路径，完成最小失败/消融、全部实际调用者适用回归、同入口和声明式；不改变图、mask、Alpha、曲线或求解器。本段完成后恢复05d3b原全部真实case，仍使用第三轮终点，若到容差上限尚未完成则执行第四轮前全过程复盘。估时依据为单个共享函数、已有两骨fixture、约30秒底层/项目回归、约110秒共享crate入口构建及当前source已完整读取。


  - 05d3c共享mesh-space静态进展2026-09-10 20:43:00：父骨旋转/零权重child反例35.546s FAIL；目标改为保留已有source_mesh旋转，仍沿原parent逆旋转回local，25.046s PASS。稀疏Pose在父骨变化时必须写入补偿：单独移除此条件23.173s FAIL；恢复后342底层/305项目45.626s PASS，空输入/空mask仍为空、未制造参考Pose通道。普通/ISPC两条UE分支均确认相同语义。生产修改只替换零权重旋转来源及必要的稀疏child写入条件，图/mask/Alpha/曲线和Rig数学不变；当前同入口构建中。复用已有reader给Default原trace补实际Standing状态、Transition播放器与pelvis/legs权重，下一次完整运行可判定首次起跳时该输入边的实际参与，不改动作或守卫。


  - 05d3c静态完成/05d3b恢复运行前2026-09-10 20:48:25：342底层/305项目45.626s、最终同入口134.201s、19图fresh spec3.227s及十命令声明式6.347s PASS。父部分/满权重、child显式/稀疏与空Pose边界均通过；单独移除稀疏补偿条件的23.173s失败已恢复。fresh Layering与UE普通/ISPC源记录纳入当前manifest；未修改Layering图/mask/Alpha或其他区域。开始20:21、预估20:42:30、实际静态完成20:47:25（偏差+4m55s）。以下新startup再完整Default，并读取新增的实际上游状态；BF109.05的有限复开与整个Default仍待真实Gate，不以共享函数正确替代故障消除。


  - 05d3b mesh-space修复startup通过/完整动作前2026-09-10 20:52:04：bc1fb22公共startup15.503s PASS，frame p95 17.9241ms/max21.3746ms、eval p95 0.0676ms；fault/graph/temporal为空、0截图。342/305回归、134.201s同入口、fresh spec与十命令静态闭合；以下完整Default保留原动作和全部守卫，并记录已声明的上游状态。


  - 05d3b最新完整结果与真实冻结输入2026-09-10 20:58:34：bc1fb22 Default18.634s manual0，六窗口及全部自身coverage通过、frame p95 21.87ms/max39.1809ms、0截图；时间守卫仍拒绝final calf_r 0.7693371rad/0.0152335s=50.50298rad/s。新增实际读数证明首次起跳正在播放A_Als_Stand_Transition_Right，满权重时Standing底图未求值；本地UE Slot默认bAlwaysUpdateSourcePose=false且满权重跳过Source，与目标相符，不修改。停止后播放器clip_time持续冻结于1.20329547，而weight依次0.9366/0.8653/0.7989/0.7227；fresh FAnimMontageInstance::Stop仅在BlendTime<=0时设置bPlaying=false，Advance在正常淡出仍推进Position。target AlsTransitionPlayback::advance在stop_elapsed分支提前return、没有推进elapsed/clip_time，构成当次实际帧序列的明确源差异。
  - 第四轮前全过程复盘2026-09-10 20:58:34：第三轮开始19:35、预估20:42:30，到本次复盘仍未完成（偏差16.1分钟）；暂停实现与新runtime，重读05a–05d、原source闭包、输入参数和各失败报告。Default转换值/固定帧/叠加/曲线/同步/预测已实际闭合，Linked context与无FSM签名分别由组合fixture修正；之后独立数学复算排除BasicIK给定输入的求解误差。足锁准备/组件空间/同目标消费是确证差异，迁移中错误的PoseState reader已用正值fixture修正；共享mesh-space排除骨语义也有独立source反例，但都未充分解释首次起跳故障，不能宣称故障已修复。主要验证缺口是最早只记录Rig结果，未同时保存实际Slot播放器生命周期，直至本轮才能看到满权重抑制底图和淡出时钟冻结。源骨长受控替换仍超过47rad/s，不作为当前修复。此次从真正参与失败帧的单一播放器停止路径开始，先证明时间推进，再核对原Default整段和共享生命周期；保留原所有动作与阈值，不改场景等待来回避重叠。
  - 纠正拆分：按上一67分30秒窗口的1.5倍，第四轮开始2026-09-10 20:58:34、预估2026-09-10 22:39:49。剩余被拆为三个各不超过60分钟的独立Gate：源停止时钟静态、Default完整场景、原共享回归。估时以单个共享advance函数及现成source停止fixture、约25秒项目回归/约70秒同入口、完整Default约19秒及其余四case记录为依据，余量用于首个仍未闭合的实际源输入链；不再无证据扩展骨骼算法。
  - [x] BF115.05d4a 停止请求后的播放时钟：开始2026-09-10 20:58:34、预估2026-09-10 21:33:34。沿fresh UE Stop/Advance和当前实际Transition更新→采样唯一消费者，扩展旧停止fixture使其先拒绝冻结时间，再把既有时间推进放到淡出分支之前，保持权重/结束/夹末帧逻辑。适用回归、同入口/source/static通过后单独提交。本段不启动runtime。
  - [x] BF115.05d4b Default全部原输入：实际开始2026-09-10 21:10:13、预估2026-09-10 21:45:13。依赖4a；新startup及完整六窗口、两次跳跃，保留首次起跳与过渡动画的真实重叠，确认停止后的实际clip_time推进及公共Root/pre-rig/final/时钟/性能守卫。CaptureCount=0。
  - [x] BF115.05d4c 共享动作与Default收口：预定开始2026-09-10 22:08:34、预估2026-09-10 22:39:49。依赖4b；原grounded九/locomotion十四及已登记standing_turn、crouching完整回归，关闭相关有限复开和Default；全目标随后仍进入Layering→Head→CR→AB_Als，其余12 overlays留待用户另行安排。

  - 05d4a静态完成/4b运行前2026-09-10 21:11:00：扩展既有站立/下蹲停止fixture后33.474s稳定FAIL于冻结clip_time；将已有elapsed/clip_time推进移到停止淡出分支之前，305项目21.020s PASS。半程/结束权重、正淡出期间采样时间及停止后的静止时钟均覆盖，旧顺序即失败消融；无新状态、参数或分支。最终同入口67.499s、19图fresh spec2.898s、十命令声明式4.520s PASS，fresh UE Stop/Advance和Slot源记录匹配。开始20:58:34、预估21:33:34、实际静态完成21:09:28（偏差-24m06s），该静态子项闭合；下面提前开始4b新startup及原完整Default，停止时钟的实际推进和小腿公共守卫仍待真实验证，相关归档有限复开保持。

  - 05d4b startup通过/完整动作前2026-09-10 21:10:45：96b8613新入口startup15.469s PASS，Root/图/时间/性能守卫通过，fault与graph错误为空、0截图。305项目回归、19图fresh spec与十命令静态均已通过；接着原Default完整六窗口及两次跳跃，检查实际Stop淡出时间推进并保留既有全部守卫。

  - 05d4b完整动作结果2026-09-10 21:12:00：96b8613 Default18.665s manual0，六窗口和全部14coverage通过；frame p95 22.9514ms/max32.9712ms、eval p95 0.0811ms通过，0截图/graph错误。首次起跳final calf_r仍0.79880106rad/0.0166967s=47.84185rad/s，公共validator失败。停止淡出时钟已通过最小反例，但不是全部故障的充分原因；保留本次逐帧实际Slot、Layering、源输入与相邻求解参数，先读取首个实际输入差异，不重复运行或放宽守卫。共享四case仍依赖此边界。

  - 05d4b输入混合源闭包/05c有限复开2026-09-10 21:22:00：fresh本地TwoWayBlend→BlendTwoPosesTogether→BlendTransform及MultiWayBlend→BlendPosesTogether均对姿态四元数加权累加后归一化；当前SourceTwoWayBlendNode与SourceMultiWayBlendNode却调用依赖LinearInterpolator的slerp。实际Default及Layering腿链的TwoWay和Base Poses的Multi均为直接消费者；现有Default逐帧检查也复用同一个目标插值器，无法拒绝该差异，因此05c仅普通混合算法观测边界取消勾选。复用依赖已有Pose.scalar_mult/linear_add/normalize_quat和项目已有root-motion加权函数，不改依赖源码、图资产、Alpha、曲线、BasicIK或守卫。此前对满权重Slot抑制Standing的推断纠正：当前Transition采样为mesh additive，Standing不可见不能归因于普通Slot早退；Grounded输入在Locomotion空中分支下不相关才是应核对的边。停止时钟修复本身仍由UE Stop/Advance独立证明。
  - [x] BF115.05d4d 当前普通姿态混合source/static闭环：开始2026-09-10 21:22:00、预估2026-09-10 21:57:00。依赖4a；在原4b预算内先补两输入/三输入、非归一化权重与四元数等价符号的实际图反例，修改两类生成节点的共享加权消费与Default observer，完成消融、适用回归、同入口和source/static后再恢复4b。估时依据为两个调用类、现成Pose线性代数和约46秒回归/135秒入口构建；第四轮总终点22:39:49不变，4b原终点未顺延。

  - 05d4d普通混合反例2026-09-10 21:28:00：首个fixture因未提供Alpha而失败，修正实际图输入前提后20.533s在两输入/三输入及0.6总权重上稳定FAIL；复用已有Pose加权累加及root-motion函数后24.976s PASS。fresh同次AnimationRuntime及TransformVectorized进一步证明Default的0.75 ApplyAdditive使用BlendFromIdentityAndAccumulate（归一化线性旋转、乘法缩放）；目标依赖AdditiveInterpolator为slerp且加法scale，Default observer同样复用目标算法。该节点已在Default十四Pose闭包，4d纳入其直接局部叠加消费；复用已验证mesh additive的旋转与非旋转运算，不改资产、Idle倍率或Alpha。

  - 05d4d回归/消融闭合2026-09-10 21:36:00：局部叠加0.25/0.75和非单位base scale反例27.890s FAIL，复用既有mesh-additive的归一化旋转/乘法scale运算后26.846s PASS；不增加速率或权重状态。普通混合保留现成root-motion加权路径，单独移除该路径20.605s FAIL，恢复后343底层/305项目49.768s PASS。首轮共享回归唯一失败是旧FSM fixture只填bones而无paths；补一条实际骨索引后原状态/生命周期断言通过，无FSM算法改动。Default普通混合和局部叠加的observer改按源加权公式计算预期，已知wrong-slerp生产反例和root步骤消融均不能通过。当前同入口构建中，原完整Default和共享四case仍未关闭。

  - 05d4d完成/4b恢复运行前2026-09-10 21:40:15：343底层/305项目49.768s、同入口154.944s、19图fresh spec3.425s及十命令声明式8.791s全部PASS。普通两/三输入的四元数加权、原始非归一化权重、局部叠加0.25/0.75与非单位base scale、等价四元数符号及root motion均由同一个实际图fixture覆盖；原slerp/scale与移除root处理均稳定失败。两普通节点复用既有Pose加权API与现成root函数，局部叠加复用已有mesh-additive的旋转及非旋转运算；未新增运行时状态、参数、资产或依赖。开始21:22、预估21:57、实际静态完成21:39:59（偏差-17m01s），4d与05c本次观测有限复开关闭。接着新startup再完整Default，原首次起跳时间异常尚待验证。

  - 05d4b新混合算法startup失败2026-09-10 21:41:25：e99b6a4 startup20.799s被性能守卫拒绝，frame p95 43.117ms/max1135.1868ms，eval p95 0.0913ms/max0.116ms；Root/姿态/图无fault，0截图。343/305回归、19图fresh spec和十命令静态通过。后续调用未根据该非零结果分支而误启动Default，属于执行流程错误，不能计作有效联合Gate；先收取该进程终态和性能阶段报告，再定位startup卡顿并恢复正确前置顺序，未以新算法已通过runtime作结论。

  - 05d4b失败诊断/4d有限复开2026-09-10 21:46:00：误启动的Default21.868s在首窗口12秒watchdog退出（manual91）；未到起跳，不能声称原小腿异常消除。逐帧只有apply additive pose mismatch，说明新局部叠加与当前observer的真实资产通道尚未闭合；4d/05c仅该新边界取消勾选，补Default/Idle/Stand三个实际加载资产的focused对比后修正，不启动新runtime。startup帧时样本的主要卡顿计入ControlRig wall time（max1107.6611ms），实际动画eval max0.116ms；当前另有非本任务calibre处理进程活动，未修改或结束其进程。先静态定位姿态差异，再检查正确前置Gate；4b原21:45:13终点及其20分钟复盘边界保持。

  - 05d4d真实通道反例闭合2026-09-10 21:52:00：新加载Default/Idle/Stand三资产的focused test39.854s稳定FAIL（新局部叠加80通道，源输入/独立预期79通道）。移除本轮新增的整套reference骨预填，只为叠加确需且base缺少的骨取参考变换，44.914s PASS；同fixture再验证空base加单骨delta仅产生1条必要通道。343底层/306项目36.846s PASS；原四元数、缩放和root-motion反例仍通过，严格79通道及2e-5误差守卫不变。该失败来自本轮实现的多余预填，未改source资产或扩大容差；当前同入口构建中，4d待最终声明式、4b待正确顺序的startup/动作。

  - 05d4d通道有限复开关闭/4b运行前2026-09-10 21:55:00：343/306回归36.846s、最终同入口118.344s、19图fresh spec3.418s及十命令声明式4.473s PASS。三实际资产覆盖两种base采样、三处Idle时间，原79通道不变；稀疏输入仅补1条叠加必需通道，整套预填已消融删除。有限复开开始21:46、原预估21:57、实际静态完成21:54:34（偏差-2m26s），05c/4d静态重新闭合；下面先单独startup，确认通过才启动完整Default。

  - 05d4b通道修复后startup通过/完整动作前2026-09-10 21:55:45：405529e入口startup15.397s PASS，Root/pre-rig/final/图/时间/性能通过，fault与graph错误为空，0截图；此前约1.1秒卡顿未重现。343/306及新真实资产通道反例、19图fresh spec与十命令静态均闭合。已单独检查startup成功结果，现在才启动原完整Default六窗口和两次跳跃。

  - 05d4b最新完整结果2026-09-10 21:57:00：405529e Default18.530s manual0，全部14coverage/六窗口通过，逐帧source errors为空；frame p95 22.2437ms/max36.6679ms、eval p95 0.0814ms通过，0截图/graph错误。首次起跳final calf_r仍0.82149166rad/0.0171461s=47.911285rad/s，公共Gate未通过。时钟推进、输入准备、mesh-space排除子骨和普通/局部叠加均有独立源反例，但仍未充分解释此时间异常。后续只读失败帧直接相关的角色移动→原生输入→图求值→Rig阶段，与UE Character/CMC/Mesh的实际tick依赖核对；不展开新动作或放宽守卫。

  - 05d4b超时复盘2026-09-10 22:06:06：开始21:10:13、预估21:45:13，当前55m53s、偏差+20m53s，结果未完成。已暂停实现与新runtime，回读同一源闭包和所有本轮报告：停止时钟已推进；普通/局部叠加误用slerp及多余参考骨预填分别修复，343/306和源静态通过；正确前置startup与全部Default自身窗口通过，但最终右小腿仍47.911285rad/s。一次startup性能失败后误启动动作的执行错误已明确作废，后续已恢复逐步确认，未计为成功。主要漂移是连续定位到独立真实差异，却未证明原故障的充分原因；本轮后续先验证完整输入的执行时序，不继续列举下游数学候选。fresh ACharacter强制Mesh在CMC之后、ALS再强制Mesh在Actor之后；CMC只在有效root-motion路径提前TickCharacterPose。目标Tnua逻辑/马达在Update、图在PostUpdate的PrePhysics、骨骼写入/Rig在physics之后，需先核对当前Transition是否启用root motion和各阶段实际值，不能因名称推断源例外或直接改变全局物理时钟。
  - 05d4b第二轮估时：按原35分钟窗口1.5倍，开始2026-09-10 22:06:06、预估22:58:36。分成两个独立闭环：22:06:06→22:31:06核清当前root-motion前提及组件/足锁/动画时间的跨阶段最小反例；22:31:06→22:58:36仅修首个已证实源输入边界、完成匹配回归/同入口/静态并正确顺序复验。第四轮父项原22:39:49终点不变，若到其22:59:49容差仍未完成，先执行父项复盘再继续；共享四case和后续全移植目标保持。

  - 05d4b时序前提确认2026-09-10 22:10:00：公开只读Transition提取72.397s PASS，当前A_Als_Stand_Transition_Right为mesh-space additive、bEnableRootMotion=False；AB_Als为RootMotionFromMontagesOnly，不能走CMC提前TickCharacterPose的root-motion例外。本地Character→Mesh和ALS Actor→Mesh依赖明确要求普通动画在移动完成之后；目标图输入仍在Avian积分前，而Rig在积分后。旧足输入fixture只在两帧之间手改Actor变换，没有在物理阶段移动Actor，漏掉了准备与执行之间组件变换改变的边界。先扩展这一现有fixture，用物理阶段的0.1米运动检查图前锁脚准备与组件空间值，不修改生产调度或启动新runtime。

  - 05d4b时序反例/公开接口方案2026-09-10 22:28:00：在既有足输入fixture的Physics阶段加入0.1米移动后34.454s稳定FAIL于图前目标坐标；此前夹在两帧外的手动位移不能覆盖这一边界。已确认Bevy公开Schedule.remove_systems_in_set可替换默认求值system，依赖公开run_animation_player保留原图求值/错误/时钟处理，set_animation切换和暂停方案均不采用。先在als-ext建立按player标记的物理前/后求值与实际单次计数fixture，未改依赖或全局物理时钟；项目只将角色主图及相关输入移到移动之后，独立montage/root-motion sampler保持物理前，随后扩展已声明共享case覆盖生命周期影响。当前先验证这一公开扩展点，再接入Actor；不据源阶段名称直接改物理tick。

  - 05d4b公开调度接入静态进展2026-09-10 22:42:00：公开接口fixture16.882s PASS（主图物理后值/采样器物理前值、各自frame_index每帧仅+1、root motion当前值）；补齐fixture的真实Children和默认pose输出ABI后获得该结果，没有修改依赖。角色主图由AnimatedSceneInstance新增事件对应的一次标记选择后阶段；既有ActualMotion、Gait、Standing/Crouching、Grounded、Layering/Head、Root、Locomotion及Foot输入按同一父set在Physics之后求值，源通知/当前图观测随后读取，原独立montage/roll/mantle采样和驱动保持前阶段。源0.1米物理移动反例修复后41.312s PASS，骨目标与组件坐标一致，物理时钟原clamp/restore数值未改。当前用完整Playable插件DAG和344/307预期回归检查跨模块顺序，尚未启动runtime；旧Stage名字在测试中的假设同步按源依赖更新。
  - 05d4c排期/覆盖说明：旧预定22:08:34→22:39:49因4b未闭合而未启动，实际开始/预估结束在4b通过后登记；父项22:39:49及22:59:49复盘界线未更改。当前改动仅为源证实的公共输入/主图阶段，Standing/Grounded/Crouching/Locomotion的源图、状态机和动作算法不扩展；除原grounded、locomotion、standing_turn、crouching，还需standing_direction及ragdoll/get_up验证被该公共阶段直接影响的现有消费者。

  - 05d4b阶段接入回归2026-09-10 22:49:00：完整Playable插件DAG、344底层/307项目42.957s PASS；保持依赖原并行方式后focused29.207s PASS，移除player后阶段标记的受控路径只得到旧值，单次frame_index仍保持，证明标记用于阶段选择而非补跑。原源图/参数/骨骼求解与物理delta clamp算法不变，复用公开remove_systems_in_set和run_animation_player，无依赖改动；Default原trace增加实际根图evaluation_frame以验证真实入口每帧一次。当前同入口构建中，随后19图spec/声明式与startup→Default完整动作，阶段调整尚未作为运行时成功结论。

  - 05d4b本轮source/static闭合与运行前2026-09-10 22:54:00：本轮源时序前提和跨阶段最小反例已于22:12:09取得；阶段接入的完整Playable DAG、344/307回归42.957s、并行/单次求值及移除标记消融29.207s PASS。同入口114.400s、19图fresh spec3.156s、十命令声明式4.500s PASS。采用公开schedule替换和原run_animation_player，保留原并行更新、错误处理与montage/root-motion采样前阶段；Actor主图及其输入改为物理后，原80/79通道和所有数值守卫仍保持。当前源码含两条仅test使用的import编译warning，不影响该构建结果。本轮整个4b原预估22:58:36及父项22:59:49复盘界线不变；先独立startup，再原完整Default，尚未将输入时序修复认定为故障已消除。

  - 05d4b物理后求值startup通过/完整动作前2026-09-10 22:56:20：cf202f8 startup15.219s PASS，frame p95 14.1943ms/max27.288ms，图/Root/pre-rig/final/时间守卫均通过，0截图。344/307、实际物理移动fixture、并行单次求值消融、19图spec和十命令静态均通过；现在原完整Default六窗口、两次跳跃及Stop重叠，验证实际evaluation_frame与公共数值边界。

  - 05d第四轮父项复盘/下一轮登记2026-09-10 23:02:34：第四轮开始20:58:34、预估22:39:49，到22:59:49容差未完成；实现及新runtime已暂停。cf202f8完整Default18.091s的运行时数值判定为PASS，原右小腿突变消失，fault/temporal/graph为空、frame p95 20.6375ms/max26.7664ms；六窗口/14coverage/source公式全部通过。公共validator只拒绝default_air观测0.0510219秒低于原0.06秒。原因已定位为场景在covered[3]达标后停止记录仍然满足源条件的Air窗口；真实trace此后仍有约0.5秒合法Air数据，当前不是角色/动画缺陷。首尾interval与累计delta的不同使首次帧时偏大时五个采样提前完成，旧测试只验窗口声明，未验完成后继续采集。此次余下先删除这个多余采集截止条件，再复用原条件完成公共窗口及七项共享动作回归；不再修改已验证的姿态算法、原动作/Stop重叠、帧时或时间阈值。
  - 第五轮依据剩余边界重分：按上一101m15s窗口1.5倍推导，本轮开始2026-09-10 23:02:34、预估2026-09-11 01:34:26，实际完成即收口，不消耗剩余窗口。拆为三个均不超过60分钟的独立Gate：当前Air采集连续性静态/完整Default；原grounded、locomotion、standing_turn、crouching、standing_direction；阶段直接影响的ragdoll/get_up及相关有限复开收口。各子项实际开始时按约25秒focused、约70秒同入口和既有case记录登记独立起止；总目标仍为全部移植，Default之后继续Layering→Head→CR→AB_Als，其余12 overlays按用户延期。

  - [x] BF115.05d5a Air窗口连续采集与完整Default收口：开始2026-09-10 23:04:23、预估2026-09-10 23:29:23。依赖已通过的源时序/同入口static；保留原0.06秒要求，先用覆盖标记已为true但Air源条件仍有效的最小反例，再删除提前停止采集条件，做适用测试、同入口/spec/声明式和startup→Default。估时依据为已定位的一处场景条件、约30秒focused与约70秒增量入口，原Default约18秒；不改actor或动画算法。

  - 05d5a采集反例闭合2026-09-10 23:12:00：既有窗口fixture增加covered=true但Air源条件仍有效的路径，62.905s稳定FAIL于None/Some(3)；删除提前截止条件后5项Default focused23.146s PASS，覆盖前后均连续采集Air/Prediction，未知预测值和非跳跃case仍不采集。保留原动作、完成时刻、0.06秒窗口要求和全部Actor算法；新增方法仅提取原选择表达式供同一个fixture调用。两个仅test使用的导入移到cfg(test)，同入口增量构建中，随后fresh spec/声明式与逐项检查的startup→Default。

  - 05d5a真实入口前2026-09-10 23:13:00：5项Default23.146s、同入口62.688s、19图fresh spec2.863s及十命令声明式4.235s全部PASS；源闭包和Actor物理后求值保持已验证实现。以下先独立startup，通过后才启动完整Default，保留六窗口/两次跳跃和所有公共数值守卫，CaptureCount=0。

  - 05d5a startup通过/完整动作前2026-09-10 23:14:00：新入口startup15.224s PASS，公共validator issues为空、0截图；5项focused、19图spec和十命令静态均已闭合。已单独检查startup终态，下面运行原完整Default以验Air持续观测、源六窗口和原Stop重叠，不改变阈值或场景动作。

  - 05d5a及4b完成2026-09-10 23:14:05：公共Default18.109s PASS，manual/validation均0、issues为空，六窗口/全部14coverage及source逐帧公式通过。Air由5帧/0.051秒恢复为70帧/1.51596秒，原0.06秒要求不变；frame p95 19.8318ms/max27.7456ms、eval p95 0.037ms，fault/temporal/graph为空、0截图，原final calf_r突变未重现。两次跳跃、原过渡重叠与停止后的采样时钟均保留。5a开始23:04:23、预估23:29:23、实际23:14:05（偏差-15m18s），修复及Gate完成；4b开始21:10:13、首预估21:45:13、第二轮预估22:58:36，实际23:14:05（对第二轮+15m29s），原完整Default边界关闭。整个Default及足输入/mesh-space/停止时钟的有限复开仍待下一项共享动作回归，不提前勾选父项。

  - [x] BF115.05d5b 共享移动与姿态阶段回归：实际开始2026-09-10 23:15:14、预估23:50:14。依赖5a；同一入口依次执行原grounded九窗口、locomotion十四窗口、standing_turn、crouching及standing_direction，确认主图/输入物理后求值与采样器物理前求值的公共变更不破坏现有流程。估时依据为当前完整344/307和单次求值fixture、19图spec/十命令静态已通过，各case历史约30–45秒；保留首次实际失败的source诊断时间。4c原预定未启动，实际从本5b开始、依赖最后ragdoll/get_up子项收口；第五轮父项01:34:26不变，不展开新图算法。
  - 05d5b Grounded运行前2026-09-10 23:15:14：Default完整Gate已通过并提交1ece4c1，同一新入口startup已独立PASS，源闭包及静态无新增差异。以下原Grounded九窗口，保留Roll、站蹲/瞄准切换、实际源状态/姿态链及公共全部守卫，CaptureCount=0。

  - 05d5b Grounded结果与观测开销定位2026-09-10 23:25:00：42.977s运行完成、九窗口及全部coverage通过，但公共Guard拒绝下蹲持续瞄准段final foot_r/foot_l旋转82.8276/51.9732rad/s及foot_l速度25.2122m/s。相邻实际Pose记录在三次异常前均有115.817/132.466/143.581ms主线程停顿，约一秒一次；已用case内elapsed而非全局elapsed对齐trace，足锁曲线为0、实际Rig目标与结果相符。回读fresh UE RefreshFeet/无锁分支与当前consumer，未证实动画/IK差异，不作修改。目标公共collector每秒克隆并pretty序列化全部累积Pose到report.json，当前文件42,781,395字节，写盘后只排除性能样本，真实动画delta和后续目标仍受停顿影响。此为本轮扩充实际输入记录后暴露的观测副作用；先用同一报告写入路径的最小生命周期fixture拒绝运行中周期写盘，保留退出终态、首帧图错误和watchdog报告，再消融删除周期状态及重复flush。现有数字/姿态数据继续逐帧收集，阈值和Actor代码不变；5b原23:50:14预估保持，暂不启动其他case。

  - 05d5b报告生命周期静态闭合2026-09-10 23:28:00：原周期写入提到同一flush路径后，真实ECS/文件fixture37.066s稳定FAIL（未退出仍重写报告）；删除周期deadline、ready/terminal重复写入和两个单次flush辅助函数，统一在Last消费AppExit，306项目26.319s PASS。一个fixture覆盖运行中不写、正常/错误退出写入最终样本、同次退出不重复写；所有原帧采集、terminal样本、图首错立即报告和watchdog保留，未修改Actor/动画或任何阈值。validator self-test0.569s PASS；省去两个旧布尔fixture，用真实文件生命周期检查替代。同入口构建随后19图spec/声明式，再startup→Default→原Grounded重新验证是否消除报告导致的真实停顿。

  - 05d5b报告修复运行前2026-09-10 23:30:00：306项目26.319s、公共validator自测0.569s、同入口63.532s、19图fresh spec2.756s及十命令声明式4.504s PASS。当前所有Actor/图/物理代码仍为已通过Default的1ece4c1，只消除采集器的运行中全量序列化。以下独立startup验证退出终态报告，再完整Default及原Grounded；完整数值数据和原守卫保留，CaptureCount=0。

  - 05d5b报告修复startup通过/Default前2026-09-10 23:30:03：15.183s公共startup PASS，退出后report.json完整存在、issues/图/时间异常为空，0截图。306项目、validator和全部声明式已通过；下面同一入口完整Default，核对六窗口及此前Air连续采集在终态报告中全部保留。

  - 05d5b报告修复Default通过/Grounded前2026-09-10 23:30:54：18.039s完整Default PASS，终态报告保留六窗口/全部coverage、issues为空、0截图；新startup性能采样616帧且没有写报告导致的排除帧。当前静态和306项目均闭合，现在恢复原Grounded九窗口，核对此前三个周期停顿后的脚部异常，输入/源图/阈值不变。

  - 05d5b Grounded通过/Locomotion前2026-09-10 23:32:05：报告改为退出写入后，原Grounded41.973s PASS、九窗口/15coverage全部通过，fault/temporal/graph为空、0截图。2234性能帧且excluded=0，frame p95 21.734ms/max41.5137ms；原下蹲持续瞄准失败段max仅23.2164ms，约一秒一次的116–144ms停顿与三处脚异常同时消失。Actor、图、动作和阈值均未改，确认本次原因为collector周期性全量写入；该漏检边界固化到领域skill和真实退出fixture。现在同一入口执行原Locomotion十四窗口，其他共享动作仍待完成。

  - 05d5b Locomotion结果与续跑复盘2026-09-11 11:26:44：上次29.908s运行的公共姿态/时间/性能均通过、fault/graph/temporal为空；manual仅land_mixed缺失，13窗口有效。真实低落差末段依次为Air/-6.160388m/s、接触地面但仍Air/-0.000418m/s、下一帧Grounded/Land Light，混合Alpha=0。公共主图移到物理后暴露了模式发布和速度采样来自不同物理阶段；其余动画选择、同步、落地结束均通过。停止新runtime，回读当前条目、CF202F8阶段变更、fresh UE OnMovementModeChanged/RefreshInAir和目标Tnua发布链，先用真实接触帧fixture证明首个源差异，不改变低落差或land_mixed守卫。
  - 第五轮/5b超时复盘与剩余重分：第五轮开始2026-09-10 23:02:34、预估2026-09-11 01:34:26，本次11:26:44恢复时未完成；5b开始23:15:14、预估23:50:14，实际仍未完成。上次23:33后的运行已结束，此次恢复前没有续启场景；不能把跨夜间隔记成持续实现或通过。已完成Default、周期报告副作用修复及Grounded；停顿来源先由同间隔时序/43MB序列化定位，再以不改Actor的原场景消除，避免误修足部算法。剩余问题收敛到接触帧的模式/速度一致性；之后仍须Locomotion、standing_turn、crouching、standing_direction、ragdoll/get_up，才能关闭Default及三个有限复开。第五轮151m52s按1.5倍登记第六轮开始11:26:44、预估15:14:32；5b原35分钟窗口按1.5倍登记第二轮11:26:44→12:19:14。按接触帧source/static、完整Locomotion与既有移动动作、ragdoll/get_up分为独立不超过60分钟的剩余闭环，实际完成即继续后续Layering→Head→CR→AB_Als，不消耗空余窗口。

  - 05d5b接触帧source闭合/04d有限复开2026-09-11 11:31:27：fresh UE ProcessLanded→SetPostLandedPhysics在碰撞当次SetMovementMode，ALS OnMovementModeChanged立即映射Grounded；RefreshInAir独立保存空中竖直速度，供Land使用。目标locomotion_state_sync_system在Update的Tnua Logic后/Physics前发布，主图改到Physics后后会在真实接触帧读到Air与碰撞归零速度的组合；失败trace已直接证明此边界。04d仅该模式发布边界取消勾选，原转换图、数据、Jump事件和其余已通过算法不复开。旧真实Tnua fixture只将ALS状态对比同一个controller缓存，无法拒绝两者同步滞后一帧；先扩展其真实平地碰撞帧断言，再使用现成Avian接触数据完成当帧发布，不重跑Tnua逻辑、不增加地面射线、不保存假落地速度。5b第二轮12:19:14及父项15:14:32保持。

  - 05d5b接触帧修复静态进展2026-09-11 11:46:00：首次两个fixture因原3米起点被Tnua提前托住，没有触发几何接触；按原场景6.2m/s对应落差及13ms步长后24.034s稳定FAIL于第49帧：高度0.9米、速度归零但Air。当前模式发布移至Physics后，复用Avian实际有冲量且靠近中的可行走接触，31.690s PASS；关闭接触分支22.723s FAIL、单独移回旧发布阶段20.845s FAIL，均已恢复。扩展同一fixture后243 core/306项目79.962s PASS（原1 ignored）：保留交替步长、事件唯一性/不回弹/ragdoll请求，双端碰撞顺序等价，墙/顶面、sensor、无冲量或分离接触均不能发布Grounded。主图输入显式在该模式/速度发布之后，未增加射线、缓存速度、状态或参数；fresh UE ProcessLanded/ALS模式CPP与已有source记录逐字一致。当前同入口构建中，随后19图spec、Default/Locomotion声明式，再完整真实回归。

  - 05d5b接触帧修复真实入口前2026-09-11 11:49:00：243 core/306项目79.962s、同入口143.708s、19图fresh spec3.404s、Default十命令6.061s及Locomotion十二命令4.751s PASS。32源子图/46RON及Default两图的结构、数据、曲线和动作算法未改；模式发布只读取本帧已解算的可行走冲量接触，必要分支和物理后时点的两次消融均失败。以下先独立startup，再完整Locomotion检验缺失的land_mixed，随后Default、Grounded及原Standing/Crouching/Direction共享回归，仍为CaptureCount=0和全部原守卫。

  - 05d5b当前模式startup通过/Locomotion前2026-09-11 11:49:59：15.387s公共startup PASS，issues为空，退出报告完整、0截图。243/306回归、19图spec与两套声明式均已闭合；现在原Locomotion七段十四窗口，首先核对接触帧Grounded与保存的空中速度、Light/Heavy实际Alpha及完整自然结束，保留原落差和canonical条件。

  - 05d5b完整Locomotion通过/Default前2026-09-11 11:51:06：29.892s公共Locomotion PASS，14窗口/全部coverage及自然结束通过，fault/graph/temporal为空、0截图。Air最后速度-6.209811m/s在进入Grounded/Land后保留，Heavy实际Alpha0.241962、Light0.758038；land_mixed71帧/1.16672秒、canonical时间差0.000926秒。frame p95 19.7942ms/max28.47ms、excluded=0。243/306及19图/双声明式保持通过；下面同一入口Default六窗口，再其余共享动作，04d有限复开待这些消费者验收后关闭。

  - 05d5b Default新边界与4b有限复开2026-09-11 11:58:00：18.108s Default六窗口及所有自身公式通过，但首次起跳final calf_r为0.762831rad/0.0163814s=46.56688rad/s，公共Gate失败。相邻求解输入显示角色已上升0.149米仍Grounded，至0.2148米才首次Air；此前较高帧率通过记录也存在延迟，首Air为0.1466米，旧Gate缺少“实际Jump开始当帧的模式”断言。fresh UE DoJump在成功设置起跳速度时立即SetMovementMode(Falling)；本地Tnua公开is_airborne明确只查询basis，ongoing()独立给出已执行且包含首帧的Action。当前参数coyote_time=0，不能把这一延迟归因于土狼时间。04d保持有限复开，4b仅此起跳模式边界取消勾选；先扩展同一真实Tnua fixture，要求实际Jump开始即Air，再合并已有执行状态，不用请求按键或竖直速度猜测，不改Stop重叠、曲线或阈值。

  - 05d5b起跳首帧修复静态闭合2026-09-11 12:01:40：既有真实Tnua fixture新增实际Jump执行与Air同帧断言，25.302s稳定FAIL于首帧Grounded。模式发布合并公开ongoing()==Jump与原basis结果，再沿原接触分支处理落地；243 core/306项目44.825s PASS（原1 ignored）。同一fixture保留13ms及交替步长，跳跃从启动到自然落地均覆盖且不能重新变Air，原碰撞边界/事件/传感器排除和ragdoll请求仍通过；旧未合并执行状态的路径即失败消融。无新状态或配置，不以按键请求/速度代替实际Action。当前同入口构建中，随后双声明式与startup→Default→Locomotion，再其余已登记共享Gate；5b第二轮12:19:14终点不变。

  - 05d5b起跳修复真实入口前2026-09-11 12:05:03：243 core/306项目44.825s、同入口129.413s、19图fresh spec3.148s、Default十命令4.230s与Locomotion十二命令4.216s全部PASS。当前仅补实际Jump执行期的Air发布，接触落地分支及其消融保留；原图、资产、曲线、Stop时钟和Rig算法未改。下面startup通过后完整Default，再Locomotion和其余共享case，原5b第二轮12:19:14预估保持，CaptureCount=0。

  - 05d5b起跳修复startup通过/Default前2026-09-11 12:05:52：15.270s公共startup PASS，issues为空、终态报告完整、0截图；243/306与19图/双声明式均已通过。下面原Default六窗口和两次跳跃，核对实际起跳首帧的Air与小腿公共时间边界，不改场景重叠或阈值。

  - 05d5b Default通过/4b起跳边界关闭2026-09-11 12:06:59：18.080s完整Default PASS，六窗口/全部14coverage及逐帧源公式通过，fault/temporal/graph为空、0截图。首次Air对应mesh高度0.06077米（起跳前0.00401），不再等到0.2148米；实际Transition仍处原满权重重叠，下一帧按源淡出，未改场景等待或参数。frame p95 21.9738ms/max27.7304ms、excluded=0，右小腿异常消除。4b该有限复开开始11:58、沿5b预估12:19:14、实际12:06:59完成（偏差-12m15s）；5b其余共享Gate及04d尚未关闭。下面当前版本完整Locomotion十四窗口，再Grounded/Turn/Crouching/Direction。

  - 05d5b短按跳跃模式修复2026-09-11 12:14:01：前次Locomotion28.178s在第四段watchdog退出，实际首跳序列均为Air→Grounded→Air，短按释放后的Jump action结束时错误进入Land，图/数值本身无错误。既有fixture只覆盖按住4帧，补短按后21.598s稳定FAIL于起跳第二帧Grounded。Tnua Walk源码证明airborne_timer创建为0秒时要下一次tick才is_finished；公开displacement在无支撑时立即为None，且在本次恢复支撑后立即为Some，因此改读此完成的basis状态，并保留实际Jump执行期和已验证的碰撞落地分支。无新增状态、timer或阈值；原is_airborne读取路径即失败消融。243 core/306项目53.093s PASS（原1 ignored），13ms及交替步长下均覆盖短按/持续按住、完整起落、事件和接触过滤。当前同入口构建中，随后fresh spec/双声明式及完整真实回归；原5b预估12:19:14及12:39:14复盘界线保持。

  - 05d5b完整模式修复真实入口前2026-09-11 12:18:20：243 core/306项目53.093s、同入口136.896s、19图fresh spec2.873s、Default十命令4.493s与Locomotion十二命令4.235s PASS。当前使用已完成basis的支撑状态、实际Jump action及本帧已求解接触统一发布模式；短按/持续按住和完整落地均由同一真实fixture覆盖，原误用延时标志路径稳定失败。以下先独立startup，再完整Default/Locomotion及所有剩余共享case，CaptureCount=0；5b原12:19:14预估未顺延，到12:39:14仍未完成则先复盘再继续。

  - 05d5b完整模式startup通过/Default前2026-09-11 12:19:01：15.211s公共startup PASS，issues为空、完整终态报告、0截图；243/306及19图/双声明式已通过。现在原Default六窗口和两次跳跃，随后立即完整Locomotion及其余共享动作；12:19:14预估及12:39:14复盘界线保留。

  - 05d5b完整模式Default通过/Locomotion前2026-09-11 12:21:00：18.099s完整Default PASS，issues/fault/temporal/graph为空、0截图，frame p95 22.8184ms/max68.4936ms、excluded=0。243/306及19图/双声明式保持通过；当前短按/持续按住的模式发布修复已通过原Default，现在完整Locomotion十四窗口，随后Grounded/Turn/Crouching/Direction，保留12:39:14复盘界线。

  - 05d5b完整Locomotion通过/Grounded前2026-09-11 12:22:20：32.162s公共Locomotion PASS，十四窗口及全部18项coverage通过、自然结束且未超时，fault/temporal/graph为空、0截图。frame p95 20.0955ms/max29.2585ms、excluded=0；原短按Air→Grounded→Air与land_mixed缺失边界均通过当前实际场景。下面同一入口原Grounded九窗口，再Turn/Crouching/Direction；静态闭包及12:39:14复盘界线保持。

  - 05d5b Grounded失败与当前直接读数边界2026-09-11 12:34:00：42.260s运行完成，manual缺滚翻退出/完成两项，另在持续瞄准段pre-rig foot_l为0.783311rad/0.0173537s=45.137985rad/s；frame p95 22.7539ms/max40.5314ms、excluded=0、无图错误、0截图。全部实际模式均Grounded，不能把此失败归因于已修起落模式。fresh公开Grounded native77.013s及本地UE SetState证明FromRoll入口与Roll退出函数合同；前次通过时重新相关帧产生Roll退出请求，本次同帧只产生两个Stop回调，FromRoll均在此前约0.13秒已发布。现有trace只有汇总计数，不能区分实际FSM选择与通知读取；临时状态候选尚未证明在该路径参与，未修改其reader。复用现有实际节点遍历，补正式FSM状态/回调/求值帧和当前入口条件读数，不新增Actor状态、替代过渡或阈值；静态闭合后仅恢复同一Grounded作直接边界验收，左脚突变仍独立保留。

  - 05d5b实际FSM读数静态完成/运行前2026-09-11 12:36:50：306项目99.419s、同入口64.501s、19图fresh spec2.599s及十命令声明式4.231s PASS；fresh Grounded入口/退出源与UE SetState纳入当前合同。新增读数直接来自原已更新节点的正式状态、原入口输出和回调列表，未修生产算法或改变场景；先startup后同一Grounded，检查汇总计数漏掉的真实选择边界。12:39:14复盘界线仍保持。

  - 05d5b实际FSM读数startup通过/Grounded前2026-09-11 12:37:15：15.181s公共startup PASS，issues为空、完整终态报告、0截图；306项目、19图和十命令静态通过。现在恢复原Grounded九窗口，只增加正式FSM/入口条件读数；原滚翻缺失和左脚异常仍待闭合。

  - 05d5b第二轮超时复盘2026-09-11 12:39:17：第二轮开始11:26:44、预估12:19:14，实际72m33s、偏差+20m03s，结果未完成；暂停实现和新runtime并回读条目、当前source/静态及两次Grounded失败。真实接触落地、起跳首帧、短按释放均已由失败fixture和原Default/Locomotion闭合；其后Grounded首次缺Roll退出/完成并有一帧左脚突变。新增正式读数的41.896s运行仍缺相同两项，但公共数值/性能通过（p95 22.0349ms/max44.5373ms）、0截图；第193求值帧入口FromRoll读取true、Grounded最终Standing而回调列表为空，所有参与FSM upcoming_states均1，排除临时状态任取假说，未改通知reader。此前约0.14秒实际FromRoll持续存在，不能再按Actor模式、输入缺失或报告停顿解释。漂移是原汇总读数无法定位回调丢失层，现已缩到同帧入口→状态转换→输出保存；左脚异常未重现但仍未关闭，须保留原完整Guard。
  - 05d5b第三轮剩余拆分：按上一52m30s窗口1.5倍，开始2026-09-11 12:39:17、预估13:58:02，父项第六轮15:14:32保持。分为可独立验收的源状态转换静态闭环与原共享运行闭环，不新增相邻模块：5b1从12:39:17→13:24:17，复用现有实际FSM fixture覆盖FromRoll已命中、重新相关当帧转换及退出回调，以fresh UE SetState为预期，先失败后最小修复/消融；5b2预定13:24:17→13:58:02，依赖5b1的同入口/static后，startup、完整Default/Locomotion/Grounded及Turn/Crouching/Direction，保留全部原守卫。估时依据为现有真实图fixture、当前约25秒底层/项目回归、约135秒共享crate入口及已测单case约15–45秒；任一段提前完成立即继续。当前只做5b1，不以本次无左脚异常替代根因证明。
  - [x] BF115.05d5b1 当前Roll采样时钟静态闭环：开始2026-09-11 12:39:17、预估13:24:17、实际静态完成12:55:50（偏差-28m27s）；范围修正为下述fresh source证实的采样→位移→速度时钟。
  - [x] BF115.05d5b2 完整共享移动恢复：实际开始2026-09-11 12:57:00、预估13:30:45，按原33m45s终段窗口提前启动；依赖5b1，原场景和全部Guard不变。

  - 05d5b1完整入口条件与时钟根因2026-09-11 12:48:00：纠正上一摘要的入口推断：T2不仅检查FromRoll，还要求非MovingSmooth且无Rotate；通过记录速度1.2295m/s，而两次失败为1.5216/1.5757m/s，均仍MovingSmooth，源规则正确拒绝Roll入口，不能修FSM或通知reader。fresh AM_Als_Roll32.837s及本地Montage Advance/CMC证明推进到当前Position后提取位移，并除以同一Delta。目标prepare_roll_slot_samples先读旧roll.time、roll_drive随后推进时间再用当前Delta除上一帧采样位移；13/25/11/33ms下真实插件调度的线性轨道fixture44.256s稳定FAIL。BF102.04.a仅该时钟边界取消勾选；旧淡出fixture预设位移而不经过采样时钟，漏检了帧长变化。下面把已有生命周期推进放到采样前，姿态/曲线/通知/位移消费同一时间，保留跨入淡出的最后时间片及随后普通制动；不改RootMotion曲线、源入口阈值或原场景。

  - 05d5b1时钟修复与消融闭合2026-09-11 12:53:17：现有生命周期推进移至采样前，采样、权重、曲线和通知共用当前roll.time；根运动驱动继续保留跨入自然淡出的最后时间片，此后普通制动接管。307项目33.811s PASS；同一插件调度fixture追加首次输入启动，单独移除首帧推进23.676s FAIL，恢复后完整307项目21.131s PASS，覆盖13/25/11/33ms线性速度不变、已有淡出/自动落地输入阻塞/ragdoll停止边界。无新持久状态、时间倍率、位移修补或阈值；将生产者与采样消费者的原循环拆成两个必要阶段，未改核心Roll规则或生成图。实际入口条件已澄清，不改FSM/通知reader；新增完整FSM诊断字段全部删除，原速度/MovingSmooth/回调数据足以保留此边界。当前同入口构建随后fresh spec/静态，再提前启动5b2原完整场景，5b1原13:24:17终点保持。

  - 05d5b1完成/5b2运行前2026-09-11 12:57:00：307项目21.131s、最终同入口63.640s、19图fresh spec2.848s、Default十命令4.232s与Locomotion十二命令4.190s全部PASS。原后推进路径与移除首次启动推进均由同一实际插件调度fixture拒绝；无额外诊断状态，源Roll曲线、入口条件和阈值未改。本项只关闭静态时钟边界，BF102.04.a仍待真实验证；下面独立startup，通过后依次完整Default/Locomotion/Grounded及Turn/Crouching/Direction。

  - 05d5b2 startup通过/Default前2026-09-11 12:57:30：011740a时钟修复及当前模式/报告工作树的公共startup15.217s PASS，issues为空、终态报告完整、0截图。307项目、19图及双声明式闭合；现在原完整Default六窗口，再Locomotion/Grounded和其余共享动作，保留所有原Guard。

  - 05d5b2 Default通过/Locomotion前2026-09-11 12:58:20：18.013s完整Default PASS，公共validator issues为空、0截图；当前滚翻时钟修复未破坏原六窗口和两次跳跃。源/static及307项目保持，下面同一入口完整Locomotion十四窗口，再Grounded/Turn/Crouching/Direction。

  - 05d5b2 Locomotion通过/Grounded前2026-09-11 12:59:10：32.357s完整Locomotion PASS，十四窗口和原全部coverage/自然结束通过，issues为空、0截图；Default当前六窗口/14coverage亦PASS，p95 19.7678ms/max25.5799ms、无fault/temporal/graph。接着原Grounded九窗口，重点核对当前采样时间片产生的退出速度及源完整入口条件，保留原左脚公共Guard；不改变输入时刻或地形。

  - 05d5b2最新Grounded结果2026-09-11 13:02:00：41.838s运行完成，frame p95 21.8059ms/max38.0993ms、0截图/图错误；仍缺Roll退出/完成，持续瞄准停步帧pre-rig calf_r/thigh_r为51.9704/54.8658rad/s，公共Gate拒绝。Roll当前时间片已由focused证明一致，但恢复Source时速度1.511884m/s，MovingSmooth=true，源完整T2正确选择普通Standing；随后下一帧1.3262m/s才进入Stop，不能修FSM或阈值以制造Roll入口。当前时钟修复是独立真实差异，尚不足以完成原场景；5b2保持未完成。停步异常与速度跨1.5m/s、Stop播放器初始权重0同帧，先只读已有Standing→Stop直接惯性化链，按源/target首个差异处理；原连续动作、left/right守卫与源Roll分支要求均保留，13:30:45终点不变。

  - 05d5b2当前Standing差异/01c与01p有限复开2026-09-11 13:11:20：fresh Standing公开37.505s证明Stop States.bSkipFirstUpdateTransition=False，Node导出文本省略该false值但结构字段保留；当前FSM合同仅解析文本并猜默认true，0.300s最小结构字段fixture稳定FAIL。左右Plant六向共12个源ExplicitFrame分别为右21/21/24/24/23/21、左4/4/6/4/7/7，源可见pin与Node一致，当前安装资产却全0；帧解析器在Default任务已修，但Standing未重生成。实际失败为Move→Stop当帧进入Plant Right，目标跳过源0.1秒混合并硬切到错误第0帧，链路直接到pre-rig腿部突变。01c/01p仅这些生成字段与Plant姿态边界取消勾选；旧Gate核对回调/Stop montage，未对Plant固定采样帧独立断言，旧源审计仅检查文本出现的FSM值。复用既有结构字段解析修FSM false，并让源审计拒绝错误采样帧与结构字段，公开重生成Standing完整owner，保留其余已完成模块。
  - [x] BF115.05d5b3 全部已生成AnimBP共享规则与生成静态闭环：开始2026-09-11 13:11:20、预估13:31:20；作为5b2当前失败的静态前置，限定上述false字段、12个采样帧、相关生成/审计与实际图反例。估时依据为已有0.3秒工具自测、当前Standing+六BlendSpace七资产公开闭包、历史约200秒读取/生成及当前约64秒入口构建；通过后恢复5b2原全部共享动作并补原standing_stop/standing_response直接消费者。5b2原13:30:45、第三轮13:58:02与父项15:14:32保持；未以新子项延后复盘界线。

  - 05d5b3共享规则影响范围/用户纠正2026-09-11 13:26:00：用户明确要求同类节点只用一套通用转换规则，并核对对所有旧生成图的影响。分派与注册表核查确认按class/function signature，禁止asset/name/GUID专用规则；旧ExplicitFrame规则虽已修，Standing仍混用旧产物，当前模块隔离执行遗漏了共同规则迁移。全installed generated扫描得到7个owner、16张图、55个相关节点（FSM、SequenceEvaluator及当前反射字段涉及的三种blend）；已用fresh source独立拒绝Standing的12个固定帧及Stop/Movement Details两个false字段，共14处，不将55个节点全部当成错误。当前271.340s公开Standing转换完成七源读取但因三类节点新增反射字段未完整建约束而统一阻断，未安装部分产物；绑定/renderer错误均为此前合同拒绝的后续结果，已有HipsDirection provider无需另建。范围由单Standing修正为共享规则与所有既有消费者的统一核对/迁移，已固化到AGENTS及领域skill；后续仅按fresh字段证明重生成受影响owner，不重做无关模块功能或其它12个overlay。

  - 用户扩大并固定全量转换Gate2026-09-11 13:35:00：用户进一步要求检查全部已生成AnimBP，包含所有历史批次遗漏，并规定以后每次修复转换器都执行全量AnimBP转换确认影响范围。当前已安装完整基线为AB_Als、Standing、Crouching、Grounded、Locomotion、Layering、Head、Ragdolling、Default九个AnimBP及256个RON；所有节点/参数/绑定/FSM/生命周期产物都纳入重生成比较，不以先前55个节点名单筛选。当前公共AB_Als根入口正在提取完整依赖闭包，复用本次已取得的七份原生记录并读取其余源资产；未安装任何不完整产物。AGENTS与领域skill已将每次修复后的全量转换、完整diff及适用回归设为必需Gate。
  - 05d5b3全量source/static进展及重估2026-09-11 13:50:00：完整九AnimBP/六BlendSpace及相关第十六份源记录均已取得，保持原记录不可变。省略默认字段使BlendList、LayeredBoneBlend和ControlRig旧合同拒绝；四类真实字段反例0.293s FAIL，公共解析修复0.305s及五工具自测PASS，图名/节点ID替换保持语义、字段false→true改变对应产物。UE通用LayeredBoneBlend默认先更新overlay再base，当前反序；Enum Blend源为定时线性权重且只更新相关分支，当前指数逼近且始终更新两支，实际图/权重fixture54.839s稳定两项FAIL。复用已有BlendList权重与图缓存修复中，不新增图专用算法。全量第二次转换24.521s拒绝最后Ragdolling SnapshotName默认字段，零RON安装；下一步补该合同后完整重生成、256文件diff及所有owner审计，再同入口运行。原5b3开始13:11:20、预估13:31:20，本轮复盘13:50（偏差+18m40s），结果未完成；范围由单Standing七记录扩大为全量十六记录与共享运行时语义，剩余拆成静态闭环和原5b2 runtime两段。5b3第二轮开始13:50、预估14:20，按原窗口1.5倍推导；5b2本轮开始12:57、预估13:30:45、实际复盘13:50（偏差+19m15s），因该静态前置未通过尚未继续runtime，第二轮开始13:50、预估14:40:38，按原窗口1.5倍并包含前置收口；父项原14:18:02复盘界线保留。

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

  - 05d5b3全量产物闭环/03b有限复开2026-09-11 13:58:00：公开根入口29.208s成功，九AnimBP/256RON完整生成，安装owner清单不变；全量比较25文件变化（Standing15、Crouching7、Layering/Head/Ragdolling各1），其余231文件完全一致。新旧节点ID、条件表达式和六FSM全部出口条目相同，只有原生执行顺序迁移；另有Standing两false、12 Plant帧、16起播点（八0.25/四0.1/四0.15）和六Enum时间旧值遗漏，Crouching一处source PlayRate默认0、左右Rotate RefPose root lock未迁入；十Layered节点记录当前源更新顺序，剩余为已有runtime默认字段显式化。03b仅这七份旧生成数据取消勾选，旧Gate在后续通用修复后没有全量重生成，不能证明当前源等价。Layering/Head/Ragdolling仅本轮全部生成数据迁移，不提前宣告后续Actor任务完成。四类字段/身份变体、PoseSnapshot字段反例与共享实际图五帧初始化/瞬时旧分支/零权重路径均已验证；345 ext回归27.823s PASS，旧指数/更新次序54.839s、移除瞬时旧分支26.963s均FAIL。下一步安装完整产物、fresh全图spec与全部owner声明式，然后同入口无截图Gate。

  - 05d5b3公共BlendList/Boolean遗漏2026-09-11 14:12:00：全量155源图/57 Pose图/491源节点/393 Pose边、九FSM/77条件图及256产物审计2.895s PASS，247实际图spec1.877s及六manifest全部PASS；第一次直接启动spec工具缺少资产根环境变量，2.896s按路径错误退出，补显式资产根后通过。沿fresh BlendListBase及ALS派生声明确认GameplayTagBlend也使用加权归一化，当前仍旧slerp；复用原实际图四元数fixture加入Tag后29.360s FAIL，统一到既有blend_source_poses并删除旧插值循环，345回归22.343s PASS。全部条件节点核对另确认十处N元布尔（Standing4/Crouching2/Grounded3/Locomotion1，最后一处四输入）；UE ExpandNode遍历所有附加pin，当前仅读A/B。04b仅原Bool执行合同取消勾选，旧Gate只测类型和二输入而漏掉附加操作数；已建立2/3/4输入全真值表实际图反例，接着只修公共AND/OR fold，重新全量转换确认无数据漂移，再完成同入口与原动作Gate。尚无新runtime启动，所有源记录与生成RON均未手工修改。

  - 05d5b3静态完成/运行前2026-09-11 14:17:36：N元布尔2/3/4输入实际图真值表27.561s复现八组漏算，改为全部Bool pin顺序fold后346 ext回归24.230s PASS，无额外状态或图专用分支；公共Tag改用已有姿态混合函数并移除旧循环，先前反例即消融失败。最终再次公开全量转换29.266s通过，256产物零新增漂移；最终同入口构建118.896s、247图fresh spec2.879s及六manifest（含155源子图/256产物完整审计）全部PASS，编辑器256文件零修复、位置/资产边界通过，项目307回归72.343s已通过。5b3第二轮开始13:50、预估14:20、实际14:17:36（偏差-2m24s），结果静态闭环完成；原Standing/03b/04b有限复开仍等真实动作Gate。
  - 05d5b第四轮前全过程复盘2026-09-11 14:17:36：第三轮开始12:39:17、预估13:58:02、复盘14:17:36（偏差+19m34s），尚未完成共享runtime。前序已分别证实并修复采集器周期写盘、接触帧模式提交与Roll当前采样时钟；此次全量差异又证明单图重生成不能覆盖旧批次产物，共享混合和N元布尔也缺少实际节点反例。纠正后的路径是完整原生闭包、同一规则全量转换/47字段旧数据失败对照、实际图数学与生命周期断言，然后一次连续真实入口验收；不再沿症状修改Actor阈值或只复跑旧图。按上一轮78m45s窗口1.5倍登记第四轮开始14:17:36、预估16:15:44；剩余按原5b2共享动作与后续独立ragdoll/get_up收口分段，各段仍不超过60分钟，5b2当前14:40:38与父项15:14:32的复盘界线保留。以下恢复startup→Default→Locomotion→Grounded及原Standing/Crouching直接消费者，CaptureCount=0；实际完成即继续，不等待预估时点。

  - 05d5b2恢复动作前2026-09-11 14:20:23：ba640ee最终同入口公共startup15.373s PASS，root Pose与首帧求值、数值/性能守卫通过，零截图且没有graph error。155源子图/256生成文件、346 ext与完整声明式均已闭合，以下先完整Default再Locomotion/Grounded；保留原案例、输入driver和所有阈值。

  - 05d5b2 Default失败/下一源边界2026-09-11 14:34:00：公共Default28.716s FAIL（manual91、12秒case进展watchdog），Idle/Walk/Crouch/Air四窗口通过，Prediction/Return未完成；frame p95 19.4559ms/max34.0376ms，零截图/graph error。全部29条prediction Alpha错误以本帧实际消费的旧输入复算误差最大2.13e-8：fresh ALS NativeThreadSafeUpdate不刷新GroundPrediction，Jump/Fall图OnUpdate才写入，而源Layered先overlay后base，因此帧末父属性不能代替先执行节点的输入；现有观测器误用帧末值并累计invalid导致超时。另在首跳Air时final calf_r出现0.84662104rad/0.0176738s=47.9026rad/s，pre-rig未报错、原40rad/s守卫保留，记录了连续两帧Basic IK输入与0.471→0.174权重；UE partial-weight仍为slerp，与当前一致，尚未证明该数值异常根因。后续场景暂停，先修实际消费值观测并fresh重读CR_Als相关输入/初始长度/坐标合同，不回退正确的源更新顺序。
  - [x] BF115.05d5b4 源更新顺序下的实际输入观测与首跳IK边界：开始2026-09-11 14:34:00、预估15:09:00。作为5b2当前失败的静态前置；以已闭合155图、29帧严格复算和当前Basic IK相邻输入为依据，复用节点已有运行时状态保留实际Alpha输入，最小实际图反例拒绝帧末覆盖后的错误比较；再以公开CR原生读取和本地UE算法确认首跳数值差异，只有source证明后才改对应最小边界。估时依据为既有约37秒源读取、约25秒focused、约120秒入口构建和现成相邻输入报告，完成静态与消融后再恢复六窗口Default。5b2原14:40:38及其15:00:38复盘界线保留，不能用新子项延后。

  - 05d5b4观测与初始长度闭环进展/5b2重估2026-09-11 15:00:00：实际图在消费Alpha=0.25后由子节点写成帧末0.9，未记录消费值34.262s FAIL，记录节点input_alpha后19.755s PASS，Default观测改读该值且保留独立源公式。fresh CR39.491s证明Basic IK长度pin无连线/注入且为空（单位默认0），UE读取初始Hierarchy位置差乘current/initial scale；target此前读取当帧动画骨距。既有读取器缺少层级初始TRS，复用Engine ControlRig API补只读导出，构建83.316s及fresh读取35.606s PASS，79骨中右上/下参考长度0.425722504/0.401968193m；本次失败帧目标却取0.42379177/0.40196872m。独立初始长度fixture59.629s稳定FAIL；现有离线profile生成器只新增初始方向差/缩放两个数组，其余runtime配置逐字段相同，数学与足部回归进行中。BF109.10既有有限复开增加该长度边界，旧Gate只验证当前链误差而未断言初始长度来源；原始导出未重写。5b2第二轮开始13:50、预估14:40:38、复盘15:00（偏差+19m22s），因完整Default暴露观测口径和真实IK输入差异而未完成；第三轮开始15:00、预估16:15:57，按前窗口1.5倍推导，剩余仍拆为当前5b4静态修复与独立共享动作Gate，各段不超过60分钟。5b4原15:09及父项15:14:32复盘界线保留，不新启runtime探测。

  - 05d5b4静态闭环/真实运行前2026-09-11 15:07:19：347 ext30.241s、307项目3.677s、16足部28.835s与初始层级源审计0.532s全部PASS；最终入口118.221s，247图fresh spec3.141s与六manifest（全量155源图/256产物）全部PASS，再次完整转换30.242s无新增RON漂移。旧动画骨距反例59.629s与未保存消费Alpha反例34.262s均稳定FAIL，恢复参考层级/实际输入后通过；新增两个配置数组由公开源和既有离线工具生成，其余配置相同，未增加状态机分支或修改守卫。以下公共startup再完整Default验证原29条Alpha误判与首跳calf_r异常，当前只确认静态修复，尚未宣告数值异常消除。

  - 05d5b4 startup后/Default前2026-09-11 15:08:30：最终入口公共startup15.350s PASS，root Pose/数值/性能通过、无graph error、零截图。接着运行原完整Default，比较原Alpha检查与首跳IK失败点。

  - 05d5b4完成/Locomotion前2026-09-11 15:09:33：公共Default17.899s PASS，六窗口与14coverage全部通过，fault/temporal/graph均为空、零截图；实际消费Alpha观测不再误判，源初始参考长度修复后首跳calf_r异常未再出现，原40rad/s与性能守卫全部保持。347 ext/307项目、原生层级及133节点Rig审计、247图spec和全部生成source/static证据闭合；本轮开始14:34、预估15:09、实际15:09:33（偏差+33s），结果完成，无新增拆分。下面同一入口继续原Locomotion与Grounded，再执行Standing/Crouching直接消费者；Default父项与此前有限复开仍待这些共享Gate收口。

  - 05d5b2 Locomotion后/Grounded前2026-09-11 15:11:24：795d4d2同入口Locomotion31.542s PASS，14窗口、全部动作覆盖和原姿态/性能守卫通过，零截图。接着原Grounded九窗口，验证先前Roll出口及站蹲转换；没有改输入driver、动作顺序或1.5m/s源条件。

  - 05d5b2 Grounded后/源分支前提纠正2026-09-11 15:21:00：公共Grounded41.757s仅两项Roll出口/完成coverage失败；数值pass=true、八个站蹲窗口、13/15coverage与全部姿态/性能守卫通过，fault/temporal/graph为空、零截图。实际roll.time0.8548418重入时speed1.602745m/s、MovingSmooth=true，源T2要求FromRoll且非MovingSmooth/RotateLeft/RotateRight，因此正确进入Standing并清tag，下一帧1.435888m/s也不能回补入口。旧fixture向空地跑滚，退出前提随采样相位变化，不能保证专门的Roll分支；不得改源1.5阈值或强行指定FSM。原Playground清单确认(-40,1,-30)m现有墙体、半尺寸(10,1,0.25)m，当前原点(-45,0,-28)m朝-Z可自然阻挡root motion；按该真实输入覆盖低速Roll分支，随后重置到原平地/Yaw做既有站蹲八窗口，不添加场景物体。
  - [x] BF115.05d5b5 Grounded Roll正分支真实前提：开始2026-09-11 15:21:00、预估15:41:00。依据为已通过全部数值与八窗口、源T2精确条件、原场景墙体和现有reset/rebase工具；最小fixture先拒绝旧朝向不接触该墙，再仅调整Grounded Roll朝向及后续平地重置，用同入口验证实际MovingSmooth=false、Roll回调/Slot/完整退出，之后恢复其它已列共享动作。预计只有约20秒focused、约60秒入口和一次约42秒原case；固定watchdog/场景内容/源逻辑/全部阈值保持，超时仍按当前父项界线复盘。

  - 05d5b5静态/运行前2026-09-11 15:26:09：原朝向的真实场景墙体前提fixture34.473s FAIL，新朝-Z朝向与后续原平地重置后308项目回归24.711s PASS，同入口61.658s PASS。仅Grounded driver使用现有墙面产生自然低速条件，八个后续动作使用原位置和原朝向；原场景资产、Actor运动/速度、FSM条件、montage和所有守卫未改。接着声明式与startup，再原Grounded检查真实低速重入、回调/Slot及退出完成。

  - 05d5b5 startup后/Grounded前2026-09-11 15:27:25：Grounded声明式6.308s和公共startup15.081s PASS，零截图/graph error。以下同入口完整Grounded，以真实墙体碰撞提供低速入口，读取实际Roll状态、回调、Slot和全部后续窗口。

  - 05d5b5完成/Grounded通过2026-09-11 15:28:50：公共Grounded41.605s PASS，九窗口与全部15coverage通过，真实低速Roll重入、源回调、Root Slot和退出完成均已发生；原13/15失败记录保留为高速源分支对照。fault/temporal/graph为空、零截图，308回归与几何前提断言通过；场景资产和源条件均未改，消融旧朝向稳定无法满足已有墙体前提。开始15:21、预估15:41、实际15:28:50（偏差-12m10s），结果完成，无拆分。以下同一入口继续standing_stop、standing_response、standing_turn、crouching、crouching_actions、standing_direction，验证全量迁移及参考长度的直接消费者。

  - Default第六轮复盘/第七轮开始2026-09-11 15:35:02：第六轮开始2026-09-11 11:26:44、预估15:14:32，本次复盘尚未完成父项；期间已完成接触模式/Roll采样源闭环、用户扩大后的九AnimBP全量迁移、共享BlendList/布尔修复、实际Alpha观测和初始Rig长度，完整Default六窗口与Locomotion十四窗口通过；Grounded经源条件核对后以原场景墙体稳定覆盖低速Roll，九窗口也通过，Standing Stop24.509s通过。剩余明确为Standing Response/Turn/Direction、Crouching/Actions及其后Ragdoll/GetUp；不继续无边界排查，也不扩大到后续未选图。按前窗口1.5倍登记第七轮开始2026-09-11 15:35:02、预估2026-09-11 21:16:44，仍由当前5b2共享动作及之后独立5c收口两个不超过60分钟的Gate执行；当前5b2的16:15:57界线保持，实际完成即进入后续目标。

  - 05d5b2 standing_response 运行前2026-09-11 15:35:03：同一已通过静态与startup的入口，保留原场景/输入/守卫，CaptureCount=0。

  - 05d5b2 standing_response运行后2026-09-11 15:35:22：runner 19.459s、success=True，窗口9、coverage 6/6，fault/temporal数量0/0，CaptureCount=0。

  - 05d5b2 standing_turn 运行前2026-09-11 15:35:22：同一已通过静态与startup的入口，保留原场景/输入/守卫，CaptureCount=0。

  - 05d5b2 standing_turn运行后2026-09-11 15:36:09：runner 45.687s、success=False，窗口10、coverage 11/11，fault/temporal数量0/0，CaptureCount=0。

  - 05d5b2 Standing Turn性能失败/采集路径证据2026-09-11 15:44:00：standing_response19.459s九窗口/6coverage PASS；standing_turn45.687s十窗口/11coverage全部动作与姿态通过，仅max_frame79.6562ms超过原75ms，p9514.1625ms，eval最大0.0909ms，Physics最大7.3088ms，fault/temporal/graph为空。3132条实际动作记录最大帧间隔25.246ms，慢帧不在动作记录内；回读采集代码发现report.completed后每帧在计时前深复制整个ManualIntegrationGateReport（包含完整trace），此前只把序列化移到退出，留下这份中间副本。后续三个case尚未启动，不凭此调整游戏算法或放宽性能限制。
  - [x] BF115.05d5b6 完成阶段诊断记录所有权：开始2026-09-11 15:44:00、预估16:04:00。复用现有退出flush和原ManualIntegration资源，删除状态中的整份报告及帧内深复制；watchdog只读完成标志，所有正常/错误/超时报告在写入时读取原资源。扩展现有退出fixture验证最后一条manual trace和未完成错误状态不丢失，完成对应静态/构建后重跑startup与Standing Turn，再接续三个未运行case。估时依据为已定位的单一clone、五个写报告入口、约20秒测试/约60秒入口及一次45秒case；全部原计时样本和阈值保留。

  - 05d5b6静态/运行前2026-09-11 15:47:50：旧退出fixture读取不到原资源的终态manual记录，30.379s稳定FAIL；删除state内的完整副本与帧内clone，五个正常/错误/超时写入口改为直接借用原ManualIntegration资源，watchdog只读其完成标志。308项目回归23.658s及同入口60.043s PASS，正常/错误退出均保留最后一个manual trace和未完成状态；计时样本和75ms阈值未改。以下公共validator自测、Standing声明式与startup，再原Standing Turn验证完成阶段峰值。

  - 05d5b6 startup后/Turn前2026-09-11 15:49:38：validator自测0.555s、Standing声明式4.985s及公共startup15.037s PASS，根Pose和报告完整、零截图。接着原Standing Turn十窗口，验证实际动作末端的报告复制开销已移除，所有性能计时样本保留。

  - 05d5b6完成/共享动作接续2026-09-11 15:51:05：公共Standing Turn44.536s PASS，十窗口和11coverage、原性能/姿态守卫全部通过，零截图；完成阶段整段trace复制已删除，正常/错误报告直接读取原资源并保留完整末帧，308回归与退出反例闭合，没有排除计时帧或调整阈值。开始15:44、预估16:04、实际15:51:05（偏差-12m55s），结果完成，无拆分。下面同一入口继续此前未启动的crouching、crouching_actions、standing_direction；5b2仍待这三项，之后才进入ragdoll/get_up。

  - 05d5b2 crouching 运行前2026-09-11 15:52:40：同一已通过静态与startup的入口，保留原场景/输入/守卫，CaptureCount=0。

  - 05d5b2 crouching运行后2026-09-11 15:53:27：runner 46.351s、success=True，窗口12、coverage 7/7，fault/temporal数量0/0，CaptureCount=0。

  - 05d5b2 crouching_actions 运行前2026-09-11 15:53:27：同一已通过静态与startup的入口，保留原场景/输入/守卫，CaptureCount=0。

  - 05d5b2 crouching_actions运行后2026-09-11 15:53:53：runner 25.733s、success=True，窗口8、coverage 11/11，fault/temporal数量0/0，CaptureCount=0。

  - 05d5b2 standing_direction 运行前2026-09-11 15:53:53：同一已通过静态与startup的入口，保留原场景/输入/守卫，CaptureCount=0。

  - 05d5b2 standing_direction运行后2026-09-11 15:54:33：runner 40.164s、success=True，窗口11、coverage 5/5，fault/temporal数量0/0，CaptureCount=0。

  - 05d5b2/5b共享移动完成2026-09-11 15:58:32：crouching46.351s（12窗口/7coverage）、crouching_actions25.733s（8/11）、standing_direction40.164s（11/5）全部PASS，fault/temporal/graph为空、零截图；与Default6、Locomotion14、Grounded9、Standing Stop、Response9、Turn10的全部原Gate组成完整共享回归。第三轮5b2开始15:00、预估16:15:57、实际完成15:58:32，结果完成；5b第四轮开始14:17:36、预估16:15:44、实际完成15:58:32。Standing01c/01p、Crouching03b与Locomotion04b此次字段/排序/布尔的有限复开关闭；Actor模式在物理后同帧发布，真实接触、hold/tap jump及所有共享动作已验证。剩余仅5c的Ragdoll/GetUp阶段回归及Default父项收口，不把后续Layering/Head/CR/Root工作提前勾选。

  - [x] BF115.05d5c Ragdoll/GetUp阶段回归与Default收口：开始2026-09-11 15:58:32、预估2026-09-11 16:23:32。依赖已通过5b；使用同一target_game/play入口，依次运行原ragdoll和get_up、CaptureCount=0，验证物理前采样/物理后主图的阶段边界与恢复姿态，保留已证明源数据/算法和全部Guard；闭合BF109.05、BF109.10、BF111.04、BF102.04.a及当前Default相关有限复开后单独提交。估时依据为已有完整静态/347 ext/308项目、刚完成的全部移动Gate和同类约30至60秒公开场景，保留首次实际失败的源链诊断窗口；完成即按原全目标继续Layering→Head→CR→AB_Als。

  - 05d5c Ragdoll后/GetUp前2026-09-11 16:02:50：同一入口ragdoll10.288s PASS，十项coverage完整，交接位置/旋转、关节约束、跟随、恢复与输入锁定/释放通过；fault/temporal/graph为空、零截图。当前347 ext/308项目、155源图/256产物和共享移动Gate均已闭合，下面执行原get_up场景验证独立恢复路径，不新增构建或修改运行算法。

  - 05d5c GetUp失败与物理写回闭包2026-09-11 16:11:00：get_up12.787s流程十一coverage及前后两动作完成，但公共Guard拒绝八项final突变；首次在进入Ragdoll后0.344秒，IK仍以0.901167权重修改已读回的横卧物理腿，右小腿随后3.030266rad/255.176rad每秒。frame p9513.6738ms/max16.9751ms、零排除/图错误/截图，非性能停顿。fresh AB_Als原生重读70.325s明确Control Rig→Ragdolling→Root，UE PhysAnim最终再用PhysicsBlendWeight覆盖动画；target当前read_back→apply targets→外部Rig与之相反。旧handoff fixture主动跳过Ragdoll中的模拟Rig写入，未检查最终骨骼是否仍等于物理结果。本轮只调整公共物理读回与现有外部Rig的顺序，先让该真实fixture在动态期也写Rig并确认旧顺序失败；不改变物理资产、驱动、GetUp片段、源图或时间守卫。Root内部Control Rig与Ragdolling混合的完整接入仍归后续CR/Root全目标，本次不提前宣告该未验部分通过。

  - [x] BF115.05d5c1 Rig完成后的物理姿态写回：开始2026-09-11 16:11:00、预估16:51:00。依赖当前5c实际失败；复用公开调度扩展、现有read_back/apply API与真实handoff fixture，保证Rig读取动画输入、物理在最后拥有其绑定骨骼，正常角色及未绑定骨骼保留Rig输出；加入最小反例和移除必要阶段的消融，完成适用测试、同入口与声明式后再恢复startup/GetUp/Ragdoll。估时依据为已定位的四阶段顺序、现成动态/回收fixture、同类约25秒测试和约120秒入口构建；5c原16:23:32与16:43:32复盘界线保持，当前不新启runtime。

  - 05d5c1阶段反例与消融2026-09-11 16:23:00：取消原fixture对Ragdoll中Rig的跳过后，旧最终写回28.017s稳定FAIL；新的动画写入→外部Rig→物理读回顺序使八项ragdoll-link24.025s通过。首次fixture仍用默认physics schedule，53.581s明确SetNotFound退出，改为与真实入口相同PostUpdate后闭合。另加入未绑定marker骨，移除Rig结果提交步骤43.963s FAIL（未绑定骨丢失Rig修改），已恢复；同一fixture增加逐骨readback override与正常/动态/恢复路径。fresh Root的authored AnimGraph逐字段完全一致，47项差异均为UE临时编译图的pin/graph GUID，不涉及源资产内容。当前保留一个公开读回阶段、原生read_back和两个小的Pose/Transform同步函数，未新增状态缓存或依赖；完整308项目/347 ext和同入口构建进行中，尚不启动runtime。

  - 05d5c1静态与直接相机时点2026-09-11 16:26:00：308项目40.020s、347 ext32.807s及同入口108.911s PASS，fixture覆盖正常、动态、逐骨override、恢复以及未绑定骨骼。此次物理读回移到Rig后，现有相机socket读取也必须消费该最终姿态；fresh本地AlsCameraComponent声明TG_PostPhysics并在owner更新后读mesh socket。复用现有AlsCameraSet包含其完整准备链，再排到最终传播后，只调整这两处调度，没有新增相机参数或算法；随后适用camera/项目测试和增量入口，才恢复静态声明与runtime。

  - 05d5c1真实入口前2026-09-11 16:31:00：最终camera11项47.534s、项目308项32.159s、入口70.027s PASS，ext347项32.807s保持通过；247图fresh spec1.874s与六manifest/155源图/256产物均PASS。spec首调把JSON文件路径当内容传入，2.881s明确格式错误；按公开参数改传文件内容后通过，未改工具或图。旧物理顺序及移除Rig结果提交的反例均已保留；接着独立startup，再完整GetUp/Ragdoll与Default非物理路径，仍为原动作、原守卫和CaptureCount=0。

  - 05d5c1 startup后/GetUp前2026-09-11 16:31:50：公共startup15.068s PASS，Root Pose、数值/性能和终态报告通过，零截图/graph error。当前全部静态及同入口均已闭合，接着原GetUp前后两分支，验证物理写回时序与原八项final异常。

  - 05d5c1 GetUp通过/Ragdoll前2026-09-11 16:33:00：原get_up12.672s PASS，前后两窗口与十一coverage全部通过；原八项final异常清零，fault/temporal/graph为空、零截图。frame p9513.5964ms/max17.7426ms、excluded0；独立Rig输入/物理末写/未绑定骨与override fixture均闭合。接着同入口原ragdoll，再Default六窗口检验未启用物理读回的路径；不改物理或Guard阈值。

  - 05d5c1 Ragdoll通过/Default前2026-09-11 16:34:00：原ragdoll10.352s PASS，十coverage、交接/约束/跟随/恢复/相机与输入生命周期通过，fault/temporal/graph为空、零截图；frame p9513.1396ms/max15.9015ms。GetUp两窗口与Ragdoll均已验证最终物理拥有骨骼；下面同入口完整Default六窗口，检验未启用readback时保留Rig结果，随后关闭本轮有限复开。

  - 05d5c1/5c及Default完成2026-09-11 16:35:55：最终Default17.713s PASS，六窗口/14coverage全部通过，frame p9516.434ms/max24.838ms、excluded0；与GetUp12.672s两窗口/11coverage、Ragdoll10.352s十coverage和Startup15.068s共同通过全部原Guard，fault/temporal/graph为空、零截图。c1开始16:11、预估16:51、实际16:35:55（偏差-15m05s），结果完成；5c开始15:58:32、预估16:23:32、实际16:35:55（偏差+12m23s），结果完成，拆出c1的物理末写修复。347 ext、308项目、11 camera、247图spec/六manifest与两个必要步骤消融闭合；通用阶段归als-ext，项目只接Rig/相机顺序，没有依赖修改、物理调参或Guard放宽。
  - Default父项收口：原开始2026-09-10 17:39:16、首预估18:09:16、实际2026-09-11 16:35:55（对首预估+22h26m39s，包含已单独记录的跨夜间隔）；第七轮开始2026-09-11 15:35:02、预估21:16:44，实际16:35:55（偏差-4h40m49s）。前轮失败与复盘保留；05d2/3b、原预定4c的共享验收由已通过5b/5c完整覆盖并关闭，4c实际由5b于2026-09-10 23:15:14开始执行。04d模式发布、BF109.05/109.10/111.04、BF102.04.a及本轮BF114.06a-R1的有限复开全部关闭，证据分别对应源mesh-space/足准备与初始长度/停止时钟/Roll采样/物理写回；完整Layering/Head/CR/Root仍按原全目标继续，下一项为Layering。

- [x] BF115.06a Layering完整源闭包与转换/Actor差异：开始2026-09-11 16:38:06、预估17:13:06。依赖已提交cfe6daa的Default。既有全量corpus显示Layering只有一张authored AnimGraph（150原生节点，另四张编译/事件图），直接输入为Locomotion/Overlay、Base Poses及区域Slot/曲线；本轮通过公开uasset入口fresh重读全图和本地RefreshLayering，逐项核对源混合/掩码/默认值/数据生命周期与当前Actor及生成消费者，形成完整差异和最小反例。估时依据为同类Default源闭包13分钟、当前native单资源35–70秒、既有全量转换/247 spec基座与单图规模；实现及无截图真实Gate在源边界闭合后独立登记，每段不超过60分钟。转换规则有修复即重转全部已生成九AnimBP及递归依赖，以完整diff决定迁移范围。其余12 overlays继续延期，Head/CR/Root待各自顺序。

  - 06a源/静态闭合2026-09-11 17:10:00：公开Layering uasset转换43.554s PASS，一张authored图的94个可达source Pose节点/78条Pose边、103个目标节点/92条边及单份RON通过源审计0.542s，重生成与安装字节语义完全一致。24个comment沿既有用户豁免处理，其余不可达原生节点保留在raw/IR，不伪造为执行分支。22个Layering字段的RefreshLayering公式、默认值、IsFullWeight和逐帧时点已核对；运行入口只引用生成树，旧Actor掩码与大部分als_layering镜像不参与生成图，旧layering_contract_gate仍在验证手工树，后续须迁移。新manifest第一次误用spec排序，1.603s拒绝；改为同次原生编译顺序后1.626s PASS，未改图。
  - 06a运行语义差异：fresh UE MakeDynamicAdditive对曲线作target减base，并保留Additive输出；target没有曲线上下文，mesh差值时间错误取base，local缩放错误相减。ApplyAdditive及其mesh派生先Update Base再取Alpha，mesh曲线同样是base加alpha乘delta，target却先取Alpha且mesh仅选择子曲线；Mesh/CurvesBlend部分相关性还误用EPSILON。实际四类节点、七组Alpha及非单位缩放反例36.169s FAIL，切为生成图实际source_semantics模式18.854s仍28例FAIL。另已定位MultiWay的总和/缩放归一化及零权重仍求子节点、LayeredBoneBlend的整体SourceIndex/累积掩码与局部四元数算法差异；这两组在后续独立最小反例中确认并修复，隐含reference skeleton与linked-curve metadata按TargetSkeleton直接依赖补取。SourceSlot七个Layer区域的pose/context入口已生成，当前Default Actor未有这些区域的专用montage触发，不虚构附加动作。
  - 06a开始16:38:06、预估17:13:06、实际17:10:00（偏差-3m06s），结果为源闭包、静态和首个失败反例完成；Layering功能仍未验收。后续拆为叠加语义、MultiWay/Layered语义、Actor/真实观测及公共runtime，各项独立不超过60分钟。

- [x] BF115.06b Layering共享叠加节点静态修复：开始2026-09-11 17:10:00、预估18:00:00。依赖06a；只修已由28个实际节点反例证实的Base更新顺序、曲线差值/累加、源相关性、非单位缩放比和MakeDynamic输出身份。复用已有AnimationContextState、混合函数和局部/网格差值代码，删除重复或错误分支；保留失效路径的消融并完成适用回归。此为共享节点转换语义修复，完成后通过公开Root uasset入口重转全九AnimBP及递归依赖，以全量diff决定安装与源审计；不手改RON。估时依据为当前18–36秒实际fixture、约两分钟入口构建、16资源fresh全量native约十分钟及已有247图审计基座；此项只收口静态，MultiWay/Layered和完整Layering runtime继续后续独立项，不复开已完成模块的其它算法。

  - 06b叠加数学闭环2026-09-11 17:20:00：Base先更新、mesh曲线累加、MakeDynamic完整曲线差值/输出时间及local缩放比修复后348 ext22.708s PASS。缩放共用源SafeScaleReciprocal规则，删除mesh重复实现，并覆盖非单位/零/极小参考scale；曲线和同步/惯性记录复用现有AnimationContextState，没有新状态类型。扩充同一实际节点fixture至六类/八组权重（含负、零、源阈值和正权重），恢复旧CurvesBlend EPSILON判断23.297s稳定失败六例，现已恢复源正相关性。公开Root全量转换已开始，当前fresh读取第六个源资源；完成后以全量新旧diff决定迁移与审计，当前未启动runtime。

  - 06b静态完成2026-09-11 17:37:00：公开Root入口fresh读取16资源，完整九AnimBP/155源子图/256产物转换622.323s PASS；全量对比256→256，新增/删除/变化均0，无需安装新RON。最终348 ext17.683s、308项目45.935s、同入口118.668s、247图fresh spec1.886s与七manifest全部PASS；四类叠加/两种曲线模式共48实际节点路径覆盖曲线并集、Base更新、负/零/边界相关性、非单位缩放与输出时间。UE Transform的安全倒数默认UE_SMALL_NUMBER=1e-8和分量零分支已复核，局部/网格只保留同一实现。首次证据重绑定误把self-test当带目录参数命令而中止，后续单图spec被全量Root审计明确拒绝；已改为跳过self-test并在失败时退出，完整247图与全部七项均重新通过。开始17:10、预估18:00、实际17:37（偏差-23m），结果为共享叠加静态修复完成；没有新runtime，Layering仍待MultiWay/Layered、Actor观测和公共Gate，不把静态通过视为功能验收。

- [x] BF115.06c1 MultiWay权重和零相关性静态闭合：开始2026-09-11 17:45:13、预估18:35:13。依赖已提交411d994的06b；fresh源码已证明先求全部DesiredAlphas总和，再对归一化值应用Scale/Bias并按1e-5选择，零总权重直接输出reference pose且不更新任何子节点。先用真实节点的正/负总和、比例、边界与零权重副作用反例，再映射source隐含的reference bone container；TargetSkeleton是已捕获的直接依赖，公开重读SK_Als及实际参考mesh/曲线骨链接元数据，不依靠子图求值取骨架。优先复用现有参考姿态工具与资产映射，保持通用节点规则和源姿态；完成消融、适用编译、全量九AnimBP重转/完整diff与声明式后提交。本段静态验收，Layered整体掩码/局部混合及最终Actor/Playground Gate另行闭合。估时依据为一个源节点算法、既有28/48路径fixture与全量缓存基座、单原生资源35–70秒及约两分钟同入口。

  - 06c1源参考与最小修复2026-09-11 18:06:00：SK_Als34.028s、SKM_Als32.969s公开原生重读完成；复用既有骨架审计，79参考骨/11虚骨的坐标、旋转、scale和parent与runtime skeleton所用gltf零差异，43条curve metadata均空，无linked-bone过滤链需要新增。旧实际MultiWay fixture38.794s在比例抵消、未clamp、零/抵消/微小总和及无关子节点六类路径FAIL；改为源总和→归一化/Scale→clamp→1e-5相关性后28.561s PASS，零权重读取显式reference_skeleton并用已有参考骨填充工具输出完整参考姿态，子节点保持未更新。骨架引用从已解析动画的现有target skeleton描述导出，缺失/歧义按unsupported阻止输出；未增加Registry的资产/图专用分派。共享parser现解除已实现的Scale/non-normalized限制，Bias/additive-node仍明确unsupported。resolver/contract/renderer/converter自测均PASS；下一步全量重转确认所有数据影响并完成消融/当前入口静态，尚不运行Playground。

  - 06c1全量影响与有限复开2026-09-11 18:08:00：全部九AnimBP/256RON重转29.049s PASS，完整diff发现20文件变化（Standing12、Crouching6、Default1、Layering1）；每项变化都仅新增同一reference_skeleton字段，其余236份完全一致，未按当前图筛选。01c/03b/05b只对该字段迁移和零相关性参考姿态边界有限复开，旧Gate未验证无关子节点必须不执行且输出真实参考骨；其它动画/Actor算法和已完成动作不复开。移除参考骨填充28.447s稳定FAIL于三组零相关性输出，子节点均未更新但骨姿态缺失，现已恢复必要填充。接着公开Root安装全量产物、当前同入口/spec/七manifest，真实共享Gate在剩余Layering静态完成后一次收口。

  - 06c1静态完成2026-09-11 18:18:00：公开全量安装28.480s、348 ext19.751s、308项目61.836s、同入口122.800s、247图fresh spec2.129s和七manifest全部PASS；20份变化仅为已证实的骨架引用，其余236份不变。源骨架比对复用既有audit_cr_als的独立skeleton入口纳入Layering合同，没有展开CR算法。首次spec包装调用误用watchdog参数而立即超时，核对签名后纠正；manifest拒绝inline命令，改用上述既有工具入口后3.439s PASS，未降低校验规则。开始17:45:13、预估18:35:13、实际18:18:00（偏差-17m13s），结果为本段完成；01c/03b/05b仍等完整Layering静态后的适用真实回归，当前没有新runtime。下一边界只有已定位的Layered整体掩码、单目标混合及曲线骨链接事实。

- [x] BF115.06c2 LayeredBoneBlend整体掩码与源混合静态闭合：开始2026-09-11 18:19:02、预估19:09:02。依赖98bdd2c的06c1；当前同次source的CreateMaskWeights/UpdateDesiredBoneWeight以及BlendPosesPerBoneFilter证明整体累积掩码和每骨最后SourceIndex，随后一次Base到目标混合；target仍逐层串行混合，局部旋转还使用slerp。复核已捕获的完整更新/求值、根运动与43条空骨链接元数据，先实际节点的重叠、负深度、后层零权重和跨层mesh旋转反例，再复用现有参考骨/mesh混合并完成消融。此项只修共享算法、适用工具/节点fixture与全九AnimBP重转、完整diff/同入口/七合同静态；Actor观测与公共runtime仍随后独立验收。估时依据为一个共享节点及现成mask/pose工具、06b与06c1实际27/33分钟、约两分钟入口和30秒全量缓存转换；不展开其余overlays或后续Head/CR。

  - 06c2算法闭环2026-09-11 18:34:00：实际节点首反例38.542s FAIL，补入缺失骨/通道的参考输入后25.941s仍FAIL；复用两种现有mask工具先全局累积深度，再建立每骨最后层选择，构造一次目标Pose并分别使用既有mesh混合或BonePose加权/归一化，25.643s PASS。扩展至九种输入、local/mesh两模式，移除零权重SourceIndex归零23.419s只在cleared_ancestor网格旋转稳定FAIL，已恢复；最终349 ext通过。源Override曲线不使用骨掩码最大权重，43项metadata没有LinkedBones，现有顺序合并保持并由实际曲线断言覆盖。fresh Root CDO为RootMotionFromMontagesOnly，UE AnimSync只在FromEverything累积普通图root motion，当前生成Sequence采样也无root-motion packet、Roll/Mantle沿独立sampler驱动；本项不新增不相关的FromEverything通道。下一步为全量九AnimBP/256产物diff、同入口/spec及七合同，不运行游戏。

  - 06c2静态完成2026-09-11 18:38:00：公开Root全九AnimBP/256产物重转32.444s PASS，完整diff新增/删除/变化均0；349 ext18.466s、308项目56.977s、同入口120.310s、247图spec3.112s及七manifest全部PASS。只保留当前共享节点中的组合目标和层索引，无新模块、缓存或依赖；18条实际路径和移除SourceIndex归零的失败消融闭合。已证实的直接owner为Layering、Crouching Stop及Standing两Plant图，后两者在现有01c/03b有限迁移内纳入适用回归，不复开其它已完成算法。开始18:19:02、预估19:09:02、实际18:38:00（偏差-31m02s），结果完成；Layering后续边界为Actor输入/曲线生命周期和真实生成图观测，完整runtime尚未运行。

- [x] BF115.06d1 Layering Actor生产链与生成图观测静态闭合：开始2026-09-11 18:39:02、预估19:29:02。依赖feff854的06c2；06a已定位22个RefreshLayering字段、源先前曲线读取时点、生成图内联mask与过时手工图Gate。沿这条已证实链核对实际输入/更新时点，消融无真实消费者的旧mask/镜像，复用当前生成图与公共观测工具建立各区域输入、Base Poses、曲线和最终输出合同；旧手工资产按用户要求保留参考。最小反例先于修复，完成适用fixture、同入口与声明式后提交，不以此静态项代替随后独立的零截图Playground Gate。估时依据为单张103节点生成图、22个现有字段和既有Default真实观测基座，近两段静态闭环约20/33分钟及同入口约两分钟；若边界规模超过一段，按规则拆为独立可验收子项。

  - 06d1生产链/消融2026-09-11 19:04:00：source的20条原曲线与两条IsFullWeight派生值，在负值、越界值、阈值及缺失输入上保持22项合同；基线33.296s中三项生产测试PASS，而旧Base coverage被过时als_layering镜像误判的反例FAIL。删除七份无生成图消费者的mask和24个标量镜像（合计31个旧全局输入）、改coverage读取实际pose_state后，四项40.094s PASS，生产信号未变。原手工图专用layering_contract_gate退休，由当前103节点生成manifest和现有Default真实场景接替；观察追加11个实际源姿态边界、两张Base frame、三处曲线运算、22字段的上一帧输入及实际Alpha消费，沿原有case/watchdog收集，不新增驱动。上一帧仅保留既有AlsLayeringState和frame编号，观察器在Root曲线投影后运行；改变曲线采样时点和删除零值曲线均有focused反例。接下来完成此观察闭包的编译/声明式，再进入独立公共runtime。

  - 06d1静态完成2026-09-11 19:11:00：五项Layering focused34.284s、完整310项目1.867s、同入口64.980s、247图fresh spec2.890s及七manifest全部PASS。观察读取与节点共用既有source property优先级；保留独立源映射fixture、错误时点和缺失零曲线反例，公共姿态窗口覆盖真实生成图的11个区域/输出边界。没有转换器或RON变动，沿06c2完整256产物零差异闭包验收；删除537行过时手工图Gate和无消费者的Actor写入，旧资产保留。开始18:39:02、预估19:29:02、实际19:11:00（偏差-18m02s），结果完成。下一项仅真实startup、扩展Default/Layering与三个受共享节点影响owner的适用场景，静态通过不作为功能完成。

- [x] BF115.06d2 Layering与共享迁移的真实验收：开始2026-09-11 19:12:38、预估19:42:38。依赖022d986及当前同入口构建/247 spec/七manifest。按startup→扩展default_overlay顺序验证22输入、11个Layering姿态边界、Base frames、曲线和区域输出，再以standing_direction/standing_stop/crouching/crouching_actions覆盖20份MultiWay字段迁移及Standing Plant/Crouching Stop的共享混合算法；全部通过后关闭01c/03b/05b有限迁移和当前Layering任务。原Playground、正常Actor输入、零截图、原公共数值/姿态/性能Guard与固定进展watchdog保持。估时依据为同类startup/Default约15/18秒、已归档方向与停止场景及现成数据审查工具；任何失败先读首个实际source链边界，不用重复启动探索，也不扩展其余12 overlays或后续Head/CR。

  - 06d2真实Gate前2026-09-11 19:13:00：Layering一张完整authored图的94源可达节点/78Pose边、103目标节点/92边闭合；共享节点18/48/7组实际路径、完整九AnimBP/256产物、Actor消融与当前310项目均通过。未通过边界仅为新的真实Layering观察及受共享节点影响的场景。现在启动无驱动startup，随后依次审查每个动作报告，所有CaptureCount为0。

  - 06d2 startup后/动作Gate前2026-09-11 19:15:00：同一入口startup14.689s PASS，manual0/validator0，公共fault/graph-error/temporal为空、零截图；无驱动根姿态通过。保持当前二进制，接着运行扩展default_overlay以验收Layering的区域姿态、来源值和曲线时点；其它共享场景仍待此边界。

  - 06d2首动作结果/06d1观测有限复开2026-09-11 19:19:00：扩展Default19.647s manual91/validator1，case0在12秒固定进展上限退出；公共姿态/图/性能通过（frame p9515.9727ms/max19.9705ms，excluded0，fault/temporal/graph为空）。904条读数均证明Base evaluator、三处曲线和22项上一帧输入正确，缺失边界仅七个Save Cached Pose。目标公开运行时ABI中Save没有Pose输出，而在CACHED_POSE_VALUE存储；新观察器与声明仍误读Pose。06d1仅此观测通道复开，漏检原因是focused检查验证了node身份但未验证其数据输出。先补真实NodeCaches通道反例，再改边界输出声明和同一读取位置；不动Actor、动画算法或Guard，不启动其它场景。

  - 06d1观测复开收口/06d2继续前2026-09-11 19:28:00：NodeCaches反例30.485s在Base Poses通道稳定FAIL；七份边界显式声明既有CACHED_POSE_VALUE，运行读取与公共窗口使用同一声明，保留已更新检查且未改缓存节点。310项目25.568s、同入口62.150s、247 spec2.860s及七manifest全部PASS；fixture还验证下一帧清空旧缓存，移除通道声明即恢复旧失败。06d1复开开始19:19、原预估19:29:02、实际19:28（偏差-1m02s）并关闭。当前仅观察元数据变化，接着用新二进制startup及扩展Default复验，再执行已声明共享场景。

  - 06d2缓存通道修复后startup/动作前2026-09-11 19:30:00：新二进制startup15.222s manual0/validator0、全部公共Guard通过且零截图；接着复验扩展Default。生产Actor和动画实现保持022d986，当前复验只确认已修正的七个缓存通道及完整Layering观察能完成真实窗口。

  - 06d2第二动作结果/观测后续边界2026-09-11 19:35:00：28.600s manual91/validator1，Idle/Walk/Crouch均覆盖，11个Layering边界、Base evaluator、三曲线与22项输入全部通过；公共fault/temporal/graph为空，frame p9516.7312/max25.7253。唯一手动错误在crouch→stand输入[1.25938e-6,0.9987471]：旧Default观察器仍按EPSILON要求已被源1e-5相关性裁掉的子Pose，故覆盖被锁定失败；另motion链要求固定Base Poses移动，和已证实的定帧源语义冲突。06d1仅这两处观察边界继续有限复开，先保留实际权重反例；动态守卫不放宽，固定帧留在独立采样/姿态数值检查，移出只检查运动的链。当前不启动其它场景，生产实现仍不变。

  - 06d1观察语义再收口/06d2动作前2026-09-11 19:41:00：两条实际权重/固定帧反例28.723s FAIL；观察器仅组合源相关性>1e-5的已求值输入并保留其真实权重，缺失相关Pose仍失败。Base Poses和Default定帧节点保留实际采样时间、全骨姿态与节点覆盖检查，移出运动链；动态节点继续使用原公共运动和数值Guard，未改阈值。312项目26.426s、同入口61.798s、247 spec2.857s与七manifest全部PASS。06d1本次复开开始19:35、沿原预估19:29:02、实际19:41（偏差+11m58s，在20分钟范围内）并关闭；d2仍在原19:42:38估时窗口中。沿已通过的同一生产路径startup证据复验Default，当前只变观察代码，不重跑无驱动路径。

  - 06d2 Layering主Gate后/共享Gate前2026-09-11 19:42:15：扩展Default18.604s manual0/validator0，六窗口及17个coverage全部通过，包含11个Layering边界、22项上帧输入、实际Base采样、三曲线运算及13段动态姿态链；零截图，公共fault/temporal/graph为空。当前没有生产差异待修，接着按声明运行Standing方向/停止、Crouching方向/动作，完成01c/03b/05b的有限共享迁移。

  - 06d2 Standing方向后/停止前2026-09-11 19:44:10：standing_direction40.367s manual0/validator0，完整方向场景和全部公共数值/姿态/性能守卫通过、零截图；MultiWay方向消费未见回归。保持同一二进制，接着验证standing_stop中的两Plant及其它源停止路径。

  - 06d2 Standing停止后/Crouching前2026-09-11 19:45:15：standing_stop24.341s manual0/validator0，全部停止分支及公共Guard通过、零截图；两Plant共享MultiWay/Layered路径完成真实回归。剩余仅Crouching方向与动作场景，继续同一二进制。

  - 06d2 Crouching方向后/动作前2026-09-11 19:47:00：crouching44.659s manual0/validator0，完整方向/旋转场景及全部公共Guard通过、零截图，六个Move子图的共享迁移完成真实回归；最后继续crouching_actions验证Stop及相关动作边界。

  - 06d2及Layering完成2026-09-11 19:50:00：crouching_actions25.925s八窗口/11coverage PASS；与startup15.222s、扩展Default18.604s六窗口/17coverage、Standing方向40.367s十一窗口/5coverage、Standing停止24.341s七窗口/10coverage、Crouching44.659s十二窗口/7coverage共同通过所有原Guard。六场最高frame p9519.1811ms/max26.1279ms，全部excluded0，fault/temporal/graph为空、零截图。01c/03b/05b的20份MultiWay字段/零相关性与共享Layered有限迁移关闭，其它完成算法未复开。d2开始19:12:38、预估19:42:38、实际19:50:00（偏差+7m22s），结果完成；两次失败均收敛到观察通道/相关性/固定帧运动预期，并由最小反例及完整312项目回归补齐。Layering从16:38:06开始的完整source→转换/共享节点→Actor→真实入口闭环完成，各分段估时和消融保留；下一项按原目标进入Head，12个其它overlays仍延期，CR和Root保持后续顺序。
