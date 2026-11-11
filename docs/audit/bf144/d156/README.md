# D156 世界启动的脚锁重定位窗口

[AB_Als 原生 CDO](../d149/ab-als.native.json) 的 TeleportedTime 是数值 0 的双精度字段，与 `AlsAnimationInstance.h:56–58,377–379` 一致。`World.h:4714–4716` 用当前游戏时间减去此值。`AlsAnimationInstance.cpp:1340–1347` 在脚锁相关、Feet 已经有效且经过时间至多 0.2 秒时按当前组件重定位。

时间戳改用数值初始值 0。六组静态重放中，恢复旧可选时间时只在 0.1 秒且无事件的组漏重定位。移位 10 cm 时该组在 RigInput 保留 10 cm 误差。0.3 秒和 1 秒均超出窗口，0.2 秒仍在源 float 边界内。

```sh
python docs/audit/bf144/d156/replay_startup_teleport_window.py
```

`startup_foot_lock_rebases_before_the_default_window_expires` 使用加载角色、上一帧真实组件变换及受控游戏时间，核对实际脚锁输入发布和事件计数。Rust、消融恢复和 Playground Gate 待统一验证。
