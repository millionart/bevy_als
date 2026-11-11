# D171 重复同组替换与旧 Turn 淡出

源 `SK_Als` 将Transition和两个Turn槽放在 `Grounded`。`AnimInstance.cpp:3671–3695` 停止同组实例，`AnimMontage.cpp:1632–1642` 在已淡出时只接受严格更短的时长。相等和更长请求保留现有进度。

站蹲两个原生Montage各重采单次、相等、更长及更短四种请求，共48帧。第一次0.2秒淡出已推进0.15秒后，再请求0.2或0.3秒，原生下一帧结束；旧算法在等长三帧还留下0.1318359375、0.078125、0.0244140625。更短0.1秒仍以当前权重重启，下一帧为0.078125。Rust fixture 使用实际Transition分发和Turn时钟，对照全部权重及终止。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/SK_Als -Output docs/audit/bf144/d171/groups.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/TurnInPlace/A_Als_Turn_90_Left -MontageClockProbe docs/audit/bf144/d171/standing.request.json -Output docs/audit/bf144/d171/standing.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/TurnInPlace/A_Als_Crouch_Turn_90_Left -MontageClockProbe docs/audit/bf144/d171/crouching.request.json -Output docs/audit/bf144/d171/crouching.native.json
python docs/audit/bf144/d171/replay_replacement.py
```
