# BF144-D75 角驱动采样先于启动限速

fresh ALS `RefreshRagdolling` 先从骨盆物理体保存完整速度，按该速度设置角驱动，最后执行启动限速。fresh PA原生读取与D65a记录逐字节相同，native Chaos角驱动刚度系数为1.5；源码片段、请求、报告和身份保留在本目录。原 `target/bf144/22/` 反例缓存缺失。

目标调整现有物理交接链，让角驱动adapter在限速system前读取同一骨盆速度，再按原有约束系数写入motor。新fixture在真实加载角色中覆盖0、1、5、12m/s与限速启用/结束，并检查19个关节57个motor以及限速结果；测试、消融、完整物理轨迹和真实Playground待统一执行。
