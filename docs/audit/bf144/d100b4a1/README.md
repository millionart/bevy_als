# BF144-D100b4a1：生产 mesh 源拓扑

fresh 原生读取七个生产三角网格，共 3210 个 cooked 顶点、5115 个三角形；源 `GetTransformedTriangle` 按 cooked 索引读取粒子。目标数据保留源顶点和面索引顺序，位置从源厘米交换 Y/Z 转成运行时米；该坐标基反射记录为 `winding = -1`，不改写原生导出。源码摘录见 `source-facts.json`，请求、完整压缩报告、资产身份和哈希见 `native-identities.json`。

`topology-summary.json` 记录各资产几何哈希、与先前原生读数的一致性及源 float 坐标经目标坐标基往返的静态核对。执行 `python3 tools/import/import_trimesh_contact_topology.py --source docs/audit/bf144/d100b4a1/meshes.native.json.gz --bounds assets/ALSExtras/Environment/Meshes/static_trimesh_bounds.runtime.json --output assets/ALSExtras/Environment/Meshes/static_trimesh_topology.runtime.json` 可重建目标数据；导入器拒绝重复资产、非单位形体及无效索引。

此数据不含 BVH 遍历顺序，旧 reader 不能提供该字段；新 reader 重建和直接次序导入由下一子项处理。已写坐标反射、索引、重复和缩放控制，编译、测试、消融、全量转换 diff 和 Playground 尚未执行。
