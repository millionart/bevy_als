# BF144-D100b3b3c4c3c3c：源组序到实际求解槽

重新读取的源实现按两侧 shape 数组顺序构造 detector，并按该顺序逐个生成约束。Avian 先按 manifold handle 查找接触组、按颜色构造约束，求解时先遍历 overflow，再依次遍历其他颜色和各颜色内的槽。源和目标代码摘录见 `source-facts.json`、`target-facts.json`。

源凸包按原生 hull 索引生成，窄相位结束后以同一个 `source_solver_slots` 映射放回 Avian 实际消费顺序；逐 hull 历史仍按碰撞对、hull 和资产 tick 关联。原先另写的一份同样排序已合并。Bench 原生样本中的 [1, 2] → [1] → [1, 2] 转换现有真实入口测试，核对每步缓存身份、solver 槽法向和持续接触事件；原生身份和哈希引用见 `native-identities.json`。

上述是静态对照和已编写的测试，不代表目标测试已通过；编译、测试、消融、reader 新字段重采、全量转换 diff 和真实 Playground 待统一 Gate。
