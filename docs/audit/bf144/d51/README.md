# BF144-D51 Grounded Roll 的 owner 最小运动时间

fresh UE 源将 actor tick 乘 CustomTimeDilation，并在角色运动与姿态提取时对小于 1 µs 的 delta 提前返回；fresh 原生角色默认倍率为 1。Avian 源使用世界物理步独立积分位置。请求、源码、原生读取和哈希保存在本目录，本次未在源端重跑微小倍率 Roll 场景。

目标 Grounded Roll 在 owner delta 低于源门槛时不写新根速度，并复用现有单帧自定义位置积分保护，物理结束后恢复原状态。已写 0、1e-5、1e-4、1 倍率的系统和加载角色对照；Custom 既有最小时间对照保留。测试、消融与动画公共视觉 Gate 待统一验证阶段执行。

本反例限定静态基座及零外力；碰撞求解器造成的位置修正与其他运动模式未在本项扩展。旧清单所引 `target/bf144/15/owner_minimum/` 已不存在，其原始报告缺失。
