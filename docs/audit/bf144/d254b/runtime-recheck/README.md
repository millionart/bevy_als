# D254b 移动支撑复验

原生 `MovementBaseUtility::GetMovementBaseVelocity` 只为可移动基座返回速度。旧夹具给静态地面设置 `LinearVelocity`，目标实际支撑速度仍为0。自由余步保留计划速度符合该输入。

夹具改用正式平台相同的Kinematic和 `LinearVelocity`，实际支撑为1 m/s。移动支撑按真实F32位置差回算，两条静止支撑原生余步仍保留计划。四项定向及正式25 ms上坡的18组落地、步长交接、动画恢复通过。见本目录的原反例、恢复日志和运行报告。
