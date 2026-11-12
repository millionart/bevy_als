# D230 AnimationRelative 末尾阶段

fresh SK_Als 仅 `ik_foot_root`、`ik_foot_l` 和 `ik_foot_r` 使用 AnimationRelative。127个 ACL 序列的原 `GetRetargetTransforms` 与同一 Skeleton 参考一致。弓的24骨均使用 Animation 模式。三个 IK 骨也不在现有 raw 虚骨的祖先闭包中。

目标在源厘米和双精度中执行当前旋转乘原参考逆、再乘实际网格参考并归一化。平移与 scale 依同一原参考处理。Prepared 只写真实采样轨道，烘焙加性跳过此阶段。Raw 只处理从 FK 写入的骨骼，D227 保留的参考值沿用原路径。

共3871个 prepared 骨骼与原生 f64 精确，包括948个烘焙加性骨。撤掉尾阶段会留下26个 f32 反例。fresh raw 340个骨骼投影精确，尾阶段改变8个原生 f64 值。这组 raw 投影在该阶段保持相同。

```sh
python tools/import/compile_als_prepared_acl.py --ue-root "$UE_ROOT" --output assets/ALSRuntime/Animations/als_prepared_acl.bin --work target/d230
python docs/audit/bf144/d230/replay_animation_relative.py
```

Rust fixture 使用真实 Crouch 首帧的 IK 足骨，随统一 Cargo 验证执行。
