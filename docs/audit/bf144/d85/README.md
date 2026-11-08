# BF144-D85 cached软角阻尼的预测旋转

fresh Chaos源码确认scalar/SIMD软限位以及角drive阻尼均使用初始与预测connector quaternion的差分，再加本轮累计DQ；该差分不是原始W×dt。fresh PA读取与D65a受跟踪输出逐字节相同，请求、报告、源片段及身份已保存，旧 `target/bf144/25/soft_initial/` 缓存缺失。

目标cached软角修正从已有pre-solve quaternion得到源预测差分，继续加累计DQ；非cached及线性drive不改。已写spine SIMD与neck scalar在30/60Hz、多个正负速度、零阻尼及角drive的源公式对照。D86仍负责源单步物理时钟；测试、消融、完整角色与Playground待统一执行。
