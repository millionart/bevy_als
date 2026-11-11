# D162a 原生复杂网格射线数据

`TriangleMeshImplicitObject.h:248–310` 以固定子节点次序和栈遍历树。原生 reader 导出实际 cooked 粒子、面、分支和已解码面边界，并验证展开的面次序。源 contact 入口按 `CollisionFilter.h:115–126` 要求网格可用于物理碰撞，首轮因此无法覆盖仅复杂查询的网格。GeometryQueryProbe 复用同一导出逻辑，从实际 query shape 取数据。Cart 的 contact 与 ray 导出五组几何字段逐值相同。

12 个 Playground 网格共 5811 个顶点、10133 个面和 651 个分支节点。离线数据将源厘米顶点映射到目标轴，保留射线内核所需的厘米值、cooked 面次序和边界。每个分支的范围与覆盖都在导入时检查，反转源顺序会失败。

八组原生边界控制的访问顶点和相对次序均可从导入数据重建。Arch 共享边依次接收面 50、231，选择 231。三个 Fountain 顶点用例和 Arch 共享边均受严格最近策略影响，四个内点控制保持同一面。

```sh
pwsh -NoProfile -File tools/build_ue_source_reader.ps1
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALSExtras/Environment/Meshes/SM_Als_Fountain_01 -GeometryQueryProbe docs/audit/bf144/d162a/mesh-data.request.json -Output docs/audit/bf144/d162a/mesh-data.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALSExtras/Environment/Meshes/SM_Als_Fountain_01 -GeometryQueryProbe docs/audit/bf144/d162a/edge-visits.request.json -Output docs/audit/bf144/d162a/edge-visits.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/PA_Als -PhysicsContactProbe docs/audit/bf144/d162a/contact-control.request.json -Output docs/audit/bf144/d162a/contact-control.native.json
python tools/import/import_playground_mesh_ray_data.py --source docs/audit/bf144/d162a/mesh-data.native.json.gz --rules assets/ALSExtras/Environment/Meshes/playground_mesh_ray_culling.runtime.json --output assets/ALSExtras/Environment/Meshes/playground_mesh_ray_data.runtime.json --audit docs/audit/bf144/d162a/mesh-data.audit.json
python docs/audit/bf144/d162a/verify_cooked_ray_data.py
```

用新读取的报告生成时，`--source` 使用原始 `.native.json`。D162b 接入实际 Rig 射线和控制。
