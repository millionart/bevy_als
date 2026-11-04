# BF144-D283 Grounded VelocityBlend 半衰期

`source-facts.json` 与 D284 的原生设置记录确定 `Grounded.VelocityBlendInterpolationHalfLife` 的来源。`native-identities.json` 固定本轮读取的五条曲线、源资产哈希及各自 watchdog 报告。`standing.generated.json` 是生成器原始输出；`generated-diff.json` 给出旧提交和两份输出的 SHA-256，新增字段为 0.1，其余曲线逐键不变。

重生成时设置 `ALS_SOURCE_CONTENT_ROOT` 指向 ALS 插件的 Content，`ALS_PYTHON` 指向 Python 3.10 或更新版本，然后在仓库根目录运行 `pwsh -NoProfile -File docs/audit/bf144/d283/generate.ps1`。第一次使用默认 Python 3.9 的失败及 watchdog 报告单独保留。标准输出包含本机路径，未纳入提交。

导入器自测、完整生成 AnimBP 及递归依赖的转换和新旧产物 diff、动画静态审计、消融和公共视觉 Gate 均待所有条目实现后执行。范围及源证据见 `tools/animgraph_authoring/bf144_d283.scope.json`。
