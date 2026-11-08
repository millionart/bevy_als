# BF144-D80b cached锁定锚点projection

fresh Chaos源码确认projection前提交并清零全部body修正，然后所有joint先初始化缓存，再按全joint外层迭代执行teleport和projection；位置约束以parent固定、child最小局部逆惯量的球形惯量计算三轴，最后将累计projection DP/DQ按源系数计入速度。当前PA位置projection alpha=1、角projection alpha=0；fresh PA读取与D65a受跟踪输出逐字节相同。原请求、报告、源码片段和身份已保留，旧 `target/bf144/24/full_initial/` 缓存缺失。

目标cached路径先冻结所有joint的已提交几何，再逐轮按源三轴和共同起点合计child位置/角增量；teleport、速度交接及零角响应也保留。非cached路径继续沿用已有实现。已写已提交位姿、单轴/多轴、teleport、关节链、零惯量及PhysicsSchedule对照；测试、消融、完整角色和Playground待统一执行，混合cached/noncached joint的跨solver顺序未单独验收。
