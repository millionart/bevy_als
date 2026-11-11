# D191 惯性平移用厘米求衰减

UE `AnimNode_Inertialization.cpp:165–172,1049–1052` 用厘米距离和厘米每秒速度调用标量衰减。负速度低于源门槛时缩短时长，防止越过目标。目标在平移调用处将距离和速度转换为厘米，衰减偏移再转换为米写回骨骼。

fresh 主 AB 原生轨迹中，2.002µm→1.001µm 的历史在 50ms 保留 +0.010250µm，旧米制调用给出 −0.316723µm。一米偏移的 20 帧中，旧调用有 11 帧的浮点结果不同，最大约 0.937µm。相同标量函数以厘米输入的逐运算 float 回放匹配原生，Rust fixture 使用实际骨骼写回。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AB_Als -InertializationProbe docs/audit/bf144/d191/inertia.request.json -Output docs/audit/bf144/d191/inertia.native.json
python docs/audit/bf144/d191/replay_translation_units.py
```
