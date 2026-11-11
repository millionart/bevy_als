# D186 Montage 槽通知相关性

UE `AnimNode_Slot.cpp:55–58` 在槽 Update 上报局部与全图权重。`AnimInstanceProxy.cpp:1020–1075` 以严格大于 `1e-5` 的局部权重记录本帧，并在下一次图更新保留一帧。`AnimInstance.cpp:890–903,950–956` 在完成图更新后翻转缓冲，隐藏自主派发读取冻结的槽历史。`AnimNotifyQueue.cpp:64,138–149` 以实例权重先筛候选，再接收相关槽的队列。

fresh AB_Als 与 Roll Montage 确认 PostLocomotion 对应同一槽。原生 Proxy 的 12 步取证覆盖严格门槛、全图权重为零、连续两帧无槽、隐藏冻结及未注册槽。使用旧的无条件槽资格会错 6 步；移除历史、隐藏时推进历史、改用含等号门槛和附加全图门槛的消融均失配。实例权重等于自身门槛时仍可进入已相关的槽。

实际 SourceSlot 求值更新现有实例的图帧记录。排队动作通知在图求值后按本帧和上一帧相关性派发；branching 动作仍在前序派发。根运动继续使用当前推进前已缓存的倍率，GraphFootsteps 读取相同槽历史。真实角色夹具覆盖门槛、单帧保留、第二帧 End、重入 Begin、隐藏无相关性，以及已有相关性冻结的控制。

读取器第一轮发现清槽函数受保护。复用现有派生 Proxy 的公开声明后重新构建取证。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AB_Als -SlotProbe docs/audit/bf144/d186/slot.request.json -Output docs/audit/bf144/d186/slot.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Actions/Roll/AM_Als_Roll -NotifyClockProbe -Output docs/audit/bf144/d186/roll.native.json
python docs/audit/bf144/d186/replay_slot_relevance.py
```
