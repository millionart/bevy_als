# BF144.498 Lean 上游的 PoseGait 方向权重

原生和目标的 Lean 消费上一已求值姿态的 PoseGait。观察域的 PoseGrounded 均为 1。原生方向 BlendSpace 权重来自实际 tick 缓冲，目标权重使用实际过滤后坐标和当前三角算术重算。两个实际 WalkRun 子曲线的值在三帧均逐值相同。用生产曲线上下文混合复算两端各自的方向权重，然后只替换方向权重或子曲线。方向权重单因素恢复两个不同的帧。

原生 VelocityBlend 和目标输入已分别记录。更早的 VelocityBlend 产生原因和物理 CMC gait 映射另核。

在仓库根目录执行 `run_python.ps1 -Name observer`、`compile.ps1`、`playground.ps1`、`functional.ps1`。分析前解压原始报告并设置 ALS_UE_SOURCE_ROOT，再运行 `run_python.ps1 -Name prepare` 和 `run_replay.ps1 -Label baseline -Build`。把 native_weights_only 的调用改回 target_weights，`no-direction-replacement` 应在两个见证帧失配。恢复字节同库重编后报告相同。
