# BF144-D100b3b3b3a 源 GJK 热启动

源 `TGJKSimplexData::Restore` 以当前相对变换重新计算缓存支持差，再执行源 simplex 归约；距离严格大于 GJK epsilon 时复用方向和距离，否则清空 simplex 从冷方向重来。目标现有冷 GJK 循环接受同一源缓存并实现该恢复规则；缓存何时创建、失效和与实际约束关联仍待接入子项。

`warm.native.json.gz` 是旧 reader 对五组连续 Bench/foot 位姿的 fresh 原始报告。源在分离和旋转两组没有恢复 manifold，实际重新执行了 GJK；`warm.native-samples.json` 保留输入及原生下一 simplex，用于目标 focused 测试。其余三组恢复了 manifold，不能把旧 reader 的下一 simplex冒称为直接热 GJK 结果。

reader 新增 `capture_warm_gjk`，与冷查询共用同一个原生 simplex 后在下一变换直接查询，测试 `test_physics_warm_gjk_probe.py` 核对输出及实际重跑的两组。统一 Gate 需重建 reader、重采完整冷/热报告并持久化，再通过 `ALS_SOURCE_COLD_GJK_REPORT`、`ALS_SOURCE_WARM_GJK_REPORT` 指定新原生报告，运行对应 Rust focused 测试以核对源末次顶点和结果。EPA、测试、消融和 Playground 均尚未运行。
