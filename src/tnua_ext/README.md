# Tnua 相关辅助

`motion.rs` 提供水平运动计算，`crouch_transaction.rs` 提供胶囊姿态事务，`source_physics_timing.rs` 接入 Tnua/Avian 的角色整帧积分与普通刚体封顶时钟。

ALS 的控制方案、运动参数、步阶和状态策略对标 `AlsCharacterMovementComponent`，放在 [`als/character/movement`](../als/character/movement/)，不把整套 ALS 策略归为通用 Tnua 扩展。时钟适配目前仍消费项目角色状态；上游提取时应按实际依赖分离策略。
