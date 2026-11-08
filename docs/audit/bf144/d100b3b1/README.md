# BF144-D100b3b1 源凸包接触拓扑

源 one-shot manifold 使用凸包面的有序顶点裁剪，目标原先只保留凸包点云。`../d100b3a/shapes.native.json.gz` 是逐形体 fresh 原始记录；`source-facts.json` 保存相关源算法、缩放手性与坐标基代码，`identities.json` 保存目标数据身份。

执行 `python3 tools/import/import_compound_contact_topology.py --source docs/audit/bf144/d100b3a/shapes.native.json.gz --collision assets/ALSExtras/Environment/Meshes/static_convex_collision.runtime.json --output assets/ALSExtras/Environment/Meshes/static_convex_topology.runtime.json` 可重建 5 个资产、15 个凸包和 326 个面。数据保留源面索引顺序、顶点、法向、平面点和 margin；坐标转换交换 Y/Z，故目标坐标基的叉乘手性为 −1。对 326 个面静态复算，目标顶点叉乘与转换后的法向均为反向。

源选面还读取每个支持顶点缓存的邻接面；转换器现按源半边构造、反向配对和 twin→next 次序重建 279 个顶点的邻接，数量分布见 `adjacency-derivation.json`。reader 已增加 `convex_core_vertex_planes` 直接读数，重建 reader 后须重新运行原生探针；转换器遇到该字段时必须与重建数据逐项一致。旧版记录不含该字段，目前只完成源代码推导与离线生成，不能冒称已原生核验。

数据目前没有运行时消费者；源裁剪及点序接入分属 D100b3b2/b3b3。已编写坐标基、手性、box 顶点邻接序、原生不一致、无效顶点索引和缺失 shape 测试，运行、全量 AnimBP 转换 diff 与消融留待统一 Gate。
