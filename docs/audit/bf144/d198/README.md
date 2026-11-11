# D198 极短正转场的状态权重

fresh Land Movement→Grounded 自动规则的触发为 −1、名义时长为 0.3 秒，使用 HermiteCubic。UE `AnimNode_StateMachine.cpp:127–155,1414–1447` 由正时长与 elapsed 计算 Alpha 并发布状态权重。目标状态权重复用姿态的时长进度。

原生四条时钟轨迹覆盖半个 float epsilon、epsilon、两倍 epsilon 和普通时长。前两个仍为正，零步长时源/目标权重为 1/0。恢复旧 epsilon 门槛会错六帧。Rust fixture 经实际状态权重发布并在下一帧读取源实例缓存。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/AB_Als_Locomotion -Output docs/audit/bf144/d198/locomotion.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/SK_Als -BlendProfileProbe docs/audit/bf144/d198/clock.request.json -Output docs/audit/bf144/d198/clock.native.json
python docs/audit/bf144/d198/replay_short_crossfade.py
```
