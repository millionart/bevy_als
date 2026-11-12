# BF134.01D Look 非缩放时钟复验

本地 `IA_Als_Look` 含 `InputModifierScaleByDeltaTime`。UE EnhancedPlayerInput 以已施加上限的游戏帧步长除以配置的 dilation，零倍率使用真实帧步长。暂停输入由源资产触发配置拒绝。

Bevy 0.20 在缩放后的时间上施加上限。真实步长 100 ms、配置倍率 0.15、上限 7.5 ms 时，`effective_speed` 为 0.075。旧 Look 得到约 100 ms，原生计算得到约 50 ms。项目用 `relative_speed` 计算，Look 先读取当帧时钟，慢动作和菜单再设置下一帧倍率。

`look_clock_clamp_regression.rs` 用 Bevy 原生 `Time<Virtual>` 复现。主项目现有真实输入回归同时覆盖 1.0、0.15、2.0、零倍率、缩放后上限和暂停，保留独立鼠标输入。
