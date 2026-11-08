# BF144-D100b3b3c1：运行时源凸包拓扑

源选面和裁剪消费有序凸包顶点、面、顶点邻接面与缩放手性。fresh 读取 Bench hull 2、Arch01 hull 4 与 Broken Barrel hull 0，顶点、面索引、平面和 margin 均与现有离线拓扑一致；请求、原生报告、资产身份、比较结果和哈希见 `native-identities.json`。

运行时现在按源资产名和 hull 索引加载同一份拓扑，源 bounds 消费者也复用该加载结果，避免重复解析。已写 Bench 原生数据、全部 15 个 hull 的索引结构及无效身份控制。旧版 reader 不提供直接邻接面读数；此前增加的 reader 字段需在统一 Gate 重建后重采并与离线重建结果对比。

面接触、边边接触及真实生命周期接入属于后续子项；编译、测试、消融、全量转换 diff 和 Playground 均未执行。
