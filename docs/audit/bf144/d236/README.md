# D236 无效 Feet 的交叉值保留

原 `RefreshFeetOnGameThread` 在 pending 时清除有效位，`RefreshFeet` 在无效时提前返回。Standing Movement T16 的两个源输入来自 `FeetState.FeetCrossingAmount` 和 `GroundedState.HipsDirectionLockAmount`，分别比较 ≤0 与 ≤−0.5。

D157 提交 `185401dc` 已将生成图的两项 Feet 曲线切到有效状态中保留的值。足部更新先于 provider，pending 路径在采样曲线前退出。新鲜原生 T16 及目标 RON 核对两条阈值和输入，12步历史重放覆盖0→0.8及0.8→0两种方向。撤回保留值会使两个无效帧的规则结果翻转。扩展加载角色的既有 fixture，使其验证上述两种 FeetCrossing 的发布。Rust fixture 随统一验证执行。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/Stances/AB_Als_Standing -Output docs/audit/bf144/d236/standing.native.json
python docs/audit/bf144/d236/replay_feet_crossing.py
```
