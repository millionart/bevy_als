# BF114 独立 source/target 合同

本轮事实来自本地 ALS `AlsCharacter_Actions.cpp`、`AlsAnimationInstance.cpp`、`AlsCharacter.cpp`，同轮 native SKM/Character/Settings/GetUp 导出，以及当前 animation-graph 与 Avian 的实际系统。前任务记录不构成本轮证明。

## 姿态与所有权

| 阶段 | Source | Target 合同与检查 |
|---|---|---|
| 正常动画 | SkeletalMesh 的动画骨骼驱动非模拟物理 body | 最终骨骼 GlobalTransform × bone-map offset 更新 kinematic proxy；由相邻最终姿态求进入时速度 |
| 进入 | Stop montages；Mesh KeepWorld detach；capsule collision off；ResetAllBodiesSimulatePhysics | 仅 Ragdolling 入口 detach player；保留已生成 proxy 姿态/速度；激活使用资产 body 默认模式，独立控制 readback |
| 模拟 | Actor 追随 pelvis；Mesh 不受 Actor 移动影响；更新物理动画 joint motor | player 为独立世界锚点；graph PrePhysics 给目标，physics 后 readback，Final 后传播骨骼 |
| 最终姿态 | SnapshotPose 读取当前骨骼局部变换 | 从实际骨骼 Transform 抓取一次 snapshot；不能把 graph Pose 的旧存储长度当有效骨骼数 |
| 退出 | snapshot → pelvis world → Actor 重定位 → Mesh 默认位置/旋转与父级 → snapshot pelvis local | 同一 Update 事务恢复 player；新骨盆 local 使用实际骨骼父级，保留骨盆世界姿态；下一次 graph evaluation 前完成 |
| 起身 | grounded 时按 facing-up 选择 Back，否则 Front；播放 PostLocomotion Montage 并锁输入 | 两分支使用真实 clip/curve/notify；GettingUp action 清除后释放输入，Montage 可继续淡出 |

SKM_Als 原生骨架是 68 raw + 11 virtual；已有 glTF 的 79 skin joints 不含 scene node 0，动画的 237 channels 也不含它。Skeleton loader 的 root 是 player/scene 容器，因此补齐 missing reference bones 时不能给它制造动画通道。显式源动画通道仍被保留。

`Pose::add_bone` 追加 storage 并替换 path index。有效通道、边界骨骼数及 translation extent 必须以 `paths` 为索引，不能遍历已经被覆盖的 storage。原先 104→81 的 storage 数变化不是骨骼丢失证明。

当前框架的 readback 已移除 parent bone 的 world transform；再次乘 player inverse 会重复变换。真实 API fixture 包含非零 Actor、旋转 avatar、旋转/平移的骨骼父级、正常移动、detach、Actor follow 和 exit snapshot。

Control Rig 与 Hand IK 在 builtin Transform propagation 之后写回，normal proxy 必须排在项目最终 `AlsControlRigPropagationSet` 之后。实际交接检查曾发现 13.246 mm / 0.180298 rad 的旧姿态差异；包含后置 Rig 的完整生产 plugin fixture 稳定复现，移动同步时点后 ragdoll-link 8 tests 通过。

## 坐标与朝向

骨骼局部基为 `B(x,y,z)=(x,z,y)`，长度 cm→m。Actor 的 source +X forward 对应 target −Z forward；世界方向基为 `Ry(π/2) B`。fresh Character CDO 的 Mesh 相对位置为 `(0,0,-92) cm`、yaw 为 `−90°`、scale 为 1；对应当前 avatar 的 Y `−0.92 m` 与 yaw `π`。

Source recovery 使用 pelvis Rotator roll≤0 判断 facing-up，Actor yaw 为 pelvis yaw（facing-up 时减 180°）。target 在非极点使用 pelvis X 轴作为 forward、`atan2(−pelvis_Z.y, pelvis_Y.y)` 作为 source roll；极点遵循当前 UE `FQuat::Rotator` 的 `0.4999995` 分支。24 个正反面/左右 yaw/俯仰极点组合及实际退出测试覆盖该映射。

## 已通过的最小闭环

