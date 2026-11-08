# BF144-D59 Mantle 根运动前的基座携带

fresh UE `PerformMovement` 先更新基座携带，再准备和消费 RMS；`SetBase` 在基座改变时保存初始变换，ALS RMS 以携带后的角色位置计算自身速度。默认 BasedMovementMode 为 2，native 角色忽略基座旋转、关闭 fast attached move。源码、原生角色请求、输出和 reader 身份保存在本目录。

目标在 Custom Mantle 的物理前阶段复用现有脚底锚点携带，首次换基座使用安装时快照；RMS 继续从实际携带后的位置计算速度。已写静止/移动、基座首次绑定和倍率 0.5/1/2 的对照，并保留动画速度消费测试；测试、消融和动画 Gate 待统一验证。物理后携带与二次 warp 属于 D60，未在本项修改。旧清单所引 `target/bf144/19/initial/` 已不存在，其原始报告缺失。
