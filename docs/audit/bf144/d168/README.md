# D168 Transition 与 Turn 的 Montage 求值资格

`AnimInstance.cpp:4648–4669` 只提交权重严格大于 `1e-5` 的实例。`AnimInstanceProxy.cpp:2105–2148` 合计已提交的槽权重，`SlotEvaluatePose:1897–1904` 在权重不相关时原样输出基础姿态和曲线。Transition与Turn在合成前应用同一门槛，Turn的曲线上下文与姿态使用相同合格实例。Turn仍可完成资源就绪，播放实例生命周期独立推进。

对实际Transition、Standing Turn及Crouching Turn各重采五帧。原生约2.998e−6及8.159e−6的实例未提交，三个槽均保留基础曲线0.999992及原始姿态；约1.082e−5及普通权重进入求值。把3e−6加性层误提交会让下一帧的过渡许可跨过阈值。精确1e−5、两个9e−6、门槛上侧及小权重与0.25混合进入Rust的实际桥接和槽测试。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AB_Als -SlotProbe docs/audit/bf144/d168/transition.request.json -Output docs/audit/bf144/d168/transition.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/Stances/AB_Als_Standing -SlotProbe docs/audit/bf144/d168/standing-turn.request.json -Output docs/audit/bf144/d168/standing-turn.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/Stances/AB_Als_Crouching -SlotProbe docs/audit/bf144/d168/crouching-turn.request.json -Output docs/audit/bf144/d168/crouching-turn.native.json
python docs/audit/bf144/d168/replay_slot_eligibility.py
```
