# D216 BlendList 的姿态和曲线求值分支

UE `AnimNode_BlendListBase.cpp:301–337` 直接输出唯一的满权重姿态，两个相关姿态的 float 权重和距 1 不超过 1e−8 时调用双姿态分支。`AnimationRuntime.cpp:527–542` 使用第一权重的补数作为第二姿态权重。`AnimCurveTypes.h:692–717` 用相同补数对曲线执行 Lerp。

fresh Standing 骨架的 26 帧原生读数覆盖 11 帧单姿态、9 帧双姿态和 6 帧通用加权。缩放近满单姿态会改变 4 帧 scale，使用原始第二权重会改变 7 帧根位置，改用曲线加权和会改变 1 帧曲线。首个普通双姿态帧的第二原始权重为 0.0196759291，求值补数为 0.0196759105。

目标共用一条 BlendList 姿态及曲线入口。同步和根运动用原 Update 权重。Rust fixture 以原生受控根变换、曲线和元数据对照三条分支。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/Stances/AB_Als_Standing -BlendListProbe docs/audit/bf144/d216/two.request.json -Output docs/audit/bf144/d216/two.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/Stances/AB_Als_Standing -BlendListProbe docs/audit/bf144/d216/multi.request.json -Output docs/audit/bf144/d216/multi.native.json
python docs/audit/bf144/d216/replay_pose_branches.py
```
