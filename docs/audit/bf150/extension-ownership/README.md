# BF150.03 扩展文件归属复核

以 `49673eac` 为起点逐文件检查 92 个 `src/*_ext` 文件，结合实现入口、当前调用方、本地 UE/ALS 源和已锁定依赖辨认职责。变更保留公开 Rust 路径和 RON 的反射名。

- `CR_Als` 的原生程序有 165 条指令，Construction 入口为 150，Forwards Solve 入口为 0，Rig 层级有 79 个骨骼。三个回归使用同一程序、CDO 层级和历史输入验证 ALS 自定义单位，现归入 `control_rig_units.rs` 的 `tests`。
- `B_Als_Character` 把角色 mesh 指向 `SKM_Als`。该网格有 15 个材质槽，索引 10 为 `Eyes_MAT` 并使用 `M_Als_Main`。角色槽接线按源资产归 ALS。可见的接收器绑定和贴花着色继续由 Bevy 扩展管理。
- `UAlsAnimationInstance::RefreshVelocityBlend` 计算方向权重，现归动画实例。空中两墙调整和斜坡限幅对应通用 `UCharacterMovementComponent`，与胶囊事务一起留在 Tnua 扩展。
- `FAlsAnimNode_GameplayTagsBlend::GetActiveChildIndex` 以合法 tag 选带一位偏移的子项，两个独立节点文件归 ALS。基础图的 TwoWay、Bool 与共享求值仍归动画图扩展。
- `UPhysicalMaterial::SurfaceType` 和 `UGameplayStatics::GetSurfaceType` 对应碰撞命中的表面元数据。`PhysicalSurface` 归 `SceneQuery` 模块，Footprint 使用同一 ECS 组件。

[原生 Rig](rig.native.json.gz) · [角色蓝图](character.native.json.gz) · [骨骼网格](mesh.native.json.gz)

## `animation_graph_ext` 起始 71 个文件

