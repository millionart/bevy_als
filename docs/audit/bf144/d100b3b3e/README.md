# BF144-D100b3b3e：胶囊端帽点序的源依赖

fresh head/Arch 原生聚合的 hull 2 有两个辅助点及末尾 GJK 点；源分支先判断源 GJK、凸包参考面和轴与面法向的夹角，再按 X1→X2 裁线并追加 GJK。目标是否存在两个 feature 点不足以证明源分支成立，因此不能只按目标法向或 feature 编号重排。

该顺序依赖已登记的 D130 胶囊/凸包源几何生成，现先实现 D130 的 GJK、接触点和真实生命周期，再回到本项。请求、完整原生报告、watchdog 结果及校验值见 `native-identities.json`；精简样本可从压缩报告通过 `tools/import/export_aggregate_contact_probe.py` 重建。
