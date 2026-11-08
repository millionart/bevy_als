# BF144-D100b3b3b3b1 源 EPA 面与见证点

源 EPA 三角面由两个核心支持点之差构造，使用双精度面积阈值，保留面法向、原点有符号距离和相邻面/边；面翻转同步更新邻接。见证点经源 simplex 归约得到，接近原点时使用面法向，源平面距离为负时保留负穿透。`source-facts.json` 保留面、可见边界和见证计算的源代码及哈希。

`epa.native-simplex.json` 摘录 fresh Bench/foot 核心重叠的原生四点 simplex，`face-evidence.json` 根据源面公式静态复算四个初始面；完整原始报告和身份见 `../d100b3b3b1/`。已编写正负面、原点接触、退化三角形和四面体翻转邻接测试。

执行 `python3 tools/import/import_contact_epa_settings.py --facts docs/audit/bf144/d100b3b3b3b1/source-facts.json --native docs/audit/bf144/d100b3b3b1/gjk.native.json.gz --output src/als/settings/contact_epa.rs` 可重建面、初始边、可见性、见证阈值、限制和 native EPA epsilon。初始多面体实现见 D100b3b3b3b2；扩展、测试、消融和 Playground 待后续条目及统一 Gate。
