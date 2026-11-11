# D161 Playground 网格射线的背面剔除

`ChaosInterfaceUtils.cpp:298–325` 用 BodySetup 的双面字段设置复杂网格剔除。`TriangleMeshImplicitObject.cpp:154–187` 在三角形射线前用局部方向与面法线的点积判定背面。`ImplicitObjectScaled.h:635–648` 把缩放网格的方向转换到未缩放形体。

12 种实际 Playground 复杂网格的原生 BodySetup 均为单面。以原生 collision 顶点、flip_normals 和目标 glTF 坐标基复算，10133 个三角形及其绕序全部逐值匹配。离线生成的 runtime 表携带各资产的剔除标志，随复杂查询形体进入实际 Rig 快照。

桶内朝下的原生射线在单位尺度和 1.5 倍尺度均剔除底面候选。移除剔除后，两组会在约 76.195 cm 和 74.293 cm 命中。正面射线的单位及非均匀尺度控制均命中。将原生双面字段改为 true 时生成关闭剔除的值，反转源绕序会使导入检查失败。

```sh
for mesh in Arch_01 Arch_02 Barrel Barrel_Broken Bench Cart Crate_Broken Fountain_01 Rock Stairs_01 Stairs_02 Stairs_03; do
  pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset "/ALS/ALSExtras/Environment/Meshes/SM_Als_$mesh" -Output "docs/audit/bf144/d161/$mesh.native.json"
done
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALSExtras/Environment/Meshes/SM_Als_Barrel -GeometryQueryProbe docs/audit/bf144/d161/barrel-ray.request.json -Output docs/audit/bf144/d161/barrel-ray.native.json
python tools/import/import_playground_ray_culling.py --source docs/audit/bf144/d161/[A-Z]*.native.json.gz --mesh-root assets/ALSExtras/Environment/Meshes --output assets/ALSExtras/Environment/Meshes/playground_mesh_ray_culling.runtime.json --audit docs/audit/bf144/d161/geometry.audit.json
python docs/audit/bf144/d161/replay_mesh_ray_culling.py
```

用新读取的报告生成时，将 `--source` 的后缀改为 `.native.json`。

加载角色的复杂几何用例从 Playground bridge 取真实桶网格，核对两种尺度及同一场景移除、恢复剔除标志。Rust、消融恢复和 Playground Gate 待统一验证。
