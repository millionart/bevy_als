# D210 BlendSpace 过滤步长与初始时钟

UE `AnimInterpFilter.h:120–133` 初始化零时钟和零输出。`AnimInterpFilter.cpp:192–297` 在非正窗口直接返回输入，在正窗口只接受严格大于 0.0001 秒的步长。接受的首帧先推进时钟，再登记样本。

fresh Right Forward 原生 X/Y 窗口分别为 0.2/0.4 秒。97 个阈值轨迹帧和 112 个普通步长帧与源环形参考逐值匹配。独立消融显示旧门控改变 97 帧，旧首时钟分别改变 26 和 22 帧。目标在 18 个环形顺序尚等价的前缀帧逐值匹配，完整轨迹其余环形槽顺序差归 D211。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Grounded/WalkRun/BS_Als_WalkRun_Right_Forward -BlendSpaceProbe docs/audit/bf144/d210/edges.request.json -Output docs/audit/bf144/d210/edges.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Grounded/WalkRun/BS_Als_WalkRun_Right_Forward -BlendSpaceProbe docs/audit/bf144/d210/regular.request.json -Output docs/audit/bf144/d210/regular.native.json
python docs/audit/bf144/d210/replay_filter_clock.py
```
