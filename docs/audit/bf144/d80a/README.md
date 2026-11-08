# BF144-D80a cached锁定锚点位置

fresh Chaos源码确认三轴Locked的SIMD位置阶段缓存parent connector轴、两侧力臂及各轴有效质量，并以同一轮的DP/DQ计算三轴lambda后合计写回。fresh PA读取与D65a受跟踪输出逐字节相同；请求、报告、源码片段及身份保留，旧 `target/bf144/24/initial/` 缓存缺失。

目标保留非cached球形约束路径，cached三轴锁定改按固定轴和累计DP/DQ同时更新，并将位置冲量交给既有速度阶段。已写无/有力臂、旋转frame、静态/动态parent、1/6迭代、native spine参数和PhysicsSchedule对照；source参考为公式复算，不冒称native Chaos仿真。projection仍在D80b，测试、消融及完整角色/Playground待统一执行。
