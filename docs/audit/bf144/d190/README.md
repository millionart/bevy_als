# D190 骨骼惯性化的源门槛

UE `AnimNode_Inertialization.cpp:875–900,927–950` 在幅度严格超过 `UE_KINDA_SMALL_NUMBER` 时保留方向，并在上一历史步长也严格超过该常量时计算速度。源平移用厘米，目标距离门槛为 1µm；scale 和历史时间分别保留 0.0001 和 100µs。

fresh 主 AB `AnimGraphNode_Inertialization` 九例覆盖平移低于、等于和高于门槛，scale 正反例，以及 50、100、100.1µs 和普通历史步长。原生 50µs 首帧保留约 0.994912mm，稳定 scale 1.00005 首帧回到 1。旧路径五例失配；移除平移门槛、scale 门槛、历史门槛分别使 2、1、2 例失配。Rust fixture 走实际惯性差值初始化与输出并核对原生读数。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AB_Als -InertializationProbe docs/audit/bf144/d190/inertia.request.json -Output docs/audit/bf144/d190/inertia.native.json
python docs/audit/bf144/d190/replay_inertial_thresholds.py
```
