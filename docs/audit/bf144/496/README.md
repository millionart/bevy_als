# BF144.496 运动段左虚拟足缩放

在 BF144.495 的同一 Playground 上读取实际 Run Start 方向权重与 Legs TwoWay alpha。新增权重之外的实测数据一致。原生新读 raw Lean 参考及两个实际方向的压缩加性叶。固定实际缓存姿态、权重和普通 Stand，Run Start 中只换 raw Lean 即减少一个 F32 步长，归 D235。

对普通 Stand 的单位缩放替换仅用于隔离动态加性来源。第 158 帧满权重路径直接消费加性结果。固定共同输入的 F64 算术改变中间差分，最终 F32 投影一致。第 105 帧 Run Start 前的 Movement 缓存已追到 Lean 施加阶段，独立校验仍待完成。

运行 `native.ps1`、`run_python.ps1 -Name observer`、`compile.ps1`、`playground.ps1` 和 `functional.ps1`。分析前解压原始报告，设置 ALS_UE_SOURCE_ROOT，再运行 `run_python.ps1 -Name prepare` 和 `run_replay.ps1 -Label baseline -Build`。取消 raw Lean 替换或普通 Stand 诊断替换，各自触发预期失败。恢复原字节同库重编后报告一致。
