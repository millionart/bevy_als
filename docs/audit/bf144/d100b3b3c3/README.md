# BF144-D100b3b3c3：源边边接触

源边边分支在 box 有 margin 时分别查询两侧选中面的最近棱边，以源 GJK 法向计算分离并投影到静态凸包棱边；超出 cull distance 时不产生点。box 边查询使用 double 厘米，凸包使用 float 厘米；缩放凸包先反缩放查询位置，在未缩放凸包上选边后再缩放结果。零 margin 与强制零距离剔除也按源分支处理。源代码及精度见 `source-facts.json`。

fresh Arch02 原生样本见 `identities.json`。`witness-derivation.json` 记录根据源法向和最终点推导的候选面及对照；隔离测试使用原生最终点作为最近棱边查询的替代输入，不声称它们是原生 GJK witness。测试还覆盖平局、退化边、非均匀缩放及 cull；重建 reader 后的直接 GJK 报告由另一测试消费。

`python3 tools/import/import_contact_edge_settings.py --facts docs/audit/bf144/d100b3b3c3/source-facts.json --native docs/audit/bf144/d100b3b3c2a/classification.native.json.gz --output src/als/settings/contact_edge_zero_cull.rs` 生成目标开关。旧 reader 没有直接记录该 CVar，当前暂用源默认值；reader 源码已补采集，统一 Gate 重建后须重采并重生成。真实接入、测试、消融、全量转换 diff 和 Playground 均待后续。
