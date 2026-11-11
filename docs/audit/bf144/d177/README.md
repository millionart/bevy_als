# D177 Turn 和 Profile 的加权旋转

`AnimInstanceProxy.cpp:2056–2085` 调用 `BlendPosesTogetherIndirect`，`AnimationRuntime.cpp:458–484` 按层乘权重、对齐最短符号、累加并在最终归一化。`AnimNode_StateMachine.cpp:1064–1111,1024` 对标准Profile转场采用相同顺序。Turn预混沿D123b的原始累加，FSM中间姿态沿D123c保留原始长度，独立Profile输出现在也使用加权累加和最终归一化。

`SlotProbe` 在真实站蹲Turn槽提交不同clip位置的实例，使用0.25与0.8秒、0.3/0.7与0.7/0.3、不等及相等输入。632个原生骨旋转与源公式对齐，旧slerp在102个旋转超过对照误差，最大约0.343°。Rust fixture 使用两站姿实际骨旋转核对Turn预混和Profile原始/最终阶段。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/Stances/AB_Als_Standing -SlotProbe docs/audit/bf144/d177/standing-turn.request.json -Output docs/audit/bf144/d177/standing-turn.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/Stances/AB_Als_Crouching -SlotProbe docs/audit/bf144/d177/crouching-turn.request.json -Output docs/audit/bf144/d177/crouching-turn.native.json
python docs/audit/bf144/d177/replay_weighted_rotations.py
```
