# D213 BlendSpace 重建采用实例归一化时钟

UE `BlendSpace.cpp:574–609,704–710,745–798` 单独保留实例 TimeAccumulator 和样本标记记录。重建组 leader 时先用实例归一化时间乘当前样本长度。组 follower 的实例记录失效时也重建最高权重样本的秒时钟，保留该样本已有的标记记录。`AnimSequenceBase.cpp:597–613` 随后记录重建后的 previous，再向组标记推进。

fresh Forward BlendSpace 与 Crouch Sequence 共 7 条轨迹 150 帧。13 帧预热后的 0.4433335066 秒在实例重建后为 0.4433335364 秒。重入时换到更短的 Run 样本，原生从 0.3129413128 秒开始。清理 follower 的本地样本记录还会错误地把 previous 映射到约 0.7978260517 秒。连续活跃和往返恰好可表示的控制均已核对。

目标保留实例归一化值。leader 在 Prepare 重建时使用该值，follower 在重建最高权重样本时保留已有标记记录。Rust fixture 通过实际同步出口和 BlendSpace 节点核对前次位置与实例时间。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Grounded/WalkRun/BS_Als_WalkRun_Forward -SyncGroupClockProbe docs/audit/bf144/d213/normalized.request.json -Output docs/audit/bf144/d213/normalized.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Forward -Output docs/audit/bf144/d213/run.native.json
python docs/audit/bf144/d213/replay_normalized_history.py
```
