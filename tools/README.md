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
| `tmp/` | 被 Git 忽略的临时导出、中间文件和 `saved/als_debug.ron` 调试偏好；删除偏好文件会恢复默认类别 |

`import/probe_anim_graph_nodes.py`、`probe_property_access.py`、`probe_gltf_curves.py` 接受显式资产参数，仍可检查原生属性或导出曲线。没有其他脚本调用一个 CLI，不等于它已经无用。

`run_ue_source_reader.ps1 -PhysicsContactProbe <request.json>` 的 capsule/trimesh sample 可选 `triangle_indices`，使用 cooked mesh 索引调用原生 `ConstructCapsuleTriangleOneShotManifold2`，输出合并前的 `triangle_contacts`。其中两个 `shape_point_*_cm` 都在胶囊形状空间，`world_point_*_cm` 与 `world_normal` 已转换到世界空间；组上的 `triangle_index` 是请求的 cooked face，未归并点的 `face_index` 保留原生−1。该选项不改变正常 `points` 或 `aggregate_contacts`；用 `python tools/tests/test_physics_triangle_contact_probe.py <output.json>` 核对输出。

接触 sample 在已有 `next` 时可选 `history`：`friction_ratios` 与首帧 manifold 点一一对应，`anchor_offset_0_cm` / `anchor_offset_1_cm` 是两侧形状空间的旧锚点偏移。读取器调用原生 `SetSolverResults` 和下一帧 `Activate`，输出 `saved_points` 与 `next_history`；这是受控保存/匹配探针，不运行 Chaos 动力仿真。使用 `tools/tests/fixtures/physics_contact_history.request.json` 和 `python tools/tests/test_physics_contact_history.py <output.json>` 核对8点保存边界、比例插值、匹配标志与世界锚点差。

`gjk_simplex` / `next_gjk_simplex` 分别保留两帧的GJK支持点和重心权重。冷重建使用另一个sample在相同当前姿态运行已有 `aggregate_mesh`，以独立的新约束作对照；`fresh_points` 仍是同一约束的重复重建，不代表冷查询。粒子的旋转按源 `PBDRigidParticles::SetQ` 保存为float32，旋转请求应采用同一精度。`physics_contact_gjk_history.request.json` 与 `test_physics_contact_gjk_history.py` 覆盖分离核心、EPA及无缓存路径；`project_initial_overlap_depenetration_velocity_cm_s` 来自项目的世界solver配置，不是PA中的RBAN配置。

`run_ue_source_reader.ps1 -InertializationProbe <request.json>` 接受AnimBP；有多个原生Inertialization节点时，用请求顶层的`node_property`选择原生CDO属性名，省略时必须只有一个节点。读取器复制该节点并通过公开Update/Evaluate入口运行受控单骨骼历史，输出也保留选中的属性名。每个sample提供`name`及`steps`；每步包含`delta_seconds`、源厘米单位的`translation_cm`、xyzw四元数`rotation`和`scale`，可选`curves`、单个`request_duration`或同帧数组`request_durations`。所有请求在该步唯一一次Update/Evaluate前排入队列。输出逐帧Transform、曲线及实际Enable/IgnoreVelocity/IgnoreDeficit；它不运行完整角色世界，不改源资产。用`tools/tests/fixtures/inertialization.request.json`及`python tools/tests/test_inertialization_probe.py <output.json>`核对正常衰减、短历史时间、微小缩放、无请求与同帧最短时长边界。Standing过滤节点的选择及缺失/大小写曲线控制使用`inertialization_filter.request.json`与`test_inertialization_filter_probe.py`。

`run_ue_source_reader.ps1 -BlendProfileProbe <request.json>` 接受USkeleton，读取其实际WeightFactor Profile。每个sample提供`name`、`profile`、非Custom的`mode`、正数`duration`、`deltas`、`curve_a`/`curve_b`以及`from`/`to`变换（`translation_cm`、xyzw `rotation`、`scale`）。通过公开FAlphaBlend、UBlendProfile、FBlendSampleData及FTransform/Curve调用，输出每帧全局alpha、FAlphaBlend累积值`blend_alpha`/`blend_value`、完成标志、缺省及各Profile条目的双权重和混合变换。它是标准转场计算路径的受控探针，未运行完整StateMachine；`complete=true`的计算结果不能当作仍被活动转场消费的姿态。使用`tools/tests/fixtures/blend_profile.request.json`与`python tools/tests/test_blend_profile_probe.py <output.json>`核对两个当前Profile的首帧、中间值和终点计算。

