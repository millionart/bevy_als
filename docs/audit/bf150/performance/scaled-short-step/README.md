# BF150.02 两倍体型的原生短步长补证

用本地 UE 5.8.3 读取真实 `B_Als_Character` 的落地、Control Rig 输入及物理发布。正式 Playground 的两倍体型释放中心为 10.819999695 m，原生匹配请求使用该高度、45° 源朝向及相同的重力。两端的浮点动画步均为 6.9855419 ms，在释放后第 195 个物理步进入布娃娃。源观测保留真实 2× actor 和 mesh，碰撞体及约束使用 UE 自己的装配。 [匹配请求](matched.request.json) [原生报告](native-matched.json.gz) [正式目标原始连续帧](target-trace/)

双方使用相同重落地到轻落地选择，入场保持前帧 mesh 世界变换，运动学根骨停在原地，骨盆由物理发布。UE 两侧前臂的 Rig 输入速度为 40.52／41.91 m/s，目标修复前为 39.94／41.34 m/s。源两侧手为 79.44／76.91 m/s，目标为 78.44／76.08 m/s。此处使用真实网格比例和相邻发布帧。 [重算结果](comparison.json)

## 关节缩放的时点

UE `USkeletalMeshComponent::InitArticulated` 从组件世界比例初始化物理，`FConstraintInstance::CreateJoint_AssumesLocked` 以创建时的 `LastKnownScale` 设置连接器。`USkeletalMeshComponent::UpdateRigidBodyScaling` 更新已存在的关节时消费 mesh 的相对比例。由角色父层改变世界比例时，运动学同步以当前世界骨骼比例更新刚体形状，mesh 相对比例保持一倍。因此，父层从一倍改两倍的首个模拟帧，原生 19 个源关节仍使用一倍连接器。出生时即两倍的对照创建两倍连接器。

| UE 控制 | 创建比例 | 左脚父侧锚点 | 左／右脚首步 |
|---|---:|---:|---:|
| 已装配后父层放大 | 1× | −0.401967 m | 0.602／0.578 m |
| 父层放大，求解时关闭地板和自接触 | 1× | −0.401967 m | 0.648／0.652 m |
| 出生时即两倍 | 2× | −0.803934 m | 0.074／0.069 m |

旧目标把父层变化写入已存在的关节，实际左脚父侧锚点为 −0.803934 m。源的地板与自碰撞消融保留了大位移，出生时的创建比例控制改变了连接器和首步表现。目标修正将碰撞体世界比例、关节创建世界比例和已有约束的 mesh 相对比例分别应用。 [关节控制请求](constraints.request.json) [求解前后和关节原生读数](native-constraints.json.gz)

修复后的正式连续帧中，左脚父侧锚点为 −0.401967 m。目标双脚首步为 0.637／0.603 m，匹配释放高度的 UE 为 0.602／0.578 m。位置差异出现在各自求解后的身体，目标所采发布骨骼与对应身体的位置误差最高 0.020 mm。根身体位置未改变，骨盆继续消费物理输出。 [修复后连续帧](target-trace-after/) [重算结果](comparison.json)

首帧与次帧的原验收拒绝了有原生证据的入场收拢。新验收保留原始速率、位移与段长观测。在源初始限速刷新期间，只有实际保留的关节比例小于 mesh 比例，且涉及的骨骼均与动态刚体的位置和旋转相符时，才归因为物理回读。无匹配身体、离开入场期或比例相同的读数继续检验。尺度野值、肢体平面翻转、镜头和动作交接检查沿用原规则。 [原总判定与故障](target-trace-after/report.json.gz)

同一物理 root 上的父层缩放、两次直接 mesh 缩放和新建约束由 PhysicsAsset 定向覆盖。170 项相关定向通过。移除相对比例基准时第二次直接缩放稳定失败，恢复字节后通过。移除段长判定对父端刚体的要求时，验证器的反例失败。最终 55 项相关定向与 Python 自测通过。无探针的正式 `startup`、原短步长、普通时钟 `ragdoll` 和 `get_up` 全部通过原预算。 [正式验收与二进制身份](acceptance.json)

## 复跑

源采集使用仓库原生读取器。按 `native-matched.identity.json` 记录应用临时探针并传入 `matched.request.json`，控制请求见 `physics.request.json` 与 `constraints.request.json`。目标使用同 `target_game` 的 `play` 构建，经正式视觉验证脚本运行 `-Case ragdoll -FrameDeltaSeconds 0.006985542 -CaptureCount 0`。两份目标连续帧补丁以 `a5c425b7` 的视觉收集器为底，后一份搭配修复后的关节装配。两端的观察探针仅在相应诊断构建里使用。

```sh
python3 docs/audit/bf150/performance/scaled-short-step/compare.py
```
