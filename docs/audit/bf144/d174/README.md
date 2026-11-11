# D174 Quick Stop 的源 yaw 分支

`AlsAnimationInstance.cpp:1590–1609` 从所选 float 度数减去 `RotationWorldSpace.Yaw` 的 double 值，再将差转为 float 并 `UnwindDegrees`。剩余角严格大于175°才逆时针重映射，0°选择左侧。实际通知保留已消费输入的度数；目标角由存储的弧度转一次度数，Actor yaw 从现有世界四元数按源double Rotator算出。

同一可用world quaternion的九组原生Rotator及实际QuickStop速率设置已重采。旧F32方向与单位往返在输入/目标两个175°组及两个零角邻域组改变左右。精确175°选择右，正负邻域、精确0°、普通角度及瞄准通过。Rust fixture 从同一选择入口核对站蹲四个根槽序列和速率。

```sh
python docs/audit/bf144/d174/replay_quick_stop.py --request > docs/audit/bf144/d174/yaw.request.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Data/AnimationInstance/AIS_Als_Default -GeometryQueryProbe docs/audit/bf144/d174/yaw.request.json -Output docs/audit/bf144/d174/yaw.native.json
python docs/audit/bf144/d174/replay_quick_stop.py
```
