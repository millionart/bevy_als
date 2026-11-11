# D179 Turn 与 Profile 的曲线累加资格

`AnimationRuntime.cpp:230–239` 以首层权重Override曲线，后续调用Accumulate。`AnimCurveTypes.h:817–829` 对后项要求权重严格大于 `1e-5`。资格使用当前累加权重。标准Profile的上一状态/下一状态遵循同一顺序，显式曲线输出按相同规则计算。

站蹲原生Slot各重采三帧输入和四组混合。原始1/1.1e−5/1全部具备求值资格，中项归一化后约5.5e−6，源曲线忽略该项。将该项置于首位会保留它，归一化门槛上侧及普通权重也通过。Rust fixture 核对实际Turn上下文的先后顺序及Profile的源曲线边界。已提交的姿态层、采样身份与时钟继续作为同一输入。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/Stances/AB_Als_Standing -SlotProbe docs/audit/bf144/d179/standing-turn.request.json -Output docs/audit/bf144/d179/standing-turn.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/Stances/AB_Als_Crouching -SlotProbe docs/audit/bf144/d179/crouching-turn.request.json -Output docs/audit/bf144/d179/crouching-turn.native.json
python docs/audit/bf144/d179/replay_curve_accumulation.py
```
