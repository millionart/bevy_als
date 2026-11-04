# BF144.499 VelocityBlend 的欧氏归一化阶段

两端实测的世界速度、旋转、前态及四向权重逐值复算。原生横向局部速度的舍入候选有 96 个产生相同最终权重。固定各候选的其他算子去掉欧氏归一化，所有候选均在同三帧失配。目标使用相同自身局部输入补该步，也改变三个字段。D280 归因限定于归一化阶段。

目标真实调用解析到 FloatExt 的差值式插值，当前窗口与源差值插值逐值一致。厘米局部速度的独立事后米制投影仅为数值诊断。保留原方向历史时，D280 单因素没有改变已验证的三帧 PoseGait。更早的实际速度、旋转和历史差异另核。

在仓库根目录设置 ALS_UE_SOURCE_ROOT，运行 `run_python.ps1 -Name prepare` 与 `run_replay.ps1 -Label baseline -Build`。把 native_direction 的原生调用从 true 改成 false 后，`no-euclidean` 应因三个字段失配退出。恢复源码字节并同库重编后，`restored` 应与基线字节相同。
