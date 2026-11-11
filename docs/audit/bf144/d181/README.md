# D181 Roll 中断与排队状态

ALS `AlsCharacter_Actions.cpp:758–777` 在进入 Ragdoll 时对活动 Montage 请求 0.2 秒混出。UE `AnimMontage.cpp:1558–1571,2818–2856` 在 Stop 时锁存中断，中断后不再从 Montage 或槽动画段入队。正常派发时，`AnimInstance.cpp:1949–1980` 结束没有继续入队的活动状态。`AnimMontage.cpp:1746–1778` 在实例终止时结束仍活动的 branching states。`AlsAnimNotifyState_SetRootMotionScale.cpp:43–61` 仅在当前倍率仍匹配状态时写回 1。

两份 fresh 原生资产确认 RootScale 来自 Roll 序列，SetLocomotionAction 来自 Roll Montage。原生时钟在停止帧权重为 0.980324，11 帧后实例才终止。源代码与原生时钟的受控回放覆盖默认倍率、外部倍率 2 和持续播放。旧清理路径 14 帧不匹配，若停止时结束全部状态则有 4 帧 branching End 位置不符。

已在派发路径按通知资格结束中断实例的排队状态，沿用原 End 对倍率的保护。Rust fixture 以实际 Roll 与 Ragdoll 输入核对两种倍率、同帧 End、尾部存在和后续 branching End。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Actions/Roll/A_Als_Roll -NotifyClockProbe -Output docs/audit/bf144/d181/roll.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Actions/Roll/AM_Als_Roll -MontageClockProbe docs/audit/bf144/d181/montage.request.json -Output docs/audit/bf144/d181/montage.native.json
python docs/audit/bf144/d181/replay_interrupt.py
```
