# BF144.502 VelocityBlend 局部单位与旋转精度

在实际首移动帧及后续 11 帧上验证。固定原生局部 D64 向量和后续 F32 归一化，分别在厘米中转 F32 和先换米再转 F32。第 99 帧剩余的右向一步在这个单位单因素对照中精确复现，归 D268。

局部旋转精度对照在同一世界基内保持实际目标 F32 速度和四元数的数值、目标展开式、历史和后续运算。F32 展开式逐值复现已锁定 glam。只拓宽到 F64 再投回 F32，第 101 帧实际右向改变一步，归 D282。对投影源输入固定 F64 精度互换展开式时，当前 12 帧最终权重一致。

设置 ALS_UE_SOURCE_ROOT 后执行 `run_python.ps1 -Name prepare` 和 `run_replay.ps1 -Label baseline -Build`。分别关闭 USE_PRECAST_METERS 或 WIDEN_RUNTIME_ROTATION，执行 `no-meter-stage` 或 `no-wide-rotation` 应触发对应失败。恢复源码字节同库重编后报告一致。
