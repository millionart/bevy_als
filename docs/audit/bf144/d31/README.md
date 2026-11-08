# BF144-D31 地面距离带内的悬浮弹簧

fresh ALS/UE 源显示 Walking 保持水平速度，`AdjustFloorHeight` 仅在 1.9–2.4 cm 带外调整高度；Tnua 默认的位置弹簧会额外施加 `offset * spring_strength`，同时独立提供重力补偿。fresh 原生五组静止角色中，2.0、2.15、2.35 cm 保持原高度，1.8、2.5 cm 均调整到约 2.15 cm；请求、逐帧读数及源代码保存在本目录。

目标将 Walking 的位置弹簧强度设为零，保留阻尼、重力补偿和现有源距离带调整。已写配置与加载角色带内/带外持续帧对照；既有 `docs/audit/bf144/446g/` 保存了先前针对该因子的实际消融证据。测试、更新后消融、图审计和真实 Playground Gate 待全部静态修复及 Bevy/依赖升级后执行。
