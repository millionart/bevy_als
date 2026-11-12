# D204 清理转场栈前更新仍活动的旧状态

UE `AnimNode_StateMachine.cpp:120–154,529–630,904–916,1414–1453,1577–1586` 先推进全栈时钟，再以清理前的权重更新仍活动普通转场的两侧状态，每个状态只更新一次。完成项不更新两侧状态。惯性完成项的 QueryAlpha 为零，清栈后只给尚未更新的当前状态一次全权重惯性更新。

fresh Locomotion 的 Land→Land Movement 矩阵包含惯性中断、自动普通中断和无中断。原生惯性尾更新保留部分权重，普通完成边界上的尾更新权重为零。Standing Movement Details 的 Start→Run→Walk→Start 验证重入状态既可先以 0.341796875 或 0.158203125 更新，也可在旧 Start 转场已完成时以 1 更新。两种情况的最终记录权重均为 1。

修复按原顺序更新即将清理的活动状态，以实际旧栈权重保留同步与事件时钟，再计算最终姿态。原生 10 个完成边界夹具中，早清理丢失 8 个边界的更新。独立 Land 同步组回放保留尾更新历史后，三条惯性返回路径保留前组时钟。Rust fixture 检查各序列实际更新、惯性作用域、权重、同步记录及最终姿态。同步上下文通用的微小权重过滤仍由 D219 覆盖。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/AB_Als_Locomotion -StateMachinePlayers -StateMachineEntryProbe docs/audit/bf144/d204/transitions.request.json -Output docs/audit/bf144/d204/transitions.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Air/Land/A_Als_Land_Light -SyncGroupClockProbe docs/audit/bf144/d204/group.request.json -Output docs/audit/bf144/d204/group.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/Stances/AB_Als_Standing -StateMachinePlayers -StateMachineEntryProbe docs/audit/bf144/d204/reentry.request.json -Output docs/audit/bf144/d204/reentry.native.json
python docs/audit/bf144/d204/replay_transition_tails.py
```
