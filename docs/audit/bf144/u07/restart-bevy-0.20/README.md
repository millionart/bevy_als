# 重新准入的默认运动模式

本地 `APawn::Restart` 停止移动并消费待处理输入。 `ACharacter::Restart` 清除 Jump 状态，调用 `SetDefaultMovementMode`。尝试 Walking 后没有有效支撑时，当帧进入 Falling。ALS 的原运动锁继续控制 Mantle 和 Ragdoll 的模式写入。

旧目标重新准入在出生支撑处理之后排队 Grounded，后续静止地板检查只改当帧最终模式，首个物理步的重力未消费。复用既有锁判定，在普通移动前提交默认模式并复用 D252 的支撑初始化。定向用例验证有支撑、无支撑、已有 Jump、输入与视角以及真实动作锁。

定向测试使用同一 `target_game` 的 play profile，临时给项目包设置 opt-level 1 和增量编译。正式 Playground 由原启动器按原 play 配置构建。
