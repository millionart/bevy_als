# BF144-D241 Mesh Space Additive 组件积

fresh 本地 Head 节点原生控制包含 current/current 与 source/source 的满权重，以及 current 输入的零权重和半权重；各有 79 骨、26 曲线，零权重完整保留 79 骨输入。UE 源码先把 local 旋转转换到 mesh 空间，按权重累积组件积，不在此处归一化，再从子到父还原 local；`AccumulateAdditivePose` 和节点末尾的 local 归一化是之后的独立阶段。原生请求、结果、坐标字段、源摘录与哈希保存在本目录。

目标把原共享操作拆为组件累积与末尾归一化：mesh 路径直接使用组件积，local additive、组合与 root motion 仍使用原有归一化结果。最小非单位积测试锁定被删除的阶段；`head-product-fixture.json` 保留实际 Head 祖先链和先前三阶段方法控制，供公开 mesh 路径检查输出确实脱离旧组件归一化结果。D248b1 的原历史方法 fixture 仍以同一组输入和三阶段受控运算检验 reciprocal 方法，但不再错误地把该历史阶段顺序当作当前公开路径。

源 F64 与目标 F32 的其余差异、满权重 delta 预处理 D170 和最后一次 local 归一化 D231 独立保留。上述测试、消融、完整图转换对比和公共 Playground Gate 均待全部静态修复及 Bevy/依赖升级后执行，未声称姿态已整体一致。
