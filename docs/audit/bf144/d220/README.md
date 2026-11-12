# D220 使用实际状态更新权重提交同步记录

UE `AnimNode_StateMachine.cpp:904–914,1414–1453,1577–1586` 先累计所有活动转场对状态的贡献，再以累计权重执行一次状态 Update。短反转时同一个状态可同时承接较早转场的剩余权重和当前目标权重。

fresh Grounded 双向短反转共 168 帧。40 帧的实际状态更新权重高于两条姿态路径的最大值，最大缺失为 0.3059685528。38 帧无旧混合的控制与单次更新合同一致。

目标在状态实际求值时收集同步记录，使用清栈前的累计状态权重。共享缓存请求仍按原合并规则取最大值。Rust fixture 通过实际中断 FSM、ProfiledBlend 和两条 Sequence 比较每个状态的同步记录。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/AB_Als_Grounded -StateMachinePlayers -StateMachineEntryProbe docs/audit/bf144/d220/weights.request.json -Output docs/audit/bf144/d220/weights.native.json
python docs/audit/bf144/d220/replay_state_weights.py
```
