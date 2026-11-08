# BF144-D76 cached硬角锁

fresh Chaos源码确认硬锁在子步初始化时从两connector quaternion计算固定Jacobian与相对quaternion分量，每轮以累计DQ求约束误差；硬锁不走角误差阈值。源只在三轴均为soft limited时使用SIMD。fresh PA读取与D65a记录逐字节相同，原生请求、报告、源码片段和身份保留；`target/bf144/23/`旧反例缓存缺失。

目标只在cached Locked轴使用该Jacobian/初始分量，沿用既有一阶归一化DQ路径与约束质量；非cached及soft limited不改。已写15个PA Locked轴的小/大角度与静态/动态parent对照、使用有限差分Jacobian的混合锁对照、真实PhysicsSchedule硬Twist对照。尚未执行测试、消融、完整角色或Playground Gate；180度附近的额外退化边界及硬约束质量极小值仍需与后续D79/验证共同核对。
