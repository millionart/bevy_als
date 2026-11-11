# D176 Turn 的原生停止窗口

`PlayQueuedTurnInPlaceAnimation` 创建 `BlendOutTriggerTime=0` 的惯性化Montage。`AnimMontage.cpp:2587–2637` 在实际substep后检查终段剩余播放时间，`GetRemainingPlayTimeToSectionEnd` 将剩余clip距离除以播放率。播放剩余≤1e−4秒即当帧停止，零delta继续保留当前实例。

八个实际Turn重采30/60/120 Hz和两组窄窗口控制，共40组。六个2秒序列在各普通帧率下旧终点判断晚一帧，两个蹲姿180°的普通控制保持源时点。窄窗口覆盖窗外、除播放率后进入窗口和零delta。当原生停止时槽权重为0，Rust fixture 在同帧核对清理及惯性化请求。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/TurnInPlace/A_Als_Turn_90_Left -MontageClockProbe docs/audit/bf144/d176/standing-90-left.request.json -Output docs/audit/bf144/d176/standing-90-left.native.json
python docs/audit/bf144/d176/replay_stop_window.py
```
