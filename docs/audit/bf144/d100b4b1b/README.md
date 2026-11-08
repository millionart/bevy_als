# BF144-D100b4b1b：源 mesh 查询范围与三角筛选

fresh 源优化路径先对凸体局部包围盒按 cull 距离扩展，在 double 厘米下逆平移、逆旋转和逆缩放得到 mesh 局部查询范围；BVH 用 float 范围筛选，随后把候选三角变换回凸体空间，以扩展后的凸体包围盒做第二次闭区间相交。负缩放行列式为负时，源交换顶点及索引的后两项。mesh 顶点是源 float，变换前须还原其 float 值。相关源码摘录与哈希见 `source-facts.json`。

已实现查询范围、对 b1a 的 BVH 查询、二次筛选、源点序和反射绕序；调用方须提供源 `UnwrapImplicit` 后实际使用的 scale，而不是假定缩放始终为正。已写八角点独立逆变换、二次筛选和边界、负缩放及损坏拓扑测试。后续三角接触生成消费这些结果，运行时接线留 b4c。

reader 已新增可选 `capture_mesh_triangle_query`：直接调用源逆变换、`VisitTriangles` 和源 AABB 相交并记录范围、身份、顺序与变换后顶点。请求 `mesh-query.request.json` 包含 box/capsule、旋转、正负缩放和远离 mesh 的样例；其中两个未旋转 box 使用已记录的生产资产接触位置。旧 reader 不包含新字段；统一 Gate 须重建 reader、保存这六项原生输出及身份，运行 `tools/tests/test_physics_mesh_triangle_query_probe.py`，并提供 `ALS_MESH_QUERY_NATIVE_REPORT` 和 `ALS_MESH_BVH_TARGET_DATA` 运行 Rust 原生对照。BVH 目标数据、编译、测试、消融、全量转换 diff 和 Playground 均待统一 Gate。
