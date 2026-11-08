# BF144-D104b3b：跨三角组的源复用判定

源大型三角网格将各点放在同一约束中，每点可有不同法向，复用要求整个约束至少保留 `min(初始点数, 4)` 个有效点。目标多个接触组因此必须合并判断，而不能让每个最多四点的组分别决定是否重建。`source-facts.json` 保存源完整判断、网格大 manifold 条件与目标分组事实。

fresh 原生 Cart/foot 有 14 点、Stairs02/foot 有 8 点，各含两种法向；两者位移 2 mm 时全部复用，6 mm 时重建。原始压缩报告、请求、watchdog、样本摘要、源资产身份和哈希见 `native-identities.json`；精简样本可用 `python3 tools/import/export_contact_restore_probe.py --source docs/audit/bf144/d104b3b/restore.native.json.gz --output docs/audit/bf144/d104b3b/restore.native-samples.json` 重建。

已实现跨组统一复用合同；新增一个组只剩三点但两组合计七点仍应成功、总计三点应失败、形体替换应失效的最小控制，并将四组原生样本加入现有逐点测试。真实目标分组写回和 solver 槽重组留待 D104b3c；全量转换 diff、编译、测试、消融和 Playground 待统一 Gate。
