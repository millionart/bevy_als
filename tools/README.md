# 工具入口

| 目录或入口 | 保留用途 |
|---|---|
| 根目录 `run_manual.ps1`、`als_graph_editor.ps1`、`run_manual_integration_gate.ps1` | Mac/Windows 共用入口；同名 `.bat` 保留 Windows 双击启动 |
| `python.ps1`、`requirements.txt` | 优先选择本工作区 `.venv`，依赖与准备步骤见根 README；本机 UE 路径由 `env.local.json` 提供 |
| `run_ue_source_reader.ps1`、`build_ue_source_reader.ps1`、`ue_graph_source_reader/` | 从本地 UE/uasset 提取图、属性与递归依赖；reader 带进展 watchdog 和 `-SelfTest` |
| `animgraph_authoring/` | AnimBP/Control Rig 转换、完整产物比较、源闭包审计及夹具；其中合同与地图仍被图审计 TODO 引用 |
| `import/` | 源事实转为独立 runtime 资产、单位/坐标换算、材质通道和图合同审计 |
| `export/` | 网格、材质、纹理、地形等资产导出；`export_playground_actors.py` 生成网格导出器消费的 actor 清单 |
| `uasset_curve_export/`、`uasset_root_export/` | 曲线、root track 的导出与验证；`probe_animdatamodel.py` 仍是可独立使用的原生格式诊断入口 |
| `run_animation_visual_validation.ps1`、`validate_animation_visual_evidence.py`、`validate_decal_evidence.py` | 当前 manual Playground 的公共运行与报告验收 |
| `validate_*`、`verify_*`、`*animgraph*`、`normalize_*` | 当前节点、上下文、布局、资产边界和编辑器审计 |
| `open_ue_asset_editor.ps1`、`ue_open_asset.py` | 本地 UE 资产编辑器辅助入口，不使用 MCP |
| `new-worktree.ps1` | 工作区及构建缓存准备 |
| `tests/test_platform_tools.py` | 平台启动参数、退出码、路径、原生 reader 库选择与缓存复制回归 |
| `examples/` | 可独立运行的 Rust 检查、图编辑器和规格导出；使用 `cargo run --example <name>` |
| `tests/` | Rust 集成测试；由 `cargo test --tests` 自动执行 |
| `manual_playground_integration/`、`animation_visual_validation.rs`、`anim_probe.rs` | manual Playground 的移植验收驱动、报告和诊断；保留 crate 内的遥测访问，运行时按 Gate 条件启用 |

公共视觉 runner 的长期证据写入 `docs/audit/`，默认目录为 `docs/audit/animation_visual/<Case>`。`-OutputDirectory` 必须位于该 Git 证据根目录，目标目录已存在时拒绝覆盖；重跑请选新的目录并随对应条目提交报告。截图关闭使用 `-CaptureCount 0`，性能验收暂停使用 `-FunctionalOnly`。
| `tmp/` | 被 Git 忽略的临时导出、中间文件和 `saved/als_debug.ron` 调试偏好；删除偏好文件会恢复默认类别 |

`import/probe_anim_graph_nodes.py`、`probe_property_access.py`、`probe_gltf_curves.py` 接受显式资产参数，仍可检查原生属性或导出曲线。没有其他脚本调用一个 CLI，不等于它已经无用。

