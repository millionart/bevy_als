# D201 无 marker leader 的秒域时钟

UE `AnimSequenceBase.cpp:515–544` 先在秒域推进 leader，再发布 ratio。`AnimationRuntime.cpp:1562–1598` 只在越出时间域时循环或裁剪，恰好到末点仍保留该时刻。目标复用序列的时间域约束，直接保留 float 秒数。

fresh Land 与 Idle 的十条轨迹包含 30/60/120Hz、零步长、精确循环末点和反向循环。1196 帧的原生 position、previous 和 ratio 与秒域回放逐值匹配。旧 ratio 往返累计造成 844 个时点失配，Land 120Hz 的第 140 个正步长提前到达末点。Rust fixture 通过实际同步组 resolver 检查发布时钟。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Air/Land/A_Als_Land_Light -SequenceClockProbe docs/audit/bf144/d201/land.request.json -Output docs/audit/bf144/d201/land.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Base/A_Als_Idle -SequenceClockProbe docs/audit/bf144/d201/idle.request.json -Output docs/audit/bf144/d201/idle.native.json
python docs/audit/bf144/d201/replay_markerless_clock.py
```
