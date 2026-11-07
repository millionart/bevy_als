# BF144-D281 标量除法的共享倒数

`source-facts.json` 固定 UE 标量除法及 ALS VelocityBlend/Lean 调用链；两处目标计算使用同一个倒数乘法。`native-identities.json` 与 `resumed-identities.json` 分别记录暂停前和恢复后主动画资产的读取请求、原始/压缩字节哈希和 watchdog 报告。`source-recheck.json` 记录恢复后源码及 D286 递归资产的身份核对。

可运行 `gzip -dc docs/audit/bf144/d281/root.resumed.native.json.gz | shasum -a 256` 核对重读输出；其他递归源资产见 `docs/audit/bf144/d286/native-identities.json`。图范围和待执行的审计见 `tools/animgraph_authoring/bf144_d281.scope.json`。

Focused 测试、消融、图静态审计和公共视觉 Gate 均按本轮要求留待全部条目实现后执行。D280 的欧氏归一化不属于本项。
