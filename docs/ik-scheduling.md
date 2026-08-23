# IK 系统调度顺序（M7.6，M7.7 修订）

## 固定链路

```
FixedPostUpdate (physics + tnua)
  └─ 角色胶囊位移/落地状态
PostUpdate
  0. AnimationGraphSet::Final.before(TransformSystems::Propagate)
     └─ 显式钉死 fork 的 apply_animation_to_targets 先于本帧变换传播
        （fork 仅在 physics_schedule 配置了该顺序，PostUpdate 内原本悬空）
  1. TransformSystems::Propagate
     └─ mark_dirty_trees → propagate_parent_transforms → sync_simple_transforms
        动画写出的骨骼局部 Transform → GlobalTransform 同步
  2. foot_bone_map_system                (als_core::foot_ik, 首帧构建 thigh/calf/foot 映射)
  3. foot_ground_probe_system            (双脚 raycast，含斜面法线；读同帧新鲜全局变换)
  4. foot_pin_request_system             (急停/原地转身 → AlsFootPinRequest)
  5. foot_ik_state_system                (grounded 门控 → AlsFootIkTargets)
  6. foot_ik_apply_system                (bevy_als::foot_ik_apply, 显式 after 1/3/4/5)
     └─ AlsFootIkRuntime 插值 → als_ext::ik 解析两骨求解 → 骨骼实体局部旋转
        + pelvis 垂直补偿
  7. 二次传播（bevy_transform 公开系统第二实例）
     └─ mark_dirty_trees → propagate_parent_transforms → sync_simple_transforms
        after 6 与 TransformSystems::Propagate；把 IK 写入的局部姿态同步到
        GlobalTransform，供渲染提取与下一帧 Update 侧读取
Render
  └─ 使用 7 的结果提取蒙皮关节
```

## 强制方式

- `AnimationGraphSet::Final`（fork apply 所在集合）经 `configure_sets(PostUpdate, …before(TransformSystems::Propagate))`
  补上 PostUpdate 内缺失的顺序约束，不改 fork。
- `AlsFootProbePlugin` 内部 chain：bone_map → probe → pin_request → state，整体 `.after(TransformSystems::Propagate)`。
- `foot_ik_apply_system` 以 `.after(apply_animation_to_targets / TransformSystems::Propagate /
  foot_ground_probe_system / foot_pin_request_system / foot_ik_state_system)` 显式声明。
- 二次传播复用 `bevy::transform::systems::{mark_dirty_trees, propagate_parent_transforms,
  sync_simple_transforms}` 公开系统的第二实例；第二实例的 change-tick 上界为本帧，
  可见动画+IK 全部写入，脏树标记与遍历均正确。

## 已知折衷

- pelvis 补偿在两腿求解之后写入，当帧腿部求解用的是补偿前的髋部位置；
  误差一帧内被下一轮求解吸收，收敛稳定。
- 探测与求解读到的 GlobalTransform 为同帧动画结果（步骤 1 已同步），无一帧滞后。
- 手部 IK（M7.5 占位）复用同一求解器，接线随 Rifle overlay 落地（M8+），
  届时插入位置为 foot_ik_apply 之后、二次传播之前。

## M7.7 实测教训

初版仅按函数引用排序（无 Propagate 约束），PostUpdate 内动画写入与传播顺序悬空：
探测读到上一帧过期全局变换，IK 输入恒近零，脚部从未贴地（pen≈+0.5 恒定）。
修正为上述显式管线后消除。任何新增消费骨骼全局变换的系统必须声明相对
`TransformSystems::Propagate` 与二次传播的先后关系。
