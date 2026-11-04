# BF144-D286 Grounded Lean 的局部空间

`source-facts.json` 保存本轮 ALS、UE 和目标消费者的源文件身份与摘录。`native-identities.json` 记录13个通过仓库读取器重新提取的资产、请求、输出哈希及 watchdog 报告。Root、Grounded、Standing 和 Crouching 原生报告较大，以无损 gzip 存放；清单中的 `raw` 字段是解压后的长度和 SHA-256，`compressed` 字段是仓库文件的长度和 SHA-256。

可在仓库根目录使用 `gzip -dc docs/audit/bf144/d286/standing.native.json.gz | shasum -a 256` 重算 Standing 原始摘要；其他压缩文件同理。读取器通过 `tools/run_ue_source_reader.ps1` 执行，资产标识和对应输出记录在 `native-identities.json`；其标准输出含本机路径，因此不纳入提交。相关图与动作范围见 `tools/animgraph_authoring/bf144_d286.scope.json`。

本项实现逆旋转后在局部三维空间限幅。加速/制动点积精度和共享倒数分别仍归 D285、D281。根图静态审计、Focused 测试、消融和公共视觉 Gate 按本轮要求待全部实现完成后执行。
