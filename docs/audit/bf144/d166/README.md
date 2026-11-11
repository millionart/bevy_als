# D166 动态过渡的锁定距离

`AlsAnimationInstance.cpp:1716–1756` 对两对世界端点分别计算 double 厘米平方。阈值在 float 厘米中乘 `ScaleWorldSpace` 并平方，锁权重与距离都严格超过门槛才合格。两脚合格时，较大距离优先，相等选左。

实际 `AIS_Als_Default` 重采确认阈值8 cm、播放率1.5、混合0.2及四个序列。14个原生 `FVector` 距离对照覆盖七组输入。两个站姿的近相等距离都选右，旧米制float将其变成平局并选左。30°输入为63.99999962647951 cm²，源拒绝且旧米制判为合格。严格相等、门槛上侧、缩放和锁权重控制通过。

图输入传递两脚的目标及锁位置，实际Idle回调与独立状态共用厘米判断。Rust fixture 覆盖两站姿、同帧守卫及真实Standing Idle消费。Playground的独立资格与恢复判据使用相同源单位，距离 telemetry 继续以米平方记录。

```sh
python docs/audit/bf144/d166/replay_distances.py --request > docs/audit/bf144/d166/distance.request.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Data/AnimationInstance/AIS_Als_Default -GeometryQueryProbe docs/audit/bf144/d166/distance.request.json -Output docs/audit/bf144/d166/distance.native.json
python docs/audit/bf144/d166/replay_distances.py
```
