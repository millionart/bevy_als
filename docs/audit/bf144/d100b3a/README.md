# BF144-D100b3a Compound 约束组顺序

源小型平铺 shape pair 按两侧形体数组顺序构造检测器，每帧重新激活各约束。目标 Playground 从源凸包数组构造 Parry compound，Parry 使用数组索引作为子形体编号；Avian 对同一动态 body 的各 manifold 分配不同颜色，先消费 overflow，再消费从低到高的颜色及颜色内槽位。

`shapes.request.json` 使用旧版已构建 reader fresh 读取 15 个源凸包，原始输出和 watchdog 报告均保留；旧版缺少 D100b1 的 BVH 字段，但本请求只读取凸包。`shape-identities.json` 可用 `python3 tools/import/compare_compound_shape_identity.py --source docs/audit/bf144/d100b3a/shapes.native.json.gz --target assets/ALSExtras/Environment/Meshes/static_convex_collision.runtime.json --output docs/audit/bf144/d100b3a/shape-identities.json` 重建。三个多凸包资产的 13 个原生凸包与对应目标点集的双向最大距离均小于 8e-9 米；单凸包破损木桶源有 48 个顶点、目标输入有 50 个，目标到源最大顶点距离约 1.35 毫米，尚未证明经凸包构造后表面不同，本项不需要排列单组。

真实 Playground 的多凸包标记在窄相位末按与 Avian 相同的预测距离查询身份，在身份唯一时把完整 manifold 分配到实际求解槽，不改变颜色、约束句柄或组内点。`arch-foot.native-sample.json` 是 `../d100b/compound.native.json.gz` 内 Arch01/foot 样本的 input、geometry 和 aggregate_contacts 摘录，用于 focused 测试；完整原始源记录仍保留。其他场景、无法唯一匹配的身份及组内点序分别保持原样或留待 D100b3b；测试、消融和真实入口验证待统一 Gate。
