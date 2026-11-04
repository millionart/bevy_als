# BF144-D285 Grounded Lean 的分支精度

`source-facts.json` 固定本轮 ALS/UE 点积、类型和目标消费者事实。`standing.native.json.gz` 是本轮重新读取的 Standing 原生输出；`native-identities.json` 记录源资产、原始与压缩结果的长度和 SHA-256，以及 watchdog 报告。父图、Crouching、BlendSpace、样本和角色骨架/网格复用同一轮 `docs/audit/bf144/d286/native-identities.json` 中刚读取的原始证据。

可在仓库根目录运行 `gzip -dc docs/audit/bf144/d285/standing.native.json.gz | shasum -a 256` 重算原始摘要。源世界坐标、厘米单位和双精度点积决定加速或制动分支；局部限幅与共享倒数分别归 D286 和 D281。

图静态审计、Focused 测试、消融和公共视觉 Gate 尚待全部实现完成后执行；范围见 `tools/animgraph_authoring/bf144_d285.scope.json`。
