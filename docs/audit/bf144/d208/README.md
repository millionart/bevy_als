# D208 Sequence 零移动的标记组时钟

UE `AnimSequenceBase.cpp:516–550` 在有效移动量为零时保留本地时钟，只按需建立本地 marker 索引。`AnimSync.cpp:219–258` 按候选排序继续尝试，全部没有有效终点时记录最后一项。`AnimSync.cpp:139–148` 仅从有效的前组标记终点恢复跨帧对齐。

fresh Crouch Forward、Left Forward 和 Right Forward 共 7 条原生轨迹 48 帧，包含 5 个全组零移动帧、12 次零移动尝试和 5 个恢复移动帧。恢复后加入的 Right Forward 原生从 0.8000000119 秒开始。用前次无效标记组的本地相位对齐，会把它映射到约 0.1395522505 秒。正步长直接加入和高分播放器零速率也有原生对照。

目标保留 Sequence 的本地时间与记录，在恢复移动时从有效的前组标记历史对齐。Rust fixture 经实际同步 resolver 核对零移动时的尝试顺序、局部时钟和标记有效性。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Forward -SyncGroupClockProbe docs/audit/bf144/d208/marker.request.json -Output docs/audit/bf144/d208/marker.native.json
python docs/audit/bf144/d208/replay_paused_markers.py
```
