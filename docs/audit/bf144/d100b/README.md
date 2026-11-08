# BF144-D100b Mesh 与 compound 接触顺序

fresh 原生 PhysicsContactProbe 保留了 12 个 mesh 和 7 个 compound 样本，包含角色形体、环境几何、接触点和 compound 中每个约束的源 shape 身份。`mesh.request.json`、`compound.request.json`、reader watchdog 报告及无损压缩的完整输出共同构成原始证据；原始长度、哈希、压缩哈希和各源资产身份见 `native-identities.json`。两个样本来自修改前的 reader，不含新增字段。

源 `FindOverlappingTriangles` 使用 BVH 同一遍历：查询包围盒只排除不相交分支和面，因此以整个 mesh 包围盒查询所得面序，保留任意较小查询的相对次序。reader 现记录 `trimesh_core_bvh_face_order`，并要求它是原生三角形编号的完整排列；`tools/tests/test_physics_mesh_contact_probe.py` 还检查跨姿态的同资产顺序。生成接触时源还会执行两轮共享边/顶点处理及替换，因此 BVH 面序本身不能直接代替最终接触点序。

本轮未重建 reader。统一 Gate 时先重建，再以本目录请求重跑 reader 和 mesh probe 测试，保存带新字段的 native 原始输出及身份，再开展依赖该字段的目标侧验证。目标 Avian 的 manifold 构造丢弃 Parry 子形体编号，compound hull 和 mesh 三角形身份恢复与接触生成顺序属于 D100b2，尚未实现。
