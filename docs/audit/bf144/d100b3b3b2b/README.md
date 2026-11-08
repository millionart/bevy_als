# BF144-D100b3b3b2b 冷 GJK 核心支持查询

源 box 核心沿各轴从外界减去有效 margin；源 convex 将搜索方向转为单精度，按原生顶点顺序以严格 `>` 选择支持点。冷 GJK 从源局部 −X 开始，在双精度厘米空间计算支持差及 D100b3b3b2a simplex，按源距离不再减小和迭代上限结束；距离小于实际 GJK epsilon 时返回待 EPA 状态。源源码及哈希见 `source-facts.json`。

`gjk.native-samples.json` 摘录自 `../d100b3b3b1/gjk.native.json.gz`，保留同一 Bench/foot 在四种核心分离及一种核心重叠下的原生形体、位置、接触和 simplex。已编写支持点平局、四组原生 simplex/法向与重叠分支测试。旧 reader 未提供末次顶点；统一 Gate 须使用 D100b3b3b1 新探针直接核对，不能从 simplex 推断末次索引。

执行 `python3 tools/import/import_contact_gjk_settings.py --facts docs/audit/bf144/d100b3b3b2b/source-facts.json --native docs/audit/bf144/d100b3b3b1/gjk.native.json.gz --output src/als/settings/contact_gjk.rs` 可重建实际 epsilon 和源迭代上限。当前函数尚未接入运行时；热启动、EPA、测试、消融及 Playground 待后续条目和统一 Gate。
