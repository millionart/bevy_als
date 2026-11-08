# BF144-D100b3b3c4c3c3a：共享源凸包生成

fresh 源代码再次确认，shape pair 先尝试复用已有 manifold；复用失败才重新执行窄相位，最后按 phi 和 cull 激活约束。源摘录与哈希见 `source-facts.json`；Barrel 内外边界沿用 D100b3b3c4c3c1 的原生报告。

已有逐 hull bounds、复用、GJK、点生成和缓存更新现由一个函数返回完整组与缓存；原有求解路径调用同一函数。已写独立 Barrel 原生组数与缓存身份测试，原 Bench 和 Arch 求解器测试继续覆盖调用路径。实际窄相位接入仍在下一子项，所有编译、测试、消融、全量转换 diff 和 Playground 待统一 Gate。
