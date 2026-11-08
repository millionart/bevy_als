# BF144-D62 无支撑 Mantle 中断帧

fresh ALS `RefreshMantling` 在角色 Tick 发现动作结束后调用 `StopMantling`，切换 Walking；UE Walking 入口查 floor、清除竖直速度，但不会当场切 Falling，角色据此发布 Grounded。下一次 Walking 物理检查才可转 Falling。fresh AB_Als 原生读取、请求、reader 身份及 ALS/UE 源码片段保存在本目录。

目标在 Mantle 动作结束或目标失效而 Ragdoll 许可失败、切回 Walking 时标记该次同步，在同帧无支撑时保留 Grounded；有支撑时仍执行原有 floor 处理，标记在同步时消费。D65b fresh 源闭包确认两种路径共用 `StopMantling`，因此有限复开并补目标失效对照。已写加载角色离地中断、当帧动画 Grounded 与下一帧 InAir 对照；测试、消融和动画 Gate 待统一验证。旧清单所引 `target/bf144/19/` 已不存在，其原始报告缺失。
