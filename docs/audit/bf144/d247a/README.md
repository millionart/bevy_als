# BF144-D247a RigTransform 向量旋转

UE `TransformPosition` 与 `InverseTransformVector` 使用 `v + w·2(q×v) + q×2(q×v)`，并在 ARM64 的 cross 和中间加法中使用 FMA；目标原 glam 四元数向量乘法使用另一种展开式。对于允许的近单位 quaternion，两式不再逐值相同。源 quaternion 与向量分别映射为 `(-x,-z,-y,w)` 和 `(x,z,y)`，源运算完成后再映回 runtime。

`trace.request.json` 从 `.462` 两条真实脚部射线历史中保留前两帧；fresh 原生第 1 帧左右脚世界端点、命中点、局部命中点和法线均与 `.463/inputs.json` 对应源记录逐值相同。完整请求、结果、源码闭包和哈希在本目录，压缩结果已逐字节验证解压。已有 `.463` 的 636 次完整消费者、因素隔离及消融恢复证据继续保留。

目标新增共用源旋转并接入 `RigTransform` 正逆变换及 hierarchy global/local 位置；ARM64 保持源 FMA，其他架构执行非融合的同一 cross 形式。近单位 yaw 下 Up 保持 Up 的最小反例已写入 Transform 与公开 FootOffsetTrace 两处 fixture。其他架构的 UE FMA 配置、非均匀负 scale 路径、完整 IK 和 Root 尚未验证；两骨 IK 直接旋转属 D247b。focused 测试、消融/恢复、图审计和公共 Playground Gate 待全部静态修复及 Bevy/依赖迁移后执行。
