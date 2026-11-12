# D205 同身份重置后的长度重同步

UE `AnimSync.cpp:139–148,195–217` 初始化前组已发布的长度比例。当惯性请求进入长度同步且前 leader 分数至少等于新 leader，`AnimSequenceBase.cpp:505–515` 先用该比例乘当前长度，再从该位置推进。播放器身份和资产相同也遵循这条路径。

8 条 Land 原生轨迹共 15 帧的时钟和比例逐值匹配。用重置的本地时间替代前组比例会破坏 3 条同身份路径。把前组发布值换成已循环归一化的相位，会让精确末点比例 1 变成 0，导致该路径的 previous 和 position 改变。不同身份、不同资产、较强新 leader、无惯性请求及无历史提供对照。

目标在同步出口保存 leader 的精确长度比例，符合资格时直接用比例推进。Rust fixture 经实际同步 resolver 比较原生 previous、position 和 ratio。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Air/Land/A_Als_Land_Light -SyncGroupClockProbe docs/audit/bf144/d205/resync.request.json -Output docs/audit/bf144/d205/resync.native.json
python docs/audit/bf144/d205/replay_length_resync.py
```
