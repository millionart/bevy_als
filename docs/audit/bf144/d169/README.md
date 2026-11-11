# D169 槽总权重的源归一化区间

`AnimInstanceProxy.cpp:2016–2031` 在源 `InTotalNodeWeight` 严格大于 `1.f + ZERO_ANIMWEIGHT_THRESH` 时才除以总权重。`GetSlotWeight` 另将公开槽权重夹在1。Transition的加性组合使用原始总和作判定。Turn在预组合与曲线上下文共用分母，最终槽权重仍用于基础姿态合成。

原生 `SlotProbe` 可在同一时间点向真实槽提交多个受控 `FMontageEvaluationState`。Transition、Standing Turn及Crouching Turn各六组。0.6/0.400005与精确 float 上界0.6/0.40001均保留层权重，0.6/0.40002和0.9/0.8归一化。18组原生曲线、槽权重及Turn根scale通过源重放，旧分母在六组容差内输入产生差异。Rust fixture 验证实际桥接上下文、Turn姿态位移与scale。以D168原请求复跑默认SlotProbe，结果逐值相同。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AB_Als -SlotProbe docs/audit/bf144/d169/transition.request.json -Output docs/audit/bf144/d169/transition.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/Stances/AB_Als_Standing -SlotProbe docs/audit/bf144/d169/standing-turn.request.json -Output docs/audit/bf144/d169/standing-turn.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/Stances/AB_Als_Crouching -SlotProbe docs/audit/bf144/d169/crouching-turn.request.json -Output docs/audit/bf144/d169/crouching-turn.native.json
python docs/audit/bf144/d169/replay_slot_normalization.py
```
