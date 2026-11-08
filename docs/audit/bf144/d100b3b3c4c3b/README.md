# BF144-D100b3b3c4c3b：已有碰撞对补齐源 hull

对已有活动 manifold 的目标 box/compound 碰撞对，现按源资产的全部 hull 顺序执行源 bounds 筛选、几何复用或 GJK 接触生成；仅源实际激活的组进入 solver。新增和删除组使用同一公开约束图同步路径，源未访问的远处 hull 不创建 GJK 热缓存。目标材料从当前同一碰撞对读取，原本只匹配目标 Parry 子形体的路径保留给非源生成场景。

原生 Bench 边缘样本在 2.4 cm 间距时有 hull 1、2，移到 3.5 cm 时只有 hull 1；已写真实 solver 组数、源顺序、原生点及远处 hull 不产生缓存的控制。源报告由 `native-identities.json` 和 D100b3b3c4c1 证据共同引用。

另一次 fresh Bench 中央采样确认源仅激活间距约 2.408 cm 的 hull 2；目标每个 hull 的 AABB 距离下界均大于 0.5 cm，但 compound 整体 AABB 相交，静态推导目标有碰撞对而没有活动 manifold，见 `isolated-summary.json` 与 `target-bounds.json`。已写该边界的目标测试，激活逻辑留 D100b3b3c4c3c。三个精简样本均可通过 `tools/import/export_aggregate_contact_probe.py` 以对应请求标签从本目录或 D100b3b3c4c1 原生报告重建。

目标测试、reader 新字段重采、消融、全量转换 diff 和 Playground 均未执行。
