# BF144.503 D282 的真实方向、姿态和 Rig 历史

原可见 Playground 中临时为两个活动方向混合器接入右向输入读取器。两个运行使用同一二进制，默认转发原属性，控制仅在 query101 使用 D282 已验证的 F64 反事实值。基线除新增诊断字段与已命名输入针脚外逐值等于 BF144.498。独立的连续 VelocityBlend 复算证明 query102 原权重已归并，验证了单帧控制的输入范围。

实际第 101 帧两直接混合输出和最终姿态改变。第 102 帧直接输出归并，Root PoseGait 和一个右足 RigInput 四元数仍变。第 103 帧 Lean 原始值变化，有效 alpha 保持。此后两条 Pole 阻尼读取不同前态，固定相同目标、时钟及半衰期逐值复算 16 对输出。只替换为基线前态可恢复对应阻尼输出。观察至第 110 帧。

在项目根目录运行 `run_python.ps1 -Name observer`、`compile.ps1`，然后对 `baseline` 与 `widened` 分别运行 `playground.ps1` 和 `functional.ps1`。分析复跑前解压原始报告，设置 ALS_UE_SOURCE_ROOT 后运行 `run_python.ps1 -Name prepare`，再运行 `run_replay.ps1 -Label baseline -Build`。分别取消拓宽旋转或保留历史，两个反例应按预期失败。恢复字节同库重编后报告一致。
