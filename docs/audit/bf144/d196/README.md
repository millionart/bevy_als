# D196 单份历史的惯性曲线

UE `AnimNode_Inertialization.cpp:647–676,954–975` 用同一份首次姿态及曲线作为两份历史。目标按姿态历史是否存在选择较早曲线。真实双历史中缺失的曲线仍按默认 0 读取。

fresh 主 AB 的八条 `PoseGrounded` 轨迹覆盖单历史、零和临界历史步长、双历史缺键及无历史。1→0.5 的原生首帧为 0.53887993，旧路径约 0.99745607；四个旧输出帧失配。逐运算 float 回放与原生一致。Rust fixture 经实际节点核对曲线值和控制组。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AB_Als -InertializationProbe docs/audit/bf144/d196/inertia.request.json -Output docs/audit/bf144/d196/inertia.native.json
python docs/audit/bf144/d196/replay_first_curve_history.py
```
