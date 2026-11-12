# D215 BlendList 子项相关性

UE `AnimNode_BlendListBase.cpp:247–263,301–312` 只对严格大于 `ZERO_ANIMWEIGHT_THRESH` 的普通子项执行 Update 和 Evaluate。fresh Standing gait 节点的 ChildUpateMode 为 Default。零时长切换的旧子项更新由原有独立分支处理。

原生 35 帧中，旧阈值会多更新 7 次 Sprint，权重介于 0.0000016867 和 0.0000097171。0.4 毫秒入口在权重位于门槛两侧时都提供原生对照。Base 缓存按原生每帧更新一次。目标统一相关性门槛，Rust fixture 通过实际 GameplayTag BlendList 与缓存读取核对各子项更新。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/Stances/AB_Als_Standing -BlendListProbe docs/audit/bf144/d215/relevancy.request.json -Output docs/audit/bf144/d215/relevancy.native.json
python docs/audit/bf144/d215/replay_relevancy.py
```
