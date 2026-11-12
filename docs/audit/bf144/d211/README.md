# D211 按物理槽顺序累计 Cubic 过滤

UE `AnimInterpFilter.cpp:88–141,275–291,299–340` 从 10 个槽开始，过期后按游标搜索空槽，必要时增加 5 个槽，并按物理索引累加。正步长首先推进 D210 修复的时钟。过期记录的时间清零，源的有效性由正时间决定。

fresh 六个 Standing WalkRun BlendSpace 共 864 帧。输入覆盖连续短步长扩容、较长步长过期和槽复用。源槽模型的过滤位置和三个或更少的三角样本权重逐值匹配原生记录。改回按时间顺序累加会在 149 帧同时改变过滤位置和样本权重。Rust fixture 通过生产过滤核和实际三角采样比较这批读数。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Grounded/WalkRun/BS_Als_WalkRun_Forward -SyncGroupClockProbe docs/audit/bf144/d211/slots.request.json -Output docs/audit/bf144/d211/slots.native.json
python docs/audit/bf144/d211/replay_filter_slots.py
```
