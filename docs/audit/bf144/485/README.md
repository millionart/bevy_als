# BF144.485 足部 Socket 与 D276 的 VM 消费

317 个有效原生帧的左右 FootTarget 精确来自上一帧已发布的本地骨骼、UE 逐骨 TQS 合成与归一化，以及当前 mesh-to-world。第 0 帧的 FeetState 尚无效，当前 mesh 正好复算该帧记录的目标。原生路径对照数据同时保留当前发布、未经最终归一化的 Root、去掉组件空间逐骨归一化三个对照。

目标 319 帧用实测绑定、逐帧实际历史与生产 VM 单元重放。只在克隆的当帧前态替换 103 侧 RigInput 旋转，足骨局部 F32 姿态随之改变 103 行。两块原场景地板几何计算的 636 条基线 trace 全匹配，反事实查询沿同一几何重新求值且完全相同。反事实分支每帧丢弃，连续轨迹未验证。第 0 帧以及其余无输入变化的帧只执行实际基线。

运行 `run_python.ps1 -Name prepare` 从 Git 证据恢复输入，然后执行 `run_replay.ps1 -Label native -Build` 与 `run_replay.ps1 -Label restored`。消融将克隆中写入的 `RigValue::Quaternion(corrected)` 替换为 `RigValue::Quaternion(baseline)`，执行 `run_replay.ps1 -Label ablated -Build`，应在完整实际基线重放后因 F32 差异数为零退出。恢复原源码字节后用同一组 target_game 库重编，`baseline` 和 `restored` 结果字节相同。
