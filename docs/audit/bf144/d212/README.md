# D212 新 BlendSpace leader 样本的标记起点

UE `BlendSpace.cpp:700–725` 在 BlendSpace 有实例记录、样本记录无效且组起始标记有效时，先按既有组相位初始化新样本。`AnimSequence.cpp:4229–4258` 再由该标记比例与样本标记时间得到本地秒数。

fresh Right Forward 和 Forward 共 8 条轨迹 54 帧。4 条低于或等于过滤阈值的路径首帧只采样 Pose，下一帧 Walk 首次成为 leader。使用本地默认起点会破坏 4 条路径。Right Forward 50 微秒入口的默认值为 0.2266666740 秒，按前组有效标记初始化为 0.2267164439 秒。4 条已有样本记录控制按各自本地时钟继续。

目标在持续 BlendSpace 的新 leader 样本缺少有效记录时采用已有组标记。Rust fixture 通过实际同步 resolver 检查新样本和已有 follower 两种路径。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Grounded/WalkRun/BS_Als_WalkRun_Right_Forward -SyncGroupClockProbe docs/audit/bf144/d212/leader.request.json -Output docs/audit/bf144/d212/leader.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Right_Forward -Output docs/audit/bf144/d212/right_walk.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Forward -Output docs/audit/bf144/d212/forward_walk.native.json
python docs/audit/bf144/d212/replay_new_leader_sample.py
```
