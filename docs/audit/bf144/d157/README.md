# D157 起跳脚选择读取 Feet 的有效曲线历史

`AlsAnimationInstance.cpp:1252–1274,1297–1305` 在 Feet 无效时保留上一值，有效时读取并钳制 FootPlanted 与 FeetCrossing。原生 [Locomotion 图](../d149/locomotion.native.json.gz) 的 Jump Entry Conduit 从父实例 FeetState 读取 FootPlanted。T0 的比较值至多为 0 时进入 Left Foot，T1 大于 0 时进入 Right Foot。

Provider 从先完成更新的 AlsFootLockState 读取两个字段，默认值为 0。八组静态重放中，以最新曲线替代 FeetState 只在两组无效且符号相反时改变起跳脚。两个有效组的相反符号控制保持按当前曲线选择。

```sh
python docs/audit/bf144/d157/replay_jump_foot_history.py
```

`jump_foot_selection_uses_the_effective_feet_curve_history` 通过正常输入触发 Tnua 起跳，核对实际 Jump States 第一帧与两条曲线输入。Rust、消融恢复和 Playground Gate 待统一验证。
