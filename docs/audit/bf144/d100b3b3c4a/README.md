# BF144-D100b3b3c4a：源 GJK 支持偏移与剔除

源 box 核心支持点返回 `(sqrt(3) - 1) * margin` 的最大支持偏移；静态凸包有效 margin 为零。源 GJK 将该值作为 `OutMaxSupportDelta` 返回，one-shot manifold 仅在 GJK 分离距离严格大于 `CullDistance + OutMaxSupportDelta` 时剔除。

fresh Bench/foot 原生样本在基础 cull 3 cm 时，按原生 simplex 复算的 GJK 间距约为 3.2 cm 和 3.5 cm；前者保留 4 点，后者为 0 点。box margin 为约 0.6 cm，源支持偏移约 0.439 cm。请求、原始读数和哈希见 `native-identities.json`，数值摘要见 `native-summary.json`。

GJK 结果现携带源支持偏移，已写严格等于边界、越界及两组原生样本控制。旧 reader 不输出直接 GJK 结果；重建后须重跑 `cull.request.json`，以 `tools/tests/test_physics_convex_cull_probe.py` 核对直接 `support_delta_cm` 和分支。几何组合、实际消费、测试、消融、全量转换 diff 与 Playground 待后续。
