# BF144-D100b3b3b1 原生冷 GJK 末次支持顶点

源 GJK 在每次核心支持查询时覆盖顶点索引，最后一次索引不一定仍在最终 simplex 中。reader 现可在 `capture_cold_gjk=true` 且 box/convex 单位缩放时，用同一源形体、约束有效 margin、相对变换和运行中的 GJK/EPA epsilon，直接调用源 `GJKPenetrationWarmStartable`，记录末次支持顶点、最近点、法向和 simplex。`test_physics_cold_gjk_probe.py` 对照原约束的 simplex、法向和有效索引。

`gjk.request.json` 包含同一 Bench/foot 的五个位置；旧版 reader fresh 原生结果与 watchdog 报告完整保留。`core-distance-evidence.json` 根据原始 simplex 的加权差记录四个核心分离和一个核心重叠。旧二进制忽略新增的 `capture_cold_gjk` 字段，现有报告不含末次顶点，不能将 simplex 中的顶点代替它。

统一 Gate 必须先重建 reader，按本目录请求重跑并持久化包含 `cold_gjk` 的完整原生报告和哈希，再运行 focused 测试；热启动、缩放凸包及 EPA 的完整行为仍由后续子项处理。