`run_ue_source_reader.ps1 -StateMachinePlayers` 接受AnimBP，输出编译后的FSM、状态及有序PlayerNodeIndices/LayerNodeIndices，并将节点索引映射到CDO属性和原图GUID。读取器校验GUID往返；Sequence Player额外输出资产长度/RateScale和公开getter取得的有效速率、起点、loop/ignore配置，原始反射字段仍在已有CDO记录中。该入口不执行动画图或模拟播放器时钟。用`python tools/tests/test_state_machine_players.py <output.json>`检查编译索引、原图身份、节点类型和有效配置结构。

`run_ue_source_reader.ps1 -SequenceClockProbe <request.json>` 接受Sequence，以真实资产的TickAssetPlayer记录无marker、非循环单leader时钟。sample提供`name`、初始TimeAccumulator `start`、`rate`和`deltas`；输出实际资产长度/RateScale以及逐帧position、previous、delta、有效历史标志和归一化比例。它不执行完整AnimBP或分组竞争。使用`tools/tests/fixtures/sequence_clock.request.json`及`python tools/tests/test_sequence_clock_probe.py <output.json>`核对零步长、正向、反向和末端夹紧。

该请求可选`follower_asset`：以相同初始时间和速率创建第二个非循环播放器，调用同一原生TickContext的`ConvertToFollower`和该资产的`TickAssetPlayer`，输出独立位置、previous和实际delta。另记录两资产的marker数量，以及两个同分FAnimTickRecord经原生Sort后的输入索引顺序；这项排序观测不运行完整同步组或模拟加入/退出。使用`sequence_clock_follower.request.json`和`test_sequence_clock_follower_probe.py`核对无marker follower的归一化取时、零步长/夹紧及同权重排序；未提供该字段时原单leader输出保持。

`run_ue_source_reader.ps1 -SyncGroupClockProbe <request.json>` 通过UE公开FAnimSync执行单个无marker同步组，成员固定为非循环CanBeLeader。请求的`players`提供asset/start/rate，`-Asset`须对应首项；每个sample提供name和steps，每步含delta_seconds及等长weights数组。权重为null表示该帧未更新，0表示实际提交零权重tick；两者保留不同的时钟/组历史。输出原生排序、leader、长度比例及每个播放器的位置/previous/delta。入口不执行完整AnimBP、姿态或世界。使用`sync_group_clock.request.json`和`test_sync_group_clock_probe.py`核对空组、同分排序、成员加入/退出、零权重、零步长及夹紧；沿正常动画更新路径调用TickAssetPlayerInstances，该函数自行切换缓冲。

同步组step可选`reset`索引数组，按Sequence Player初始化语义把对应时间恢复为start并清marker记录；`inertial: true`为该步tick设置实际惯性请求标志。入口仍由原生组决定是否按前leader分数重同步，不手工改组相位。自测覆盖无请求重置、满足分数条件的惯性重同步，以及新leader更强时保留本地起点；不提供这两个字段时旧输出保持。

2026-09-26 删除了 UE MCP client 及其完整调用链（输入审计、旧相机导出、拓扑/截图和五个 shell 探测脚本），当前取证统一使用原生 reader。相机曲线的目标转换使用 `import/import_curve_float.py`、`import/import_debug_curves.py` 和 `import/camera_curve_conversion.py`。

同批删除六个早期 Interchange API 试验脚本、没有消费者的 `probe_scene2.py`，以及反复改写旧 Playground parity 文件的 `patch_parity_ron.py`、`test_lighting_only.py`。实际导出器、原始资产、生成图、归档证据和本地配置保留。
