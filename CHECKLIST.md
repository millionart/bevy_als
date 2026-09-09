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

- [ ] BF115.01m Standing冲刺启动与稳定分支Gate：开始：2026-09-09 07:02:51 +08:00；预估结束：2026-09-09 07:37:51 +08:00。依赖已提交01l；当次本地RefreshStandingMovement与Standing图确认SprintTime<0.5使用前向加速度、pending update直接置0.5、停止冲刺清零，两条Sprint sequence按同一Movement同步组混合。以这两条实际播放器、同源输入和既有公共sprint场景为闭包，补齐启动/稳定分支及curve/pose观测后再运行。估时依据为现有7帧native函数fixture、已验证时钟/曲线/资产、当前入口增量编译约一分钟及方向Gate每场约41秒；不展开Grounded、相邻图或物理调整。

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
  - [ ] BF115.01m2 原场地连续冲刺验收：开始09:28:00、预估结束10:16:45。依赖01m1；提前开始并保留复盘后的48m45s窗口。实际原路径后段进入坡道、速度降为Walking/Running，不能完成稳定玩家收敛。先按原地图Floor与障碍bounds验证完整平地路径，修正唯一fixture前提，再公共sprint验证启动/稳定、相位签名、curve/pose和性能；不改地形或运动算法。

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
