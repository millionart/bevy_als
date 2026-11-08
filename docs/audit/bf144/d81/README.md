# BF144-D81 世界Ragdoll的solver迭代预算

fresh native `/Script/Engine.Default__PhysicsSettings` 给出世界position/velocity/projection为8/2/1；fresh PA控制台覆盖均−1，21个body及fresh CharacterMesh均未启用迭代覆盖。UE `PhysicsAsset::SolverSettings` 明确属于RigidBody Anim Node，世界路径由PhysicsSettings进入scene solver并在island缺省时使用。三个原生请求/报告、设置输出、角色/PA证据和source身份均保留；旧反例缓存不可用。

通过 `python3 tools/import/import_world_solver_iterations.py --settings docs/audit/bf144/d81/settings.native.json --physics docs/audit/bf144/d65a/physics_asset.native.json.gz --character docs/audit/bf144/d81/character.native.json.gz --output src/als/settings/world_solver_iterations.rs` 生成独立世界目标数据；不改PA原始导出或动画节点预算。仅世界Ragdoll模板将三项预算传给body和joint。已写native值、实际加载角色19个joint、以及1/2次速度求解源公式对照；其他actor启用per-body island覆盖的情形未在当前源资产范围内实现或验收。测试、消融、全量转换和Playground均待统一执行。
