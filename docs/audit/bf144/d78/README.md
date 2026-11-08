# BF144-D78 三轴soft限位同时更新

fresh Chaos源码与PA原生配置确认当前linear solver启用SIMD，仅三轴均Limited且twist/swing soft时，从同一轮起点计算各轴lambda并合计DQ后写回。fresh PA读取与D65a受跟踪输出逐字节相同；源码片段、请求、报告及身份存于本目录，旧 `target/bf144/23/` 反例缓存缺失。

目标复用单轴误差与弹簧计算，仅在该源配置下暂存三轴角增量并统一写回；Locked、非cached和非全soft路径仍顺序执行。已写真实spine_01参数、耦合/无耦合惯量、单轴/混合轴、1/6迭代及PhysicsSchedule对照；非零初始角速度的源预测quat阻尼差属于D85。测试、消融、完整角色和Playground均待统一执行。
