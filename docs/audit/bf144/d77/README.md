# BF144-D77 角容差与物理子步

fresh Chaos cached solver以 `min(1, 60*60*dt*dt)` 缩放原始角容差，并在drive初始化及角limit判断消费；原生PA配置的角容差为0.001，fresh读取与D65a记录逐字节相同。源码片段、请求、报告和身份保留，旧临时反例缓存不可重验。

目标使用传入solver的实际子步dt调整cached drive和limit容差；硬锁保留D76的无角容差路径，非cached路径不改。已写30/60/120/240Hz角误差、零角阻尼和真实PhysicsSchedule对照；SIMD同时更新仍属于D78。测试、消融、实际角色和Playground尚待统一执行。
