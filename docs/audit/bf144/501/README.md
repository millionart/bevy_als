# BF144.501 首运动帧 VelocityBlend 种子

实际 Playground 的第 98 帧四向权重为零，初始化位已清除。第 99 帧按正常衰减产生首个非零种子。源第 98 帧以 F64 位移差复算速度，投影后等于目标计划速度。目标写回世界 F32 位置后的平面差分逐值复算已发布速度。保持实测朝向和零前态仅换回计划速度，显著缩小种子差，归 D254。

在固定原生局部向量、欧氏归一化和衰减的情况下，仅将共享倒数乘法替换为逐分量除法，前向种子改变两步，归 D281。目标实际同输入仅换共享倒数也有变化。组合控制仍留右向一个 F32 步长。

从项目根目录运行 `run_python.ps1 -Name observer`、`compile.ps1`、`playground.ps1`、`functional.ps1`。解压原始观察并设置 ALS_UE_SOURCE_ROOT 后，执行 `run_python.ps1 -Name prepare` 和 `run_replay.ps1 -Label baseline -Build`。把 plan_control 改回实际速度、或把 native_direct_direction 改回原生倒数法，分别触发两个负例。恢复源码字节和同库重编后报告相同。