| 文件 | 复核后职责与源边界 |
|---|---|
| `als_curves.rs` | 图所消费的 GLTF 曲线扩展、目标曲线名和 `AlsAnimCurves` 资产加载 |
| `animation_context.rs` | 图更新与求值上下文、linked pose 入口、惯性化请求及缓存更新路径，对应 `FAnimInstanceProxy` 的运行阶段 |
| `blend_space_1d.rs` | 用位置选取相邻样本，传时间并混合 Pose，对应 BlendSpace 的一维采样 |
| `control_rig_program_tests.rs` | 删除该独立测试文件，三个用例迁入 `control_rig_ext/control_rig_units.rs` 的现有测试模块 |
| `curve.rs` | 通用标量曲线资产与步进、线性采样 |
| `curve_bundle.rs` | 图的命名曲线取样、加权、覆盖与合成，属于 Pose 曲线通路 |
| `curve_weighted_samples.rs` | 从图输入取加权样本并组装曲线束 |
| `direct_transition_event.rs` | 一次消费直接转场的图事件边 |
| `distance_match.rs` | 按剩余距离反查曲线时间，对应 `AnimDistanceMatchingLibrary` 的序列采样 |
| `event_lifecycle.rs` | 通知时钟、区间边界、正反向越界及 Begin/Tick/End 事件 |
| `event_loop.rs` | 图内循环时间和循环边界上的事件输出 |
| `event_queue_blend.rs` | 按 Pose 权重合成通知队列 |
| `f32_to_vec2.rs` | 图节点的两个标量输入到 BlendSpace 坐标转换 |
| `f_interp.rs` | `FInterpTo` 数值更新及逐节点历史 |
| `first_update.rs` | 图首更新锁存与首次信号抑制 |
| `gameplay_tag_blend.rs` | 迁入 `als/nodes`，独立节点按 `FAlsAnimNode_GameplayTagsBlend` 选子项并使用共享 BlendList 权重 |
| `gameplay_tag_layer_blend.rs` | 迁入 `als/nodes`，在 ALS 标签选择接线中组合 Pose、曲线和信号 |
| `global_bone_mask_input.rs` | 图全局骨骼遮罩输入与根默认值 |
| `global_bool_input.rs` | 图全局布尔输入与根默认值 |
| `global_f32_input.rs` | 图全局标量输入和 `ROOT_DEFAULTS_ENABLED_KEY` |
| `global_pose_input.rs` | 外部 Pose 到内层图 pin 的输入节点 |
| `inertialization.rs` | 惯性混合的标量节点和状态 |
| `inertialization_passthrough.rs` | `AnimNode_Inertialization` 的 Pose 历史、差值及请求处理 |
| `inertialized_linked_anim_layer.rs` | `AnimNode_LinkedAnimLayer` 的切换与惯性化请求边界 |
| `interruptible_fsm.rs` | 状态机转场、alias、权重与中断时序，承载 `AnimNode_StateMachine` 合同 |
| `layered_graph.rs` | 嵌套图的缓存、输出 alias、曲线和固定 pin 接线 |
| `loader.rs` | 图标量曲线资产类型的 Bevy loader |
| `local_ref_pose.rs` | 参考局部 Pose 及显式时间端口 |
| `mesh_space_additive.rs` | `AnimNode_ApplyMeshSpaceAdditive` 对应的 mesh-space 加性叠加 |
| `mesh_space_difference.rs` | 基准 Pose 与目标 Pose 的 mesh-space 差值 |
| `mesh_space_layered_blend.rs` | `AnimNode_LayeredBoneBlend` 的骨骼层混合和旋转空间 |
| `multi_way_blend.rs` | 依图 pin 权重混合任意数量的 Pose |
| `nodes.rs` | 常量、曲线求值和曲线驱动标量等图基础节点 |
| `normalized_blendspace.rs` | BlendSpace 归一化、坐标滤波、每骨骼插值及根运动混合 |
| `physics_asset.rs` | `PhysicsAsset` 的骨骼体、形状、约束、回读资格及交接装配 |
| `pose_timestamp.rs` | Pose 通过图边时的采样 timestamp |
| `post_physics_animation.rs` | 对应 `PhysAnim` 的物理后图求值和 Pose 回写；ALS 调用方提供实例上下文 |
| `profile_blend.rs` | `BlendProfile`、`AlphaBlend` 的曲线和每骨骼权重 |
| `ragdoll_angular_drive.rs` | 把图内当前骨骼姿态送到物理约束目标，对应 `UpdateRBJointMotors` |
| `ragdoll_config_node.rs` | 图节点的 dynamic/readback 配置输出 |
| `raw_curve_sampling.rs` | 原始动画序列的曲线时间和插值数据读入 |
| `raw_sequence_sampling.rs` | 原始序列轨道时间与旋转插值 |
| `reference_local_difference.rs` | 图内参照 Pose 的局部加性差值 |
| `reference_mesh_difference.rs` | 以参考和目标 pin 计算 mesh-space 加性差值 |
| `reset_on_activation.rs` | 根据图输入激活状态重置上游时间 |
| `rich_curve.rs` | `FRichCurve` 权重切线、Bezier 和 Hermite 取样 |
| `root_graph_boundaries.rs` | 图内 Control Rig Pose、曲线及执行事件接线，角色持有场景查询上下文 |
| `root_motion_clip.rs` | 动画 clip 根轨采样及其根运动/通知输出 |
| `root_motion_defaults.rs` | 扩展插件按图和 skeleton 为 clip 资产解析默认根骨 |
| `skeleton_aware_layered_graph.rs` | 以骨骼绑定对齐嵌套 Pose、Slot 与曲线 |
| `slot_node.rs` | 图内 Pose 进入 `AnimNode_Slot` 对应混合端口 |
| `source_animation_state_getter.rs` | 读取 `AnimInstanceProxy` 所有的状态机、相关资产和时间 |
| `source_cached_pose.rs` | `AnimNode_SaveCachedPose`／`UseCachedPose` 的本帧缓存和回写顺序 |
| `source_compression.rs` | AnimationCompression 的目标序列、曲线和解码时间输入 |
| `source_gameplay_tag_input.rs` | 图求值使用的 tag、槽、linked 输入和借用式 Rig 上下文接线 |
| `source_instance.rs` | 图实例属性、linked owner 与当前机器/状态权重 |
| `source_linked_input_pose.rs` | `AnimNode_LinkedInputPose` 的 Pose 输入上下文 |
| `source_per_track.rs` | `AnimEncoding_PerTrackCompression` 轨道及采样转接 |
| `source_pose.rs` | 图内 Pose 权重叠加、乘法与源骨骼量累积 |
| `source_pose_nodes.rs` | 共用 TwoWay、Bool 与 BlendList 求值、状态和缓存；ALS tag 的选择与权重调用 `als/nodes` 接口 |
| `source_pose_passthrough.rs` | 图 Pose/曲线透传及 source sync 播放器顺序 |
| `source_scalar_expression.rs` | 图内标量/布尔表达式以及类型化 pin |
| `source_sequence_evaluator.rs` | `AnimNode_SequenceEvaluator` 的帧与时间端口及 Pose 采样 |
| `source_sequence_player.rs` | `AnimNode_SequencePlayer` 的 clip 采样、虚拟骨骼和曲线 |
| `source_slot.rs` | Slot 实际消费的 Montage 栈、相加模式与通知资格 |
| `source_stop_nodes.rs` | 序列求值、枚举 BlendList 和 `AnimNode_LayeredBoneBlend` 等共用节点 |
| `state_label_event.rs` | 图状态标签到事件的适配 |
| `static_animated_scene.rs` | Animation Graph 所用的静态动画场景资产 loader |
| `static_skeleton.rs` | Animation Graph 所用的静态骨骼资产 loader |
| `sync_group_phase.rs` | 图内源 SyncGroup 同步、标记和播放相位 |
| `time_position.rs` | 图 Pose 的绝对时间、循环及其事件端口 |

## 其余 21 个起始文件

