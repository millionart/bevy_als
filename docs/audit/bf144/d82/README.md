# BF144-D82 速度约束的预测几何

fresh Chaos源码确认cached solver用本步预测body变换建立connector轴及力臂，速度约束随后复用这些固定量。fresh PA读取与D65a受跟踪输出逐字节相同；原始请求、报告、源码片段和身份存于本目录，旧 `target/bf144/24/velocity/` 缓存缺失。

目标cached线性速度阶段用已捕获的pre-solve旋转更新parent轴和两侧力臂；非cached路径保持原值。已写native spine frame/COM、共同预测旋转0/0.5/1rad及世界预算两次速度迭代的独立源公式对照。测试、消融、实际角色和Playground待统一执行；未将自由旋转积分差异当作此项证据。
