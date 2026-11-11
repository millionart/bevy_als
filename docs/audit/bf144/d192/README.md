# D192 惯性平移的混合精度

UE `AnimNode_Inertialization.cpp:875–899,1049–1052` 用 double 厘米位置求差值，距离先落为 float，方向再除以该距离。旧历史沿 double 方向投影后再次落为 float，两个 float 幅度相减求速度，最终保存 float 方向与厘米速度。`Vector.h:324–328,1545–1548,1714–1717` 确认倒数乘法与 XYZ 求和顺序。目标沿项目坐标基交换 Y/Z，单位转换在位置相减前执行。

fresh 主 AB 六例覆盖相同历史、100µs 边界、多轴和非零目标。相同位置 0.0010000000474974513m 在 100.1µs 历史下得到 −0.00007443137cm/s，原生 50ms 输出 0.000896456912160m。去掉该速度后 50ms 和 1/60 秒分别偏约 27.5nm 和 11.9nm。100µs 控制仍为零速度。Rust fixture 核对差值投影及实际骨骼写回，D190 与 D191 的原生夹具保留。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AB_Als -InertializationProbe docs/audit/bf144/d192/inertia.request.json -Output docs/audit/bf144/d192/inertia.native.json
python docs/audit/bf144/d192/replay_inertial_projection.py
```