| 文件 | 复核后职责与源边界 |
|---|---|
| `avian_ext/angular_impulse_threshold.rs` | Chaos 约束角冲量阈值的目标标量 |
| `avian_ext/cooked_mesh_raycast.rs` | Chaos 烘焙三角网格的面顺序、方向、精度与射线内核；同一原生回归还测角色查询消费 |
| `avian_ext/joint_degenerate_rotation_limit.rs` | Chaos 约束退化角度的目标标量 |
| `avian_ext/scene_query.rs` | `UWorld` 几何查询、碰撞查询资格和 `UPhysicalMaterial::SurfaceType` 的 ECS 元数据 |
| `avian_ext/shape_raycast.rs` | 碰撞形状射线分支、三角回退和复杂网格精度 |
| `avian_ext/sleep_force_parameters.rs` | Chaos 休眠前外力条件的目标输入 |
| `avian_ext/sleep_velocity_tolerance.rs` | Chaos 休眠速度唤醒容差的目标输入 |
| `avian_ext/source_body_dynamics.rs` | Chaos 刚体休眠、转动积分与 `MassConditioning` 所需物理补足 |
| `avian_ext/source_capsule_sweep.rs` | Chaos GJK 胶囊扫掠在 Avian 场景查询中的形状补足 |
| `avian_ext/source_physics_joint.rs` | Chaos 双体局部帧、角/线性限制、驱动及缓存冲量的约束能力 |
| `bevy_ext/overlay_materials.rs` | 迁入 `als/character`，按 `SKM_Als`、相机及道具实例的源槽设置材质和接收资格 |
| `bevy_ext/projected_decal.rs` | Bevy 的打包投射贴花 GPU 接收、执行和读回；旧表面公开路径转发 `SceneQuery` 的类型 |
| `bevy_ext/source_gaussian_bloom.rs` | 对应 UE Gaussian Bloom 级联采样的 Bevy 渲染图接线 |
| `bevy_ext/source_gaussian_bloom.wesl` | 同一 Gaussian Bloom 的滤波 shader |
| `bevy_ext/source_world_time.rs` | `WorldSettings` 时间界限的虚拟世界 tick 适配 |
| `bevy_ext/text_overflow.rs` | Slate/UMG 式文本省略与当前 UI 消费 |
| `bevy_ext/time_dilation.rs` | actor/world dilation 下的未缩放 delta |
| `control_rig_ext/control_rig_units.rs` | `CR_Als` 对应的 `FAlsRigUnit_*` 执行和测试，接收三个程序/lifecycle 测试 |
| `tnua_ext/README.md` | 说明通用 CMC 辅助与 ALS 动画实例、角色时钟的源码边界 |
| `tnua_ext/crouch_transaction.rs` | 通用 `Crouch`／`UnCrouch` 胶囊状态事务，策略和碰撞回调由角色提供 |
| `tnua_ext/motion.rs` | 通用 CMC 滑动、两墙调整和坡面限幅；四向动画速度与其三个测试迁入 `actual_motion.rs` |

## 验证

`verify.py` 核对起始 92 条表项、现行 88 个扩展文件、算法和断言等价、原生测试夹具路径、插件注册以及资产与依赖身份。把一个旧程序指令数断言从 165 改到 164 的受控输入可被拒绝。标签节点在新目录中继续读同一 `weights.replay.json`。 [对照结果](relocation.json)

129 项关联库测试通过。三个 Rig 用例迁移前后各通过一次，生命周期 authoring 声明使用三个精确的新测试路径。移动消费链的三个定向、三个图资产集成测试和两个 authoring 检查通过。 [Rig 新路径](rig-after.stdout.log.gz) [定向回归](focused.stdout.log.gz) [图资产回归](graph-assets.stdout.log.gz) [生命周期声明](authoring-lifecycle.json) [单位声明](authoring-units.json)

## 正式入口对照

用 `run_manual.ps1` 的同一 `target_game/play`、普通时钟和原预算比较。起点二进制与 BF150.02 的封存 SHA 一致。两边的图错误均为 0。

| 场景 | 起点与搬移后的情况 |
|---|---|
| startup | 完整验证都通过 |
| control_rig | 动作覆盖和渲染回执都通过。两边有最终左脚速率告警，图求值 p95 为 5.53 → 5.79 ms；搬移后一次普通时钟样本另记录右小腿和右脚的速率告警 |
| default_overlay | 两边都在初始动作窗超时，退出码及 12 个缺失覆盖项一致 |
| footprint | 两边都在初始动作窗超时，退出码及 17 个缺失覆盖项一致。两边均缺完整贴花回执，最大墙钟帧超过 75 ms |

逐场正式报告保留起点和搬移后的原判定。复核后按字节恢复搬移源码，三个 Rig 用例在同 target 再次通过，恢复构建 SHA 与搬移后取证一致。 [场景结果、二进制身份与报告路径](runtime.json) [恢复后的 Rig 测试](rig-restored.stdout.log.gz) [恢复二进制](restored.binary.sha256)

```sh
python3 docs/audit/bf150/extension-ownership/verify.py
```
