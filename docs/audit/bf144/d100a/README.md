# BF144-D100a 水平胶囊与箱体端点顺序

fresh PhysicsContactProbe保留head的原生胶囊/box接触：源先输出胶囊X1，再输出X2。源shape构造把X1放在局部负Z，坐标基变换后对应目标胶囊局部负Y；native端点与目标生成shape轴的方向点积为1。源只在轴与接触法向点积绝对值小于0.01的圆柱分支使用这条线段顺序。原生请求、完整无损报告、哈希和source/target片段已保存。

目标在narrow phase后、约束准备前，仅对含Ragdoll的capsule/cuboid圆柱接触按源X1→X2顺序排列完整ContactPoint，保留点上的缓存字段；其他形体及非圆柱分支不改。执行 `python3 tools/import/import_capsule_contact_axis_threshold.py --facts docs/audit/bf144/d100a/source-facts.json --output src/avian_ext/capsule_contact_axis_threshold.rs` 可重建目标阈值；执行 `python3 tools/import/export_capsule_contact_order_probe.py --source docs/audit/bf144/d100a/contact.native.json.gz --profile assets/ALS/Character/PA_Als.physics.json --sample head_0_-0.1_wide --bone head --output docs/audit/bf144/d100a/axis-evidence.json` 可重建轴与端点证据。

已写native端点、两种collider顺序、阈值边界和加载head实际manifold对照。目标多余中点及非水平几何归D101，mesh/compound顺序归D100b；测试、消融与Playground待统一Gate。
