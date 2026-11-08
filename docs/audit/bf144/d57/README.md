# BF144-D57 当帧瞄准与 Mantle 提前淡出

fresh ALS 角色先刷新 RotationMode，Mesh tick 依赖角色 tick；EarlyBlendOut branching notify 读取该模式并停止对应 Montage 实例。fresh High/Low Montage 均检查 Aiming，淡出分别为 0.4 s Cubic 和 0.35 s HermiteCubic。原生输出、请求、reader 身份及源码片段保存在本目录。

目标将 Mantle 通知消费者排在 aim_rotation_mode_system 之后，仍在发布 Slot 样本之前；已写同帧瞄准/非瞄准和两种淡出时长的调度对照，以及加载角色真实通知与 Slot 中断标记测试。测试、消融、图审计和动画 Gate 待统一验证；旧清单所引 `target/bf144/17/final/` 已不存在，其原始报告缺失。
