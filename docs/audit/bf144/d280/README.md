# BF144-D280 VelocityBlend 欧氏归一化

`source-facts.json` 记录 fresh ALS/UE 的局部速度、欧氏归一化、阈值及后续 L1 分配。`root.native.json.gz` 是本轮重读的主动画资产，`native-identities.json` 保留源资产、原始/压缩字节哈希和 watchdog 报告；其余递归动画资产引用 D286，恢复后身份核对见 D281。

`precision-witness.json` 从 D282 的真实第101次查询局部向量重算 F32 欧氏归一化及 L1 分配，保留去除此阶段的一步差异。单位转换时点仍归 D268。图范围见 `tools/animgraph_authoring/bf144_d280.scope.json`。

Focused 测试、消融、图静态审计及公共视觉 Gate 按本轮要求待全部条目实现后执行。
