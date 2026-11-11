# D184 旧 Roll 实例的 branching End

UE `AnimInstance.cpp:3696–3711` 在 Stop 时移除活动实例引用，ALS `AlsCharacter_Actions.cpp:27–32` 允许已停止 Roll 重入。UE `AnimMontage.cpp:1746–1778,2818–2827` 让中断实例保持已活动的 branching state 到 Terminate。`AlsAnimNotifyState_SetLocomotionAction.cpp:37–48` 的 End 在当前 tag 仍匹配时清除动作。

fresh Roll Montage 确认该状态的 tag 为 Rolling。原生时钟的旧实例在第 1 帧停止、权重仍为 0.068587，第 6 帧终止；新实例在这段时间仍位于有效动作区间。按源回调重放，旧实例 End 清除同名动作，外部 Ragdolling 保留。恢复旧目标保护会令同名动作的 4 帧失配。

移除派发器的 Roll 专属保护。原有真实重入测试现在区分旧状态自然结束及中断后终止，核对 End/Begin 次序、动作组件与根图输入，同时检查尾部姿态与新实例仍有效。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Actions/Roll/AM_Als_Roll -MontageClockProbe docs/audit/bf144/d184/reentry.request.json -Output docs/audit/bf144/d184/reentry.native.json
python docs/audit/bf144/d184/replay_reentry_termination.py
```
