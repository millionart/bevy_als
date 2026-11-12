# D240 惯性化到期帧

原生节点先推进 elapsed 和 deficit，到达逐骨最大时长时清理活动数据，再根据当前状态决定是否施加惯性。到期帧原样转发姿态及曲线集合，随后记录该帧作为历史。

用实际 Fall 骨骼顺序和两份原生姿态，对主图的惯性节点运行恰达、越过和无请求三组控制。根变换和曲线为受控输入，其余 78 骨沿用原生姿态。1185 个骨骼结果精确复制输入。两种到期帧都保留输入根旋转的 w 值和缺失的 `PoseInAir`。旧求值顺序的消融在两帧都把 w 改为 1 并补入 `PoseInAir=0`。Rust fixture 核对实际惯性节点输出和到期状态，编译和执行列入统一验证。

```sh
python docs/audit/bf144/d240/replay_terminal.py --request
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AB_Als -InertializationProbe docs/audit/bf144/d240/terminal.request.json -Output docs/audit/bf144/d240/terminal.native.json
python docs/audit/bf144/d240/replay_terminal.py
```
