# BF144-D100b3b3c2a：源选面与接触分类

源按 GJK 最后支持顶点的相邻面分别选择 box 与凸包平面，再比较法向吻合度；偏置倾向凸包参考面，法向接近阈值决定面接触或边边接触。非单位缩放使用源 float 选面和 double 平面变换，负缩放及接近零的缩放遵循源形体构造规则。源片段见 `source-facts.json`。

fresh 原生样本包括 Bench 的凸包参考面、box 参考面及 Arch02 的边边接触，分别为 4、4、1 点；请求、原始读数、身份与摘要见 `native-identities.json`。精简样本可由 `python3 tools/import/export_convex_classification_probe.py --source docs/audit/bf144/d100b3b3c2a/classification.native.json.gz --output docs/audit/bf144/d100b3b3c2a/classification.native-samples.json` 重建。

`python3 tools/import/import_contact_face_classification.py --facts docs/audit/bf144/d100b3b3c2a/source-facts.json --native docs/audit/bf144/d100b3b3c2a/classification.native.json.gz --output src/als/settings/contact_face_classification.rs` 可生成目标阈值。旧 reader 未记录该 CVar 或直接 GJK 支持顶点，当前只使用源默认阈值；reader 源码已补采集，统一 Gate 重建后须重跑请求、重生成配置并对比直接顶点和分类，不能把当前推断当作原生验证。

已写参考面偏置、面/边分类、缩放及 Bench 候选控制，另写可消费重建 reader 报告的直接对照。面点与边边生成、测试、消融、全量转换 diff 和 Playground 均待后续。
