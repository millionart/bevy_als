# BF144-D100b3b3d：胶囊与 compound 的圆柱端点顺序

fresh head/Arch01 原生聚合含两个 hull：hull 0 有两个圆柱接触点，轴与法向点积绝对值约 `3.0e-16`；hull 2 有三个端帽/辅助点，对应约 `0.584`，不属于圆柱分支。源 `GenerateLineConvexManifoldClippedVerticesSameSpace` 初始按胶囊 X1→X2 排列并分别更新被裁端点；当前胶囊目标构造及 Parry segment feature 的端点方向相反。请求、原生报告和哈希见 `native-identities.json`，两侧源码摘录见 `source-facts.json`、`target-facts.json`。

复用 D100a 的同一圆柱端点排序，扩展至带源资产标记的 compound；仍只在 ragdoll 且轴与 manifold 法向满足源圆柱阈值时处理。端帽分支和非源 compound 保持原路径，端帽辅助点几何仍归 D130。

已写 head/Arch 真实碰撞入口的两种实体顺序，以及无源标记、普通 body 和非圆柱接触的受控对照。目标编译、测试、消融、全量转换 diff 和 Playground 均待统一 Gate；精简样本可从原生压缩报告通过 `tools/import/export_aggregate_contact_probe.py` 重建。
