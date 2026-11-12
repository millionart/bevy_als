# D222 入口选择占用同帧转换额度

UE `AnimNode_StateMachine.cpp:450–515,1293–1334` 将初始 Conduit 选中的出口计入当前帧已执行转换数。首次更新的 skip-first 阶段清理混合状态。

fresh Grounded 的 8 条双向轨迹共 24 帧，原始编译额度为 1。两个方向在额度 1 时首帧保留入口状态，下一帧启动普通 0.5 秒转场；额度 2 会在首帧接受两条边。同姿态路径提供控制。

目标在成功选择入口后扣除同帧额度。Rust fixture 经实际 FSM 验证两个额度、下一帧额度恢复及普通初始状态。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/AB_Als_Grounded -StateMachinePlayers -StateMachineEntryProbe docs/audit/bf144/d222/budget.request.json -Output docs/audit/bf144/d222/budget.native.json
python docs/audit/bf144/d222/replay_entry_budget.py
```
