# D154 恢复有效脚部时保留锁定淡出

`AlsAnimationInstance.cpp:1252–1274` 更新 Feet 有效性。`1360–1392` 在恢复有效且旧锁定相关时用当前目标重设锚点并重算基座空间。`1437–1501` 允许部分权重下降、禁止部分淡入，权重到 1 时允许建立新锁。`1515–1523` 以当前权重生成最终位置，`GetControlRigInput:252–255` 将其传入原生 RigInput。

旧恢复入口先归零，导致 0.8→0.5 变为 0→0.5 并失去锁定。删除提前重置，沿用 `refresh_foot_lock` 的源重基准和淡出。以原生 AB_Als 的 [RigInput 绑定](../d149/ab-als.native.json) 与 [FootLock 默认值](../d147/settings.native.json) 验证十二组静态重放。重新插入旧清零条件时，0.8→0.5 两帧的权重失败，第二帧移动 8 cm 后 RigInput 多出 4 cm。

```sh
python docs/audit/bf144/d154/replay_revalidation.py
```

`revalidated_feet_keep_the_fading_lock_in_the_actual_rig` 从运行中的 CR_Als 读取 RigInput，覆盖有效性恢复与下一帧锚点保持。Rust、消融恢复和 Playground Gate 待统一验证。
