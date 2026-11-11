# D200 跨状态姿态缓存的相关性

UE `AnimNode_StateMachine.cpp:688–738` 只检查已编译的 PlayerNodeIndices 及 LayerNodeIndices。fresh Locomotion 和 Standing 编译记录确认，Land Movement 有两个本地 Additive 播放器，两个 Start 和两个 Pivot 各有四个本地播放器，五个状态均无 layer 候选。当前生成图的 18 个播放器 GUID 与原生表匹配。

在状态中使用 scoped cache 时，目标按实际图上下文祖先判断缓存 producer 是否在该状态内。跨状态缓存读数只移除相关性候选，姿态、曲线、同步和请求仍从缓存交付。五条实际自动规则的受控回放在旧候选时钟下提前触发，恢复后使用本地时钟。Rust fixture 经已加载 scoped cache 检查状态内外的相关性，并核对姿态、曲线与请求。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/AB_Als_Locomotion -StateMachinePlayers -Output docs/audit/bf144/d200/locomotion.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/Stances/AB_Als_Standing -StateMachinePlayers -Output docs/audit/bf144/d200/standing.native.json
python docs/audit/bf144/d200/replay_state_relevancy.py
```
