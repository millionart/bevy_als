# BF144-D282 VelocityBlend 局部旋转精度

`root.native.json.gz` 是本轮重读的 `AB_Als`，`native-identities.json` 固定源资产、请求、解压字节哈希及 watchdog 报告；同轮 Standing、Crouching 和姿态依赖复用 D286 的原生证据。`source-facts.json` 记录源 F64 逆旋转、目标原先的 F32 路径以及 .502 隔离实验。

`precision-witness.json` 保留 .499 实际第 101 次查询的 F32 世界向量与四元数，以及按 .502 展开式在 F64 中求值后投回 F32 的局部分量。此项只改变逆旋转精度，D280/D281 的归一化与共享倒数以及 D268 的单位转换时点仍分别处理。

图静态审计、Focused 测试、消融和公共视觉 Gate 尚待全部实现完成后执行；范围见 `tools/animgraph_authoring/bf144_d282.scope.json`。
