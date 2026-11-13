# D320 动画速度航向

本地 `AAlsCharacter::RefreshLocomotion` 在速度至少 1 cm/s 时以源世界平面的 double atan2 得出角度，再将度数保存为 float。无速度时保留上一角。`RefreshMovementDirection` 在 View 的 double 世界角上执行差分和最后一次 float 转换。目标的运动和动作仍能读取自己的 Bevy 朝向。实际图、方向节点和 Standing 的速度分类消费同一份源速度角。

`request.json` 包含首五个移动帧两端各自的速度及五个边界。通过项目 `run_ue_source_reader.ps1 -GeometryQueryProbe`，以 `CF_Als_RotationYawOffset_ForwardBackward` 资产执行原生 atan2、差分和曲线求值。报告压缩前的身份见 `identity.json`。移除源角消费使实际运动快照首共同输入稳定失败。恢复后同 target 重编，重新运行实际运动、Standing 和方向的回归。

`playground/` 是正式可见入口的完整 319 帧历史及功能验收。与前一次的 319 帧物理输入和 79 冷帧姿态逐值相同。五个活动帧的目标速度角逐值等于同输入原生。首个 Root 偏航由 0.00032401096541434526° 改为 0.0003243075334466994°，原生为 0.0003243905957788229°。FeetCrossing、上游虚拟足及独立曲线算序仍有余差。
