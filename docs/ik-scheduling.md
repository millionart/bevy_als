# IK 系统调度顺序（M7.6）

## 固定链路

```
FixedPostUpdate (physics + tnua)
  └─ 角色胶囊位移/落地状态
PostUpdate
  1. apply_animation_to_targets          (bevy_animation_graph, AnimationGraphSet::Final)
     └─ 动画图 pose → 骨骼实体局部 Transform
  2. foot_bone_map_system                (als_core::foot_ik, 首帧构建 thigh/calf/foot 映射)
  3. foot_ground_probe_system            (双脚 raycast，含斜面法线)
  4. foot_pin_request_system             (急停/原地转身 → AlsFootPinRequest)
  5. foot_ik_state_system                (grounded 门控 → AlsFootIkTargets)
  6. foot_ik_apply_system                (bevy_als::foot_ik_apply, 显式 after 1/3/4/5)
     └─ AlsFootIkRuntime 插值 → als_ext::ik 解析两骨求解 → 骨骼实体局部旋转
        + pelvis 垂直补偿
Render
  └─ Transform propagation 汇总最终骨骼世界变换
```

## 强制方式

- `foot_ik_apply_system` 以 `.after(apply_animation_to_targets)` / `.after(foot_ground_probe_system)`
  / `.after(foot_pin_request_system)` / `.after(foot_ik_state_system)` 显式声明，
  保证 IK 写入的局部旋转不会被当帧动画覆盖，且消费同帧探测结果。
- `AlsFootProbePlugin` 内部 chain：bone_map → probe → pin_request → state。

## 已知折衷

- 探测读取的 GlobalTransform 为上一帧传播结果（一帧滞后）。标准做法，
  行走速度下视觉误差 <1cm；M7.7 Gate 实测无穿地。
- pelvis 补偿作用于 pelvis 骨局部 Y（UE Manny pelvis 直立于角色轴）；
  ALS 原版为 component-space 位移，等价收敛。
- 手部 IK（M7.5 占位）复用同一求解器，接线随 Rifle overlay 落地（M8+），
  届时插入位置为 foot_ik_apply 之后、propagation 之前。
