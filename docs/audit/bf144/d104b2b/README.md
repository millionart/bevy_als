# BF144-D104b2b：逐凸包接触复用

源 shape-pair 为每个凸包保存独立约束：只有无法复用时重新运行窄相位；睡眠期间保留历史。fresh Arch01/foot 的 hull 0、2 分别测试 ±2 mm 和 6 mm 位移，四个小位移复用、两个大位移重建；小位移下源保留点与新生成点相差约 2 mm。请求、原生报告、摘要、精简样本及哈希见 `native-identities.json`。精简样本可用 `python3 tools/import/export_contact_restore_probe.py --source docs/audit/bf144/d104b2b/restore.native.json.gz --output docs/audit/bf144/d104b2b/restore.native-samples.json` 重建。

目标在修改原始法向或交换求解槽前提取 hull 索引，以接触 ID、hull 索引和资产标记变更 tick 分离缓存；形体或 body 变化时重建。成功复用保留旧点和法向，不再调用核心 GJK；失败时重新计算并缓存当前法向与表面点，最后按既有源组序分配 solver 槽。单 box 与 compound 共用复用合同，各自缓存隔离。

当前接入要求目标已有能唯一对应的活动 manifold；身份歧义或目标缺失组仍由 D100b3b 等后续条目处理，trimesh 属于 D104b3。已写原生双 hull 复用/重建、真实多 hull 消费、资产标记变化、形体替换与原 box 控制；构建、测试、消融和 Playground 待统一 Gate。
