# BF144-D100b4b1a：源 mesh BVH 查询

fresh 源 `FTrimeshBVH::VisitTree` 从根节点入栈，按 child 0→1 检查；叶面立即访问，内部节点入栈后后进先出。节点及面 bounds 与查询 bounds 都按 float 厘米进行闭区间相交。源空树返回空；代码摘录与哈希见 `source-facts.json`。

已按源顺序消费导入的节点和有效面 bounds，不用面编号排序；循环节点或无效引用返回缺失，节点访问次数由源节点数有界。已写全树、子区域、float 边界、节点裁剪、空树和循环/缺面控制；循环控制也是该有界遍历的 focused watchdog 测试。

reader 新增可选 `mesh_bvh_query_bounds_cm`，直接输出源子区域面序。`bvh-query.request.json` 为 Rock 和 Stairs02 各准备全域、半域、窄切片及外部查询；输入身份见 `input-identities.json`。旧 reader 不支持新字段，统一 Gate 须重建并保存原生查询报告，然后运行 `tools/tests/test_physics_mesh_bvh_query_probe.py`，并设置 `ALS_MESH_BVH_NATIVE_REPORT` 与 `ALS_MESH_BVH_TARGET_DATA` 运行 Rust 原生对照。实际 BVH 目标数据、查询 bounds 构造、全量转换 diff、编译、测试、消融和 Playground 均待后续。
