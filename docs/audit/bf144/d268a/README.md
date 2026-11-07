# BF144-D268a VelocityBlend 的厘米投影阶段

`source-facts.json` 保存 fresh ALS `GetVelocity`、源 `FVector3f::Normalize` 阈值、目标两个消费者，以及 `.502` 第99帧固定源局部输入的厘米与提前换米对照；图闭包和同轮原生资产身份见 `tools/animgraph_authoring/bf144_d268a.scope.json`。

目标世界速度先以 F64 转为厘米再旋转，随后投 F32；归一化平方阈值从米制等价数改为源厘米单位。输入已在目标侧丢失的精度不在本项重建。已写第99帧局部方向、源阈值和双精度旋转控制。全部静态修复及 Bevy/依赖迁移后执行 focused 测试、消融、图静态审计和公共视觉 Gate；把单位换算移到 F32 投影前的米制结果应恢复旧反例，源码字节恢复并同 target 重编后复验。上述测试及消融尚未运行。
