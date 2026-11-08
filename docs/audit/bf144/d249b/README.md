# BF144-D249b 足位置最终限长

ALS 足位置 unit 在 spring 更新后，以 float 计算 `LegLength * MaxLegStretchRatio`，再对目标足位置与大腿位置的 FVector 差调用 `GetClampedToMaxSize`。源实现按 XYZ 计算平方和，超限时使用 `Max * (1 / sqrt(SizeSquared))` 缩放。目标已由 D249a 提供源 XYZ 到 runtime XZY 的点积顺序，本项复用它并保留原有 float 乘积和小值分支。

`foot.request.json` 保留两帧 Rig 历史，fresh `foot.native.json.gz` 记录目标足位置、大腿、spring offset、长度与左右脚结果。`witness.json` 保存左右脚源公式复算与原生数据：两者均逐值相同，也与 `.468` 同一帧记录相同。右脚原生 Z 为 `13.793534620321694` cm；把倒数乘法替换为直接除法的独立模型得到 `13.79353462032168` cm。此数是源输入上的算子控制，不称为已经运行的目标结果。native 输出无损压缩，原始及压缩长度和 SHA-256 在 `native-identities.json`。

新增公开足位置 unit 的精确 fixture；`.468` 保留 636 源调用、两次因素消融和恢复证据。源厘米与 runtime 米制下的其他双精度舍入、不同坐标基以及完整 Rig/Root 未由本项证明。focused 测试、全量历史回归、消融及源码恢复、图静态审计和公共 Playground Gate 待全部静态修复及 Bevy/依赖迁移后执行。
