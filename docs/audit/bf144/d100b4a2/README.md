# BF144-D100b4a2：源 mesh BVH 导入合同

fresh 源 `VisitTree` 按节点栈和子节点顺序访问叶面；小查询仅过滤 bounds，不能按三角编号排序。源 BVH 可以使用完整 float 面 bounds，也可以从节点 bounds 和量化字节解码面 bounds，因此仅保存顶点或完整遍历序不足以精确重放查询。源码摘录见 `source-facts.json`。

reader 已增加源 BVH 节点、实际使用的逐面 bounds 和完整遍历序；逐面 bounds 在源遍历的过滤回调中直接读取，覆盖量化与非量化路径。reader 还交叉核对全树访问序与完整 mesh 查询的原生输出。导入器 `tools/import/import_trimesh_bvh_order.py` 核对源几何身份、全排列、节点索引、循环、源访问序及跨姿态/scale 一致性，并保持源序；已写缺字段、量化 bounds、无效引用和顺序冲突测试。

当前旧 reader 没有这些字段，**尚未生成目标 BVH 数据**。统一 Gate 须先重建 reader，对 `docs/audit/bf144/d100b4a1/meshes.request.json` 和 `docs/audit/bf144/d104b3a/scaled.request.json` 重采并保存原始报告及身份，再运行 `python3 tools/import/import_trimesh_bvh_order.py --source <unscaled-report> <scaled-report> --topology assets/ALSExtras/Environment/Meshes/static_trimesh_topology.runtime.json --output assets/ALSExtras/Environment/Meshes/static_trimesh_bvh.runtime.json`。随后运行 `tools/tests/test_import_trimesh_bvh_order.py` 和原生 mesh probe 合同；编译、消融、全量转换 diff 和 Playground 也均未执行。现有输入证据哈希见 `input-identities.json`。
