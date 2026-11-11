# D165a 自动 IK 骨长的厘米舍入

`RigUnit_TwoBoneIKSimple.cpp:153–188` 从初始全局骨段计算 `FVector` 厘米差，乘当前相对初始缩放并在长度赋值时舍入为 float。第二段沿用 A 的当前缩放。项目先按源厘米和向量累加顺序完成该段，随后以 double 投影到米并交给既有位置核。显式长度仍以调用者传入的米值进入。

原生 `scaled_distance` 在三组实际骨段与缩放上复核六个长度，覆盖双腿和非均匀缩放。九个 native `SolveBasicTwoBoneIK` 控制的膝位置在模型中均小于 10 nm 误差。旧米制 float 在三个伸直边界改变分支，偏差 74–98 µm，正常弯曲及完全伸直控制均低于 10 µm。Rust fixture 以自动长度零值从真实回调读取并写回关节。`.477` 的 1,272 次实际消费和删除厘米 float 的物理消融保留原证据。

```sh
python docs/audit/bf144/d165a/replay_lengths.py --request > docs/audit/bf144/d165a/lengths.request.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALSExtras/Environment/Meshes/SM_Als_Stairs_01 -GeometryQueryProbe docs/audit/bf144/d165a/lengths.request.json -Output docs/audit/bf144/d165a/lengths.native.json
python docs/audit/bf144/d165a/replay_lengths.py
```
