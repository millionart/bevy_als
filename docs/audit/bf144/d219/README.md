# D219 保留已更新的同步成员

UE `AnimNode_AssetPlayerBase.cpp:26–58` 在播放器 Update 时以最终 Context 权重创建 tick。`AnimSyncScope.cpp:31–71` 与 `AnimSync.cpp:49–70` 将该记录直接提交给同步组。

fresh Standing 的 BlendList、SprintBlock、TwoWay 和 Base 缓存共 80 帧调用记录，并经原生 FAnimSync 检查成员。旧合并阈值丢弃 8 个正权重及 46 个零权重记录，清空 16 帧仍有更新的同步组。首个嵌套反例含两个约 9.83784e−6 的 Sprint 记录。全阻断及普通权重对照匹配。

目标保留实际提交的记录，合并重复缓存时取最大权重。Rust fixture 覆盖零权重、弱权重和经过两层实际父混合的同步出口。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/Stances/AB_Als_Standing -BlendListProbe docs/audit/bf144/d219/membership.request.json -Output docs/audit/bf144/d219/membership.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Grounded/Sprint/A_Als_Sprint -SyncGroupClockProbe docs/audit/bf144/d219/sync.request.json -Output docs/audit/bf144/d219/sync.native.json
python docs/audit/bf144/d219/replay_membership.py
```
