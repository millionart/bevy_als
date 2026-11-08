# BF144-D30 世界暂停时的基座跟随

fresh 角色 CDO 的 Actor 和 CMC 主 tick 均为 `bTickEvenWhenPaused=False`；UE 暂停帧只运行允许暂停 tick 的函数，CMC 后置物理 tick 继承默认 false。组件 tick 的 owner 自定义倍率仅缩放 delta，不等于世界暂停。当前原生 reader 不支持暂停世界，暂停结论由源码和 CDO 组合推导。

目标在世界 `Time<Virtual>` 暂停时跳过基座前/后物理跟随及缓存写入；不检查角色自身倍率。已写外部移动平台、暂停静止平台、恢复和 owner 倍率零对照。历史目标报告 `target/bf144/10/final/` 已缺失；测试、消融、图审计和真实 Playground Gate 待全部静态修复及 Bevy/依赖升级后执行。