- 正常角色错误 detach：实际系统测试先失败，加状态 guard 后通过。
- 缺失 reference channel 写入 scene root：实际 apply-animation 测试先失败，删除制造该通道的分支后通过；als-ext 318 tests。
- 正常物理代理漏骨骼旋转：实际 WorldAsset→AnimatedScene→Avian 流程先失败，final bone→proxy 后通过。移除 RigidBodyDisabled 分支后同一 Gate 仍通过，已删除该分支。
- Exit snapshot 的旧父空间与缺失 Actor yaw：同一真实 API fixture 分别先失败，事务修复后 ragdoll-link 7 tests 通过。移除额外的 pelvis Transform 写回仍通过，仅保留 snapshot 更新。
- Pose storage/path 混计：root 与 source-boundary 回归先失败，修复后 visual collector 31 tests 通过。
- reader watchdog：normal / no-progress / result self-test 通过。CDO subobjects 原导出缺失；新导出已读到实际 Mesh/Camera 默认值。

运行证据位于 ignored `target/bf114_takeover/`。以上是静态/实际 API Gate，不是完整 Playground 验收。

## PhysicsAsset 合同

公开 uasset 转换入口自动读取 PA 的 PreviewSkeletalMesh，验证 native reference 与现有 glTF，再生成独立 target ragdoll、bone-map 与 physics profile。当前 PA 为 21 bodies、22 shapes、20 joints、49 禁用碰撞对；CollisionDisableTable 的 key 存在即禁用，不能按其存储 bool 过滤。

约束直接使用两个 source RefFrame，不再使用初始动画帧校准，也不重复应用 AngularRotationOffset。SourcePhysicsJoint 复用 Avian point 约束及公开 XPBD；按源父/子与坐标轴执行限位、implicit acceleration drive、每关节质量/惯量 conditioning、child-only projection 和 projection velocity alpha。数据明确保存 active unsupported feature；转换器拒绝未实现语义。默认物理材质也来自 native effective material。

数学和实际 PhysicsSchedule fixture 覆盖质量变化、软/硬限位、旋转锚点、disabled 与 conditioning 消融。实际 PA spawn 验证 19 个启用关节、一个禁用 pelvis joint、原生角限位与全轴阻尼已移除；native 矩阵 Gate 的 164 个字段差异为零。

直接重新解码原生 `A_Als_Stand_Pose.uasset` 证明 calf 动画与 PA reference frame 本来就有约 0.195 mm 的长度差；转换误差小于 0.00002 mm。handoff Gate 因此验证全部 body 等于实际同帧骨骼乘 bone-map offset，位置与旋转误差分别不超过 0.1 mm / 0.0001 rad；reference anchor gap 仅作诊断，不校准 source frame。

## GetUp 与同槽动作

CS_Als_Default 启用初始限速：入口立即执行，并在后续 8 次 Refresh 限制 body 线速度为 `max(2 m/s, entry locomotion speed)`。离线设置转换器从 CS、两个 Montage 与本地 C++ 生成 target 参数。两个 GetUp 均为 1.5 s、RateScale 1.2、BlendIn 0、BlendOut 0.3 s；两条 sequence 均无 root motion。GettingUp notify 在 clip 1.166667 s 结束，此时淡出尚未完成。

Ragdoll Start 的 Montage_Stop(0.2) 要求同槽 Roll 停止驱动 Actor 并保留淡出。fresh Roll 默认值为 HermiteCubic；旧线性权重与 generated slot 重复乘 weight 均已纠正。Slot 使用完整曲线上下文；权重 1 的空曲线容器也必须清除 base curve 影响。GetUp 禁止移动、跳跃、手动与落地自动 Roll，保留视角与 ragdoll 输入；输入标记由实际 notify action 生命周期释放。

## 真实 Gate 与后续范围

独立转换已覆盖 9 份 AnimBP、155 个可达 source subgraph、256 个安装文件，source-visible/static Gates 与 startup 通过。2026-09-08真实Playground的未干预ragdoll和两个GetUp分支均为manual/validator exit0；分别72/66 pose samples，graph、fault和temporal anomaly均0。两个起身按实际clip 0.6±0.1s比较，canonical腿部姿态差2.0235rad；各自通知输入锁定/释放通过。

接下来按用户指定的 Standing、Grounded、Crouching、Locomotion、Layering、Head、CR_Als、AB_Als 顺序修复转换器和 Actor；旧手工图清理延期。Camera detached pivot 按本地 source 修复，用户反馈的视角和距离仍待对应 Actor 链取证。
