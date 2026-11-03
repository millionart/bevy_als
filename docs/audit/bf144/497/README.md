# BF144.497 实际 Lean 采样、时钟与左虚拟足缩放

使用原生 Standing 的真实 tick 记录与三条 time-zero 原生压缩加性叶。目标新增只读观察取得 Lean 插值的前后状态、BlendSpace 输入位置和实际样本时间。第 99 帧激活，第 105 帧首次超过半步门槛并影响左虚拟足缩放。各帧真实位置的三角形权重、目标 alpha 转移和施加结果逐值复算。

求值后的图查询时钟字段为空。观察器记录播放器发布的 Delta，再用生产映射逐帧验证它能复算状态转移。Absolute(0) 是各样本的采样请求。原生同运动帧的 tick 权重用实际步态运行量复算，父图尚在转场的其他 tick 保留独立边界。

在仓库根目录运行 `native.ps1`、`run_python.ps1 -Name observer`、`compile.ps1`、`playground.ps1` 和 `functional.ps1`。解压原始报告并设置 ALS_UE_SOURCE_ROOT 后执行 `run_python.ps1 -Name prepare` 和 `run_replay.ps1 -Label baseline -Build`。令播放器 Delta 为零或取消 raw 参考替换，各会触发预期失败。恢复字节同库重编后报告一致。
