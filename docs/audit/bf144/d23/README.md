# BF144-D23 落地滚翻使用保留的角色速度

fresh UE 组件 tick 先于 owner tick；ALS 在落地通知中用上一轮 `LocomotionState.Velocity` 判断滚翻，随后 `RefreshLocomotion` 才从当前物理速度更新该快照。fresh 原生四组均实际落地：保留速度约 −3.16 m/s、当前 −8 或 −3 m/s 时没有 Roll montage；保留速度约 −8.16 m/s、当前 −3 或 −8 m/s 时 Roll montage 均实际播放。请求、逐帧角色状态及 montage 证据保存在本目录。

目标落地事件的 Roll 阈值改用现有 `AlsActualMotion.world_velocity` 保留快照，与碰撞仲裁及滚翻方向使用同一阶段；无事件的既有回退和 pre-impact 记录保持原样。已写速度交叉、阈值和加载角色落地事务对照，并更新旧隔离 fixture 的源快照。历史目标报告 `target/bf144/07/retained_velocity/` 已缺失；测试、消融、图审计和真实 Playground Gate 待全部静态修复及 Bevy/依赖升级后执行。
