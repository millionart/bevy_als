# D162b 原生候选顺序进入实际 Rig 射线

`SceneQuery.cpp:511–515` 使用 float 射线长度。`TriangleMeshImplicitObject.h:140–157,248–310` 对 BVH 边界和收紧后的长度用 float，沿固定分支顺序访问面。`Triangle.h:375–418` 用 double 厘米坐标和 1e−8 重心容差，`TriangleMeshImplicitObject.cpp:181–188` 允许同一 float 长度内的后续候选覆盖结果。

实际 Playground 网格通过 Rig 快照消费 D162a 的 cooked 数据。每次射线使用原生分支、面边界和精确三角形。缩放包装沿用原生 float 起点、方向及法线变换，输出前回到世界米。单面标志与 cooked 数据合并到同一查询记录。

静态重放逐访问核对 38 条原生射线，包含默认 FootOffsetTrace 距离的八条边界/内点射线、D159 端点和 D161 桶内/正面。严格最近、严格重心、保留 double 距离、将顶点先降为米制 float、取消背面剔除的消融分别改变 7、2、4、27、2 条输出。Arch 共享边选择后访问的面 231。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALSExtras/Environment/Meshes/SM_Als_Fountain_01 -GeometryQueryProbe docs/audit/bf144/d162b/rig-edges.request.json -Output docs/audit/bf144/d162b/rig-edges.native.json
python docs/audit/bf144/d162b/replay_cooked_ray_kernel.py
```

`cooked_mesh_faces_and_foot_offsets_match_native_queries` 核对实际查询入口的面、位置和法线，并通过 FootOffsetTrace 验证八条默认距离的偏移。Rust、消融恢复和 Playground Gate 待统一验证。
