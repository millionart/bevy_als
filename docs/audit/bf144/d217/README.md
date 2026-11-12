# D217 BlendSpace 的近零步长样本历史

UE `BlendSpace.cpp:700–752,773–795` 用有效移动量的 `IsNearlyZero` 判断 leader。已有实例记录时保留样本当前和前次时钟，留空组标记终点。冷 leader 从实例归一化时钟建记录并发布位置，前次样本时钟保持默认值。带有效组标记的 follower 只在 Context Delta 非零时同步。

fresh Forward 与 Right Forward 共 22 条轨迹 136 帧。11 次冷 leader 发布有效标记，9 次热候选保留本地样本，2 次零步长 follower 保留冷默认或热历史。统一按首候选选 leader 会错 6 次，统一发布热终点会错 3 次，拿当前时钟覆盖前次时钟会错 20 个样本。2e−8 秒和普通步长提供上界对照。

目标保存实例标记有效性和每样本前次时钟，并按实际组候选角色消费。Rust fixture 通过实际同步出口、BlendSpace 和采样时间引脚对照原生时钟。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Grounded/WalkRun/BS_Als_WalkRun_Forward -SyncGroupClockProbe docs/audit/bf144/d217/near_zero.request.json -Output docs/audit/bf144/d217/near_zero.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Forward_Pose -Output docs/audit/bf144/d217/forward_pose.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Right_Pose -Output docs/audit/bf144/d217/right_pose.native.json
python docs/audit/bf144/d217/replay_near_zero.py
```
