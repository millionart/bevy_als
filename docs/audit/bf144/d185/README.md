# D185 Mantle 提前混出读取已消费输入

ALS `AlsAnimNotifyState_EarlyBlendOut.cpp:35–66` 在 branching Tick 读取角色 `bHasInput`。`AlsCharacter.cpp:1109–1126` 从 CMC 加速度归一化得到该值。`AlsCharacterMovementComponent.cpp:202–210` 在输入被阻止时令消费向量为零，UE `CharacterMovementComponent.cpp:6442–6453` 再由该向量生成当前加速度。

目标通知复用本帧控制器已消费的 `basis.desired_motion` 和实际运动快照的归一化。六种资产均采用同样的输入、空中、瞄准和蹲姿条件。12 个阻止命令或覆盖输入的控制在旧路径误触发，绕过归一化会新增 6 个微小输入误触发。

fresh 原生 Low 和 High 时钟在停止 0.1 秒后权重分别为 0.8017492294 和 0.84375，同期未调用 Stop 的控制仍为 1。两份探针验证混出权重，另四份通知属性复用 D183 原生记录。实际 Mantle 尾段夹具覆盖放行、阻止及独立瞄准条件，并观察根槽采样。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Actions/Mantle/AM_Als_Mantle_Low -MontageClockProbe docs/audit/bf144/d185/low.request.json -Output docs/audit/bf144/d185/low.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Actions/Mantle/AM_Als_Mantle_High -MontageClockProbe docs/audit/bf144/d185/high.request.json -Output docs/audit/bf144/d185/high.native.json
python docs/audit/bf144/d185/replay_consumed_input.py
```
