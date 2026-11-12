# D209 在 leader 推进前清理失效标记记录

UE `AnimationAsset.cpp:121–224` 在 Prepare 阶段清理无历史、失活、换资产或公共标记集合变化的记录，再由 `AnimSequence.cpp:3373–3440` 从本地秒时钟重建并推进。Finalize 阶段的 leader 索引变化规则随后作用于 follower。

fresh Crouch Backward、Forward 和 Left Forward 共 20 条原生轨迹 784 帧。独立回放逐值匹配当前、前次位置及标记区间。延后清理使三条轨迹的 14 帧失配。Forward 的首个失败位置为 0.6500000953674316 秒，原生重建后为 0.6500000357627869 秒。九帧缓存探针确认失活空帧保留子时间，恢复时继续推进。

目标复用 Prepare 阶段的清理条件，在 leader 消费记录前应用。Rust fixture 保留实际序列记录穿过整组空帧，并覆盖连续活跃对照。BlendSpace 内部用归一化时间重建样本的边界由 D213 覆盖。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Backward -SyncGroupClockProbe docs/audit/bf144/d209/marker.request.json -CachedPoseLifecycleProbe -Output docs/audit/bf144/d209/marker.native.json
python docs/audit/bf144/d209/replay_marker_prepare.py
```
