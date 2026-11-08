# BF144-D100b3b3c4c1：源 cull 与热启动配置

fresh PhysicsSettings 的 `CollisionCullDistance` 为 3 cm，与既有受跟踪原始报告逐字节一致；当前 PA 原生 CVar 指示 solver cull 覆盖为 −1、GJK 热启动为 true。源 midphase 以动态粒子整体局部包围盒的最大轴长度计算缩放，再将世界 cull 距离乘以该缩放。普通非 MACD 路径还根据积分前 PreV 的最大轴速度和物理 delta 扩展距离；当前倍率为 1、上限为 3 cm。已离线生成配置并实现同一计算，多个 collider 使用同一刚体局部边界；目标窄相位在速度积分前运行，读取对应的速度。源代码、请求、原生报告与哈希见 `source-facts.json` 和 `native-identities.json`。

fresh Bench/foot 聚合样本中，源按顺序激活 hull 1 和 hull 2；hull 2 最小 phi 约 2.4 cm。向外移动后只有 hull 1。目标静止且无附加 margin 时，Avian 的窄相位预测距离由源码可知为 0.5 cm，因此不能生成间距 2.4 cm 的 hull 2；这是静态源码与源原生读数的推论，目标真实测试尚未运行。已写按原生点序检查该缺组的测试，修复留 D100b3b3c4c3。

运行 `python3 tools/import/import_contact_lifecycle_settings.py --facts docs/audit/bf144/d100b3b3c4c1/source-facts.json --settings docs/audit/bf144/d81/settings.native.json --native docs/audit/bf144/d100b3b3c4a/cull.native.json.gz --output src/als/settings/contact_lifecycle.rs` 可重建配置；`python3 tools/import/export_aggregate_contact_probe.py --source docs/audit/bf144/d100b3b3c4c1/aggregate.native.json.gz --label bench_aggregate_within_cull --output docs/audit/bf144/d100b3b3c4c1/aggregate.native-sample.json` 可重建精简样本。旧 reader 未记录 cull 缩放 CVar 与约束实际 cull，当前缩放常量暂用源默认值；reader 已补字段和可指定 PreV 的请求，重建后须重采并执行 `tools/tests/test_physics_convex_candidate_probe.py` 与 `tools/tests/test_physics_convex_velocity_cull_probe.py`。

真实接触生成、缺组恢复、测试、消融、全量转换 diff 和 Playground 均待后续。