`PhysicsContactProbe` 提供 UE 原生接触数据，用于复验历史取证。当前布娃娃功能按 [布娃娃物理边界](../README.md#布娃娃物理边界) 验收。

`run_ue_source_reader.ps1 -PhysicsContactProbe <request.json>` 的 capsule/trimesh sample 可选 `triangle_indices`，使用 cooked mesh 索引调用原生 `ConstructCapsuleTriangleOneShotManifold2`，输出合并前的 `triangle_contacts`。其中两个 `shape_point_*_cm` 都在胶囊形状空间，`world_point_*_cm` 与 `world_normal` 已转换到世界空间；组上的 `triangle_index` 是请求的 cooked face，未归并点的 `face_index` 保留原生−1。该选项不改变正常 `points` 或 `aggregate_contacts`；用 `python tools/tests/test_physics_triangle_contact_probe.py <output.json>` 核对输出。

接触 sample 在已有 `next` 时可选 `history`：`friction_ratios` 与首帧 manifold 点一一对应，`anchor_offset_0_cm` / `anchor_offset_1_cm` 是两侧形状空间的旧锚点偏移。读取器调用原生 `SetSolverResults` 和下一帧 `Activate`，输出 `saved_points` 与 `next_history`；这是受控保存/匹配探针，不运行 Chaos 动力仿真。使用 `tools/tests/fixtures/physics_contact_history.request.json` 和 `python tools/tests/test_physics_contact_history.py <output.json>` 核对8点保存边界、比例插值、匹配标志与世界锚点差。

`gjk_simplex` / `next_gjk_simplex` 分别保留两帧的GJK支持点和重心权重。冷重建使用另一个sample在相同当前姿态运行已有 `aggregate_mesh`，以独立的新约束作对照；`fresh_points` 仍是同一约束的重复重建，不代表冷查询。粒子的旋转按源 `PBDRigidParticles::SetQ` 保存为float32，旋转请求应采用同一精度。`physics_contact_gjk_history.request.json` 与 `test_physics_contact_gjk_history.py` 覆盖分离核心、EPA及无缓存路径；`project_initial_overlap_depenetration_velocity_cm_s` 来自项目的世界solver配置，不是PA中的RBAN配置。

`run_ue_source_reader.ps1 -InertializationProbe <request.json>` 接受AnimBP；有多个原生Inertialization节点时，用请求顶层的`node_property`选择原生CDO属性名，省略时必须只有一个节点。读取器复制该节点并通过公开Update/Evaluate入口运行受控单骨骼历史，输出也保留选中的属性名。每个sample提供`name`及`steps`；每步包含`delta_seconds`、源厘米单位的`translation_cm`、xyzw四元数`rotation`和`scale`，可选`curves`、单个`request_duration`或同帧数组`request_durations`。所有请求在该步唯一一次Update/Evaluate前排入队列。输出逐帧Transform、曲线及实际Enable/IgnoreVelocity/IgnoreDeficit；它不运行完整角色世界，不改源资产。用`tools/tests/fixtures/inertialization.request.json`及`python tools/tests/test_inertialization_probe.py <output.json>`核对正常衰减、短历史时间、微小缩放、无请求与同帧最短时长边界。Standing过滤节点的选择及缺失/大小写曲线控制使用`inertialization_filter.request.json`与`test_inertialization_filter_probe.py`。

惯性请求可增加`bone_names`，必须完整匹配当前源骨架的名称及顺序；此时每步以同长度`bone_transforms`数组提供全部局部变换，替代根骨骼字段，通道格式仍为厘米平移、xyzw旋转和无单位scale。工具逐项映射到compact pose索引，并输出`bone_names`、`parent_indices`及每帧`bone_transforms`；旧根骨骼`transform`仍保留。缺骨、错名、混用输入格式均拒绝，不补造通道。`inertialization_full_pose.request.json`和`test_inertialization_full_pose_probe.py`验证79骨骼映射与根/子骨骼平移、旋转和缩放。

`CharacterAnimationProbe`的实例记录包含`asset_players`：实际资产、节点GUID、当前时点、缓存Update权重及可查询序列的`pose_mode_at_observation`。BlendSpace另记录实际样本缓存的声明索引、源资产、夹取时点/权重及观察时mode。缓存权重不证明该pose已执行，观察时mode不证明此前求值时mode；使用`test_character_asset_player_probe.py`核对观察结构与非法值。

`CharacterAnimationProbe`同时读取初始化`movement_component`和每步`movement_before_tick` / `movement_after_tick`：实际CharacterMovement模式、角色LocomotionMode、注册/tick启用状态、当前及末次更新位置/速度和floor状态。上述观察只读移动组件；`test_character_movement_probe.py`检查前后连续性、字段类型/有限值与actor速度一致。注册或tick启用不代表该次tick已执行，AnimInstance的LocomotionMode也不能替代角色或移动组件状态。`initial_engine_frame`位于请求顶层；时钟控制必须核对每个case输出的同名实际回显。

角色case可选`local_controller: true`：读取器通过标准GameMode公共SpawnPlayerController建立本地controller，先完成actor初始化再Possess原角色，记录实际local/owner/authority。该case每步可设置`control_rotation_degrees: {pitch, yaw, roll}`、`desired_stance`（Als.Stance.Standing/Crouching）与`desired_gait`（Als.Gait.Walking/Running/Sprinting）；角度必须为有限数值，标签经原公开setter调用，随后正常world tick。`controller_before_tick`和`controller_input`保留实际控制、desired标签及View/Head只读状态；actual stance仍从原角色/AnimInstance读取。未请求controller时不创建这些对象或观察包；缺controller的控制请求、错误类型和未知标签拒绝。此能力不提供强制velocity，不等于旧受控运动样本；用`test_character_controller_probe.py`验证实际控制器与回显。

请求顶层可选`capture_player_view: true`在上述实际controller快照中增加`player_view`：直接调用const `GetPlayerViewPoint`，记录返回视点、manager/cache时点和旋转、实际viewtarget所属及actor旋转。该字段不设置相机、缓存或viewtarget，不代表调用了输入事件；缺flag时报告保持原形。`test_character_player_view_probe.py`对照cold actor回退和updated cache分支，并要求control与cold返回值存在可区分样本。

同一controller case每步可选`movement_input_world: {x, y, z}`，三个分量必须为有限JSON数值；读取器在world tick前调用原角色`AddMovementInput`，省略时不施加输入。方向为源世界空间的无单位输入，CMC自行约束、归一和生成加速度，不是速度请求。移动快照增加`pending_input_world`、`last_input_world`、`acceleration_cm_s2`与`max_acceleration_cm_s2`；pending允许在两次tick间由请求改变，其余快照保持连续。`test_character_movement_probe.py --input <result> --request <request>`核对请求、实际pending消费与last输入；加速度和实际速度另按原阶段核对，不用desired值代替。

`CharacterBrakingProbe`在临时原生ACharacter的有效Movement上调用UE原`ApplyVelocityBraking`，仅作受控数值核，不tick世界。请求`samples`每项包含`name`、double厘米速度`velocity_cm_s: {x,y,z}`及float参数`delta_seconds`、`friction`、`braking_deceleration_cm_s2`、`friction_factor`、`braking_substep_seconds`；全部必须为有限JSON数值，delta绝对值不超过0.25秒，其余负值由源方法自身处理。回显实际输入、前后速度、有效性和无root motion；`test_character_braking_probe.py`检查schema。纯核输出不代替自然角色运动或位置回算历史。

`CharacterVelocityProbe`复用同一原生组件与输入合同，在Walking模式调用UE原`CalcVelocity`（非fluid），额外要求double向量`acceleration_cm_s2`与float参数`max_speed_cm_s`、`max_acceleration_cm_s2`、`min_analog_speed_cm_s`、`analog_input_modifier`。输出实际getter值、加速度、mode和前后速度；`test_character_velocity_probe.py`检查schema及八个无效包反例。此核不tick世界、不移动角色，不替代ALS覆写的移动基座旋转或末端位置差分；静态基座下ALS `CalcVelocity`委托该Engine算法，可用于共同输入的pre-move速度控制。

角色请求可选`force_raw_data`布尔值：在world创建前，以带tag的`ECVF_SetByTemp`覆盖原生`a.ForceEvalRawData`，函数退出时`Unset`还原旧值/flags，输出`raw_data_control`记录施加阶段与恢复结果。true只在源raw模型有效时强制raw；false关闭强制，不保证compressed；省略时不写CVar。`test_character_raw_mode_control.py`核对控制与恢复，不能用`SetWithCurrentPriority`隐式替换Constructor优先级。

`SequencePoseProbe`可用`animation_pose: true`与`raw_additive_pose: true`读取有效加性sequence的原生raw delta；工具设置实际RequiredBones raw标志后调用GetAnimationPose，并输出`required_bones_raw`。可同时准备compressed数据以排除偶然raw回退，但不得混用`raw_absolute_pose`或绝对FK `capture_source_stages`。`test_raw_additive_pose_probe.py`检查模式、真实标志与完整骨序；绝对pose不能代替该delta。

`SequencePoseProbe`的`mesh_asset`仅用于`animation_pose`，选择与sequence共用Skeleton且包含完整骨骼的实际SkeletalMesh来初始化RequiredBones；不传时保持原Skeleton容器。输出`required_bones_mesh`读取实际骨容器，不能用请求字段充当执行证明。raw Sequencer会按reference-skeleton对象身份决定是否保留mesh参考变换，因此Skeleton容器的采样不能直接替代真实角色的mesh采样；`test_sequence_mesh_container.py`核对实际容器、时点与完整骨序。

`CharacterAnimationProbe`的`capture_pose_boundaries: true`在实际Rig对象构造完成时从所属mesh定位主实例，透明读取Rig输入与主Root结果的原求值，不追加查询。初始化和每个world步的`pose_boundaries`保留完整pose/curve、attribute数量、实际骨容器与求值时点；原生遍历计数从INDEX_NONE开始，首值0对应第一次求值，不能当作未执行。`test_character_pose_boundaries.py`验证完整观察和首query，默认字段关闭时保持原出口；Rig内部Hierarchy和初始mesh参考缓冲不能代替这些节点的真实输出。

同一观察还按编译Rig输入LinkID→linked class→该类Root前驱解析Head，读取真实`head_base`/`head_additive`、实际alpha和空间模式；实例类与属性名也来自实际对象，不按图名或GUID选择算法。需要资格Head两端输入时使用`test_character_pose_boundaries.py --head-inputs`；已有仅输入/Root记录仍可用默认检查，未实际求值的输入不能用独立采样补齐。

`base_provider_input`继续沿Head caller输入读取Layering的原Locomotion入口，`base_provider_class`对应实例的`layered_blends`记录实际总权重/相关性。动态输入链接会复制父link，观察必须在主实例构造完成、DynamicLink之前接入；Rig创建时再改父link可能已晚。使用`test_character_pose_boundaries.py --base-provider`验证该域；身体曲线为零不能证明LayeredBoneBlend整图直通，应检查节点的实际总权重和源执行分支。

`run_ue_source_reader.ps1 -BlendListProbe <request.json>` 运行原生FAnimNode_BlendListBase的Initialize/Update；请求提供`mode`、`blend_times`及命名`samples`，每步使用`active`子索引和`delta_seconds`。输出每帧权重、按现有权重缩短的时长、实际子Update权重及初始化次数。入口使用默认子更新策略，无BlendProfile，不执行完整AnimBP；默认不求值pose，`-Asset`用于读取器的常规资产记录。`blend_list.request.json`与`test_blend_list_probe.py`核对Cubic首帧、反转时长、子相关性与初始化。

sample可选`poses`数组，每个child一份`bone_transforms`（source厘米translation、source quaternion和scale，按当前AnimBP目标Skeleton完整骨序）及`curves`。此时`-Asset`必须为AnimBP，工具另执行CacheBones/Evaluate，逐帧输出`input_poses`和`pose`；该模式不与`two_way`、`cached_child`或`outer_two_way`组合。`test_blend_list_pose_probe.py`要求单个权重1输入的完整姿态和曲线直接传递，拒绝缺失或被修改的输出。

BlendList请求可选`two_way: {child, property}`，从当前AnimBP CDO复制指定Float TwoWay的配置，接入相应子项；每步提供`two_way_alpha`。可选`cached_child`把另一个子项接入原生SaveCachedPose，在PostGraphUpdate更新。输出额外记录TwoWay alpha、子初始化次数/更新权重，以及终端`update_order`（外层子索引，TwoWay两叶使用外层子数及其后一个索引）；未指定时原输出保持。`blend_list_two_way.request.json`与`test_blend_list_two_way_probe.py`覆盖gait离开/返回、父状态保留、子重新初始化和缓存延后顺序，不执行完整AnimBP或姿态。

已指定`cached_child`时，可用`outer_two_way: <CDO property>`在外层接入另一个Float TwoWay：A为BlendList，B为同一缓存，每步提供`outer_alpha`。输出`cached_requests`和实际缓存更新权重，用于核对双请求取最大值而非相加；初始化前推进原生proxy计数器，保持共享缓存只初始化一次。`blend_list_outer.request.json`与`test_blend_list_outer_probe.py`覆盖SprintBlock阻断、内部状态保持和共享缓存最大权重。

`run_ue_source_reader.ps1 -BlendProfileProbe <request.json>` 接受USkeleton，读取其实际WeightFactor Profile。每个sample提供`name`、`profile`、`mode`、正数`duration`、`deltas`、`curve_a`/`curve_b`以及`from`/`to`变换（`translation_cm`、xyzw `rotation`、`scale`）。通过公开FAlphaBlend、UBlendProfile、FBlendSampleData及FTransform/Curve调用，输出每帧全局alpha、FAlphaBlend累积值`blend_alpha`/`blend_value`、完成标志、缺省及各Profile条目的双权重和混合变换。它是标准转场计算路径的受控探针，未运行完整StateMachine；`complete=true`的计算结果不能当作仍被活动转场消费的姿态。使用`tools/tests/fixtures/blend_profile.request.json`与`python tools/tests/test_blend_profile_probe.py <output.json>`核对两个当前Profile的首帧、中间值和终点计算。 `mode: Custom`还必须提供`custom_curve`原生UCurveFloat路径；输出记录该路径及原生float精度的`custom_curve_keys`，避免六位小数反射文本掩盖键值差异。用`custom_blend.request.json`与`python tools/tests/test_custom_blend_probe.py <curve-native.json> <probe-native.json>`核对曲线采样、累计混合值和精确键值输出。

同一探针可用`mode: Linear`、`custom_curve: <UCurveVector路径>`和JSON数字整数`custom_curve_axis: 0|1|2`读取指定轴的原始RichCurve。`custom_curve_times`提供原时点采样；输出精确键值、切线、插值/切线/权重枚举、切线权重及外推枚举，并标记`custom_curve_applied_to_blend: false`，选轴曲线不参与FAlphaBlend。字符串、小数及越界axis均拒绝；旧Float Custom输出字段保持原样。可用`-Asset /ALS/ALS/Character/SK_Als.SK_Als -BlendProfileProbe tools/tests/fixtures/vector_curve.request.json -Output <output.json>`读取三轴，再运行`python tools/tests/test_vector_curve_probe.py --input <output.json> --request tools/tests/fixtures/vector_curve.request.json`与`--self-test`核对合同。

同一BlendProfile探针同时输出`two_way_effective_alpha=1−(1−alpha)`与原生FBlendedCurve::Lerp的`two_way_curve`，旧`curve`仍保留加权和结果。可选`weighted_inputs`包含1–8个`{weight, transform}`，weight必须是JSON数字且转成float后有限、正数并不超过1，transform使用相同源厘米/xyzw格式；同时提供`pose_mode: multi|two|gait`。探针用公开FAnimationRuntime API输出单骨骼`weighted_pose`：multi追加原节点末尾归一化，two要求两个输入并由API回算第二权重，gait复现无Profile的单姿态直通/双姿态分支。这是相同输入的算子控制，不负责运行原AnimBP、决定相关性或生成输入权重。

各Profile骨骼的`raw_transform`记录Overwrite/Accumulate之后、NormalizeRotation之前的原生FTransform，`transform`保持原归一化输出。该字段可核对多层FSM中的非单位中间四元数；输入仍须通过原IsNormalized合法性检查，不按检查容差自动归一化。

`run_ue_source_reader.ps1 -StateMachinePlayers` 接受AnimBP，输出编译后的FSM入口、状态reset标志、有序退出规则和PlayerNodeIndices/LayerNodeIndices。退出规则包含编译起止状态、逻辑、时长、blend/profile及重入/自动规则配置；节点索引映射到CDO属性和原图GUID。别名可能把同一原节点编译成多个实例：读取器校验索引与GUID反查均指向同一原节点，并记录`shared_origin_node_index`；不能要求GUID与编译索引一一对应，也不能把共享规则图所属节点当作所有引用转场的身份。Sequence Player额外输出资产长度/RateScale和公开getter取得的有效速率、起点、loop/ignore配置，原始反射字段仍在已有CDO记录中。该入口不执行动画图或模拟播放器时钟。用`python tools/tests/test_state_machine_players.py <output.json>`检查入口/边界索引、原图与共享实例身份、节点类型和有效配置结构。

每条编译退出的`transition_guid`/`transition_path`由FStateMachineDebugData按实际TransitionIndex反查原转场节点；`rule`继续记录规则节点。共享规则可被多个转场复用，不能用rule GUID推断转场身份。

`run_ue_source_reader.ps1 -StateMachineEntryProbe <request.json>`以原生FSM执行给定条件矩阵并用受控叶记录更新，不执行完整AnimBP pose。请求包含`machine_index`、`samples[].name/steps`；每步提供`delta_seconds`、完整`state_entries`和按编译TransitionIndex排列的`transition_rules`，共享delegate的输入必须一致。可选`update_machine`严格为bool、默认true；false仍推进proxy更新counter，跳过机器与叶更新，不能用delta0代替。原生重新相关初始化会清空pose links，探针会重新链接受控叶。输出`updated`、状态/elapsed/权重/叶调用及`active_entries`，后者记录source/target、alpha和原生`SourceTransitionIndices`；该indices用于活动栈身份，不以规则GUID或姿态值推断走过的边。

`StateMachinePlayers`另输出`asset_players`，按编译属性完整枚举FAnimNode_AssetPlayerBase派生节点，覆盖FSM列表之外的共享/直接播放器；每个节点的`synchronization`由原生getter读取组名、方法、角色、重入覆盖标志、当前资产及marker名称。SequenceEvaluator的覆盖标志继承基类false，不要求存在对应反射字段。`test_compiled_sync_players.py`交叉核对完整CDO播放器集合与配置；清单是静态配置证据，不能代替运行时同步验收。原machines及已有字段保持。

`run_ue_source_reader.ps1 -SequenceClockProbe <request.json>` 接受Sequence，以真实资产的TickAssetPlayer记录不使用marker同步的单leader时钟。sample提供`name`、初始TimeAccumulator `start`、`rate`和`deltas`，可选`looping`（默认false）；输出实际资产长度/RateScale以及逐帧position、previous、delta、有效历史标志和归一化比例。它不执行完整AnimBP或分组竞争。使用`tools/tests/fixtures/sequence_clock.request.json`及`python tools/tests/test_sequence_clock_probe.py <output.json>`核对零步长、正向、反向和末端夹紧。 循环控制使用`sequence_loop.request.json`与`test_sequence_loop_probe.py`，保留恰好到达末端时的位置及随后循环行为。

该请求可选`follower_asset`：以相同初始时间和速率创建第二个播放器，两播放器使用同一`looping`设置，调用原生TickContext的`ConvertToFollower`和该资产的`TickAssetPlayer`，输出独立位置、previous和实际delta。另记录两资产的marker数量，以及两个同分FAnimTickRecord经原生Sort后的输入索引顺序；这项排序观测不运行完整同步组或模拟加入/退出。使用`sequence_clock_follower.request.json`和`test_sequence_clock_follower_probe.py`核对无marker follower的归一化取时、零步长/夹紧及同权重排序；未提供该字段时原单leader输出保持。

`run_ue_source_reader.ps1 -SyncGroupClockProbe <request.json>` 通过UE公开FAnimSync执行单个同步组，成员为CanBeLeader。请求的`players`提供asset/start/rate及可选`looping`（默认false），`-Asset`须对应首项；每个sample提供name和steps，每步含delta_seconds及等长weights数组。权重为null表示该帧未更新，0表示实际提交零权重tick；两者保留不同的时钟/组历史。输出原生排序、leader、长度比例及每个播放器的位置/previous/delta。默认只采时钟，不执行完整AnimBP或世界。使用`sync_group_clock.request.json`和`test_sync_group_clock_probe.py`核对无marker空组、同分排序、成员加入/退出、零权重、零步长及夹紧；沿正常动画更新路径调用TickAssetPlayerInstances，该函数自行切换缓冲。

开启组pose捕获时可选`mesh_asset`，按该SkeletalMesh建立RequiredBones；mesh必须与全部player共享Skeleton并包含其骨名。输出`required_bones_mesh`及每个player的`pose_mesh`，`test_sync_group_sample_poses.check_mesh_contract`核对实际容器。省略时保持Skeleton容器与旧输出；Skeleton/mesh骨名一致不能证明参考姿态求值相同。关闭pose时不能请求mesh，且该能力不代表完整角色AnimBP反馈。

有marker的Sequence另输出原生精确marker名称/时刻，以及组的`marker_start`和`marker_end`（previous/next/alpha/valid）。Proxy持有实际AnimInstance及SkeletalMeshComponent owner，使跨marker时的原生trace路径可执行。使用`sync_marker_group.request.json`和`test_sync_marker_group_probe.py`核对六个循环Crouch Walk播放器的跨marker区间及follower位置；两套自测共同覆盖有/无marker入口。

组成员也可使用BlendSpace：该player必须提供固定的`blend_position: [x, y]`，`start`为0–1的归一化累计值。每个BlendSpace使用独立过滤器和样本缓存，沿原生TickRecord及FAnimSync推进；输出`time_unit: "normalized"`，其position/previous为归一化时间，playback_delta仍是实际DeltaTimeRecord的秒数。每帧另给出filtered_position和内部samples的权重、previous_time/time（秒）、sample_index、animation及play_rate。Sequence原输出保持；`reset`目前只接受Sequence索引，不把BlendSpace重建混同为重置累计值。`sync_blendspace_group.request.json`和`test_sync_blendspace_group_probe.py`覆盖单成员、不同轴输入的leader/follower、循环、样本时钟及单位。

`sync_blendspace_reentry.request.json`与`test_sync_blendspace_reentry_probe.py`核对动态轴下整组重入：最高权重样本切换后，原生用播放器保留的归一化时间重建当前样本，不沿用这个样本上次自己的秒数。fixture中的源duration用于精确float复算，空档帧必须没有更新成员。

`sync_blendspace_zero.request.json`与`test_sync_blendspace_zero_probe.py`核对BlendSpace的冷/热零步长：冷leader可建立有效marker，零delta follower保留默认样本；已有记录的近零leader不刷新样本，组内继续尝试候选。该合同与Sequence零步长不同，包含1e−8秒控制，不能用单一的零delta相位规则代替。

同步组step可选`blend_positions`，数组长度须等于players；BlendSpace项为当步`[x,y]`，null或省略整个字段均使用player的固定blend_position，不沿用上一帧覆盖值。Sequence项只接受null。过滤器和样本历史仍由每个播放器独立保存；输出结构保持。`sync_blendspace_dynamic.request.json`与`test_sync_blendspace_dynamic_probe.py`核对独立动态输入、过滤过渡、回退默认输入及同步时钟。

同步组step可选`reset`索引数组，按Sequence Player初始化语义把对应时间恢复为start并清marker记录；`inertial: true`为该步tick设置实际惯性请求标志。入口仍由原生组决定是否按前leader分数重同步，不手工改组相位。自测覆盖无请求重置、满足分数条件的惯性重同步，以及新leader更强时保留本地起点；不提供这两个字段时旧输出保持。

通用属性记录与Rig VM浮点数组元素对有限浮点额外给出`numeric_value`，直接读取原生FNumericProperty数值；原有`exported_text`及其他字段保持。文本往往只有六位小数，不能据此猜测源值是分数或将其替换为更“精确”的常量。`test_numeric_property_capture.py`验证CDO原生float、Rig VM默认内存与持续执行历史的浮点数组；`--self-test`验证最小有效/缺失/错误类型记录。非有限值仍由原文本记录，不写非法JSON数值。

2026-09-26 删除了 UE MCP client 及其完整调用链（输入审计、旧相机导出、拓扑/截图和五个 shell 探测脚本），当前取证统一使用原生 reader。相机曲线的目标转换使用 `import/import_curve_float.py`、`import/import_debug_curves.py` 和 `import/camera_curve_conversion.py`。

同批删除六个早期 Interchange API 试验脚本、没有消费者的 `probe_scene2.py`，以及反复改写旧 Playground parity 文件的 `patch_parity_ron.py`、`test_lighting_only.py`。实际导出器、原始资产、生成图、归档证据和本地配置保留。

`RigNodeHistoryProbe` 可选 `unit_boundary: {function, items}` 按原生函数与输入参数名选择 typed FRigElementKey 边界；每个有效 item 的 `current_local_dirty_before_global_read` 是原单元执行后、既有 current global getter 之前的原生 local dirty 布尔值。该值来自公开 const dirty-state getter；读取不触发 local 重算，可用于区分最终输出的 local 重算与 cache 保留。

`RigNodeHistoryProbe` 的 frame 可选 `mesh_to_world` 使用 `translation_cm[3]`、`rotation[4]`、`scale[3]`；提供时在同一 mesh 的本帧 Node.Update/Evaluate 前通过公开 SetComponentToWorld 设置缓存，保留原 quaternion 数值，不经过普通 SetWorldTransform 的 Rotator 往返，省略时保持当前变换（旧请求始终为identity）。提供字段的帧导出实际 `mesh_to_world`，执行事件另导出 `rig_to_world`，必须同时核对组件与真正的执行上下文，不能只检查请求值或pose。`tools/tests/test_rig_node_world_transform.py` 保留缺字段、未更新组件与未更新上下文的反例；无字段请求的旧输出合同保持。

`GeometryQueryProbe` 的可选 `move_input: {yaw_degrees, axes: [x,y]}` 按已核实的源输入算法，调用 Core FMath/FVector API，读取2D clamp、float角度/弧度与SinCos方向、double世界输入和CMC的`GetClampedToMaxSize(1)`阶段。它不调用角色输入事件、不创建或驱动角色，也不模拟相机选择；yaw与axes必须为有限JSON数字，yaw转float后也须有限。输出只在请求含该字段时新增，旧样本保持原样；用`tests/test_movement_input_math_probe.py --input <output.json>`或`--self-test`校验数值合同。

`GeometryQueryProbe` 的可选 `foot_lock_rotation` 数值样本消费 `rotation_xyzw`、目标 `target_rotation_xyzw` 和 `[0,180]` 的 `limit_degrees`。它按源脚锁末段保留 double quaternion 差、FQuat4f twist/角度/轴角与最终 double 乘法，不额外归一化。`tests/test_foot_lock_rotation_probe.py` 校验必需输出和非法 schema；旧样本不含该字段时不进入新增分支。
