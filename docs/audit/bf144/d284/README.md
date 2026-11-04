# BF144-D284 General Lean 半衰期

`settings.native.json` 是本轮重读的 `AIS_Als_Default`，`native-identities.json` 固定资产、请求、输出和 watchdog 报告；图、Lean 样本及骨架/网格沿用本轮 `docs/audit/bf144/d286/native-identities.json`。`source-facts.json` 记录 Grounded/InAir 共用字段、非正值分支、目标两个消费者和系统排序。

目标已有离线生成的 General 值及 InAir 图输入。Grounded 的源刷新读取同一输入，Standing 兼容路径读取同一生成配置；后者先于 InAir 输入写入，故不通过反向调整系统排序读取图输入。范围和待执行的图静态审计、Focused 测试及公共视觉 Gate 见 `tools/animgraph_authoring/bf144_d284.scope.json`。
