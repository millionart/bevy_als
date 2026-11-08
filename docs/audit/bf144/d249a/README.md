# BF144-D249a Pole 与两骨 IK 向量运算

本地 ALS 的 `TryCalculatePoleVector` 对 AC 规范化，按源 XYZ 点积投影 AB，再规范化垂直方向。UE TwoBoneIK 的距离、平面、方向投影及旋转主/次轴也消费 FVector 点积和规范化；double `InvSqrt` 是 `1 / sqrt`，各分量再乘倒数。当前导入将源 XYZ 映射为 runtime XZY；目标原先使用 runtime XYZ 的 glam 点积和逐分量除法。

`witness.json` 记录 fresh `.250` 七帧历史末帧的左右腿 Pole 输入及原生 projection/direction；用源公式逐值复算两腿均相同。合成最小控制的输入为源 A=(0,0,0)、B=(0.5,1,2)、C=(3,4,5)，源公式 direction.y 为 `−0.35977521071801355`；旧 `.284` 原始 target 报告缺失，因此此控制的完整数据明确标为源公式复算，而不是新原生采样。

`ik.request.json` 从 `.478/request.request.json` 选择第 19 和 28 行，fresh 读取的两个原生 joint/end 与 `.478/inputs.json` 相应值逐位一致。两个源样本均有独立点积和倒数因素反例，完整 native 输出、请求、源文件和哈希保存在本目录；`.478/.479` 保留原 2596 位置和 5192 旋转控制及消融证据。压缩的原生结果已逐字节验证解压。

共享 `RigGeometry` 点积、向量规范化、Pole、两骨位置和旋转的已证 FVector 消费者使用源顺序和倒数乘法；新增 source Pole 与真实腿 IK 精确 fixture。该顺序依据当前导入的固定 XZY 映射，其他自定义坐标基未在此验证。最终 FootLocation clamp 属 D249b，四元数归一化属 D248；退化/拉伸、跨平台数学函数、完整后代姿态及所有目标运行结果尚未验证。focused 测试、消融/恢复、图静态审计和公共 Playground Gate 待全部静态修复及 Bevy/依赖迁移后执行。
