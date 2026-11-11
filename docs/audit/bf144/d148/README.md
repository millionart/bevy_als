# D148 动画实例的曲线投影所有权

`AnimInstance.cpp:363–374,446–455` 在分配新实例身份前卸载旧实例。`AnimInstanceProxy.cpp:456–466,3566–3572` 清空每种曲线。`AnimInstance.cpp:2177–2195` 对缺失曲线返回 0，`AlsAnimationInstance.cpp:400–405` 据此得到首帧 Head alpha 1。fresh 原生 AB_Als_Head 的 Apply Mesh Space Additive 绑定 `GetParent.ViewState.HeadBlendAmount`，已安装图从同一语义字段取 alpha。

在播放器 Discard 时同步清空投影。替换和重插均可在新实例写入前读取默认曲线值，实体销毁时也无需排队修改组件。静态重放覆盖两条替换路径和两种旧姿态。保留旧瞄准曲线会在两条路径上将 alpha 降至 0.005088。只删瞄准曲线可以恢复该 alpha，仍会留下旧 FootLeftIk。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/AB_Als_Head -Output docs/audit/bf144/d148/head.native.json
python docs/audit/bf144/d148/replay_curve_reset.py
```

真实角色反例 `replaced_aiming_player_starts_head_with_empty_previous_curves` 由 Rifle Overlay 产生旧曲线，通过一次零步长更新观察实际替换首帧。Rust、运行时消融恢复和 Playground Gate 待统一验证。
