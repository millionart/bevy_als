# D203 自动规则使用上次状态更新权重

UE `AnimNode_AssetPlayerBase.cpp:27–33` 缓存播放器最终更新权重。`AnimNodeBase.h:281–287` 将机器权重和状态权重相乘。`AnimNode_StateMachine.cpp:688–709,820–850,1230–1235` 清除进入状态的旧缓存权重，在更新当前帧状态之前用严格正权重挑选播放器。

原生 Locomotion FSM 提供进入 Land Movement 的状态更新权重，随后真实 Land 同步组提供两个序列的前帧时钟。将两者回接到已编译的自动规则后，0.875 和 1.0 秒起点的零步长入口均等到正权重更新后退出。将候选权重替换为状态内局部权重会使两条路径提前一帧。较早时钟及两种正步长入口提供对照。

目标自动规则和转场时长计算读取同一个状态内候选，按上次记录的机器和状态更新权重缩放。Rust fixture 通过实际 FSM 与序列缓存覆盖当前权重为正时前次机器或状态权重为零的情况。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/AB_Als_Locomotion -StateMachinePlayers -StateMachineEntryProbe docs/audit/bf144/d203/weights.request.json -Output docs/audit/bf144/d203/weights.native.json
python docs/audit/bf144/d203/replay_zero_weight.py clock-request
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Air/Land/A_Als_Land_Light -SyncGroupClockProbe docs/audit/bf144/d203/clock.request.json -Output docs/audit/bf144/d203/clock.native.json
python docs/audit/bf144/d203/replay_zero_weight.py transition-request
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/AB_Als_Locomotion -StateMachineEntryProbe docs/audit/bf144/d203/transition.request.json -Output docs/audit/bf144/d203/transition.native.json
python docs/audit/bf144/d203/replay_zero_weight.py check
```
