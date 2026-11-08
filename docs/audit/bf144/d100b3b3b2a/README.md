# BF144-D100b3b3b2a 源 GJK simplex 归约

源 `SimplexFindClosestToOrigin` 使用双精度线段最近点、三角形投影及 Cramer 余子式、严格符号匹配、四面体子面比较，随后按选中索引同步重排差向量、两侧支持点和权重。`source-facts.json` 保存完整相关源片段和哈希；本项独立移植该合同，尚未接入支持查询。

`simplex.native-samples.json` 摘录自 `../d100b3b3b1/gjk.native.json.gz` 的五组 fresh 原生结果：四组三点、一个核心重叠四点。已编写逐权重、最近点和两侧支持点顺序对照，以及线段退化与四面体内部控制。编译、测试、消融和真实入口验证留待统一 Gate。
