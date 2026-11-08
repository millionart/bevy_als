# BF144-D12 零 owner delta 的贴地调整

fresh 源闭包显示组件 tick 使用世界 delta 乘 owner `CustomTimeDilation`，`StartNewPhysics` 对小于 `1e-6` 秒的 delta 直接返回，ALS `PhysWalking` 也有同一阈值；外部移动只使下次 floor 检查失效，不会绕过该阈值执行贴地调整。原生角色默认倍率为 1；本次原生 reader 不支持设置角色倍率，零倍率行为的证据为可重读的 UE/ALS 源代码，不将静态推导冒充原生运行样本。

目标在 Grounded 后置 floor 消费前按 owner delta 应用源阈值，同时保留碰撞结果清理和世界暂停的独立处理。已写加载角色的零倍率、已贴地、阈值上下边界及恢复对照。历史目标报告 `target/bf144/04/frozen/` 已缺失；测试、消融、图审计和真实 Playground Gate 待全部静态修复及 Bevy/依赖升级后执行。
