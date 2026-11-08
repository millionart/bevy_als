# BF144-D70 Ragdoll mesh 的 Pawn 响应

fresh B_Als_Character 原生输出记录 CharacterMesh 的 Pawn 为 Ignore，PA Enabled body 在 UE 中与 mesh 响应取较小值；ALS 启动 Ragdoll 只修改 object type 和启用状态。fresh `RagdollTraceGround` 同样使用胶囊的 Pawn channel。原始请求、完整原生输出及源码身份位于本目录；原 `target/bf144/21/initial/` 反例缓存缺失。

通过 `python3 tools/import/import_mesh_pawn_response.py --actor docs/audit/bf144/d70/character.native.json.gz --output src/als/settings/mesh_pawn_ignored.rs` 生成独立目标数据。角色 capsule 标识为 Pawn，仅绑定此角色 mesh 的 rig collider 按目标数据标识忽略 Pawn；角色普通移动、站立净空、Ragdoll 贴地查询、常规物理配对和自有 CCD 均消费该响应，场景物体及其他响应保持原过滤。

已写双角色实际胶囊查询、地面保留、贴地查询、物理配对、CCD 和导入器字段对照；D69 的所有权反例已通过移除 Pawn 标记独立化。全部测试、设计消融与真实 Playground Gate 待统一执行；当前未宣称双方物理轨迹验证通过。
