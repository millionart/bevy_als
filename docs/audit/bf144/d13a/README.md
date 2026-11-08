# BF144-D13a 当前坡面切向扫掠

fresh UE `ComputeGroundMovementDelta` 和 `MoveAlongFloor` 显示：当前地面是非 line 的可行走坡面、几何与接触法线均满足条件时，将水平请求投影为沿坡位移，再先做碰撞扫掠；本角色 `bMaintainHorizontalGroundVelocity=True`。fresh 原生 40° 坡后续帧向见证墙的实际扫掠请求水平约 18.974 cm、升高约 15.921 cm，与坡面切向一致。ALS PhysWalking 随后独立复查和调整 floor。原生请求、结果、时序、源码和诊断 reader 身份保存在本目录。

目标地面运动先按当前 floor 的接触/几何法线计算切向扫掠；无阻挡时单步 CMC 直接消费已扫掠位移，避免物理阶段重放水平请求，多子步事务消费同一结果。已有碰撞后的剩余 StepUp 输入仍为源水平位移。已写源条件、20°/40° 坡和平行低顶、加载角色单步消费对照；首次撞上新坡面后的剩余时间处理留给 D13b。旧 `target/bf144/05/ramp_loaded/` 报告已缺失；测试、消融、图审计和真实 Playground Gate 待全部静态修复及 Bevy/依赖升级后执行。
