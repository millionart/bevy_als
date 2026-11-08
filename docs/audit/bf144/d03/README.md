# BF144-D03 Turn 曲线与速度方向目标

本地 ALS 在静止 Grounded 更新中先应用 `RotationYawSpeed × DeltaTime`；仅当角度增量绝对值超过 `UE_SMALL_NUMBER` 时，它从更新后的 actor 世界朝向同步 target 与 smooth target。随后 blocked 的 VelocityDirection 分支读取该目标。fresh Standing 蓝图读取到 `TurnInPlaceStanding` slot；fresh `A_Als_Turn_180_Right` 原生动画采样在 0.2 秒和 0.3 秒分别给出约 50.555 和 119.534 度/秒。原始源记录、时间请求、源码与哈希均在本目录，压缩数据已逐字节核对。

目标在实际应用大于源阈值的曲线增量后，先同步 target，再执行原有平滑及模式分支；零曲线和零 delta 保留原目标。新增独立响应 fixture 覆盖正反曲线、零曲线、零 delta 和从 ViewDirection 切到 VelocityDirection；加载角色 fixture 排队实际 Turn、使用真实切换输入，并在曲线消费前后比较世界 yaw。该 Turn 由受控请求排队，尚不声称完整自然触发链通过。

目标原有 `curve_driven` 入口按曲线速度而非源增量阈值判断，极小曲线与零 delta 下的其余既有行为不由此项认作已对齐。测试、消融、图审计和公共 Playground Gate 待全部静态修复及 Bevy/依赖升级后执行；旧 `target/bf144/02/turn/` 原始报告缺失。
