# D158 pending 帧发布当前脚目标

`AlsAnimationInstance.cpp:1252–1294` 在 pending 更新中置 Feet 无效，随后仍读取两个足目标。`1297–1305` 在无效时跳过锁定和曲线刷新。`1695–1767` 的动态过渡回调用当前目标到已有锁点的距离。原生 [Standing Idle](../d150/standing.native.json.gz) 在 On Update 调用它，[AIS_Als_Default](../d147/settings.native.json) 的门槛为 8 cm。

pending 且本帧更新图时，使用与常规路径相同的足目标采样，保留已有锁点和权重，Rig 有效标志维持 false。六组静态重放覆盖 0 cm 与 20 cm 位移。移除当前采样后，20 cm 的 pending 帧将距离平方从 0.04 m² 变为 0，漏掉 StandingLeft。

```sh
python docs/audit/bf144/d158/replay_pending_foot_target.py
```

`pending_feet_publish_current_target_distance_to_standing_idle` 在加载角色上核对目标距离、有效性和实际回调的本帧队列写入。Rust、消融恢复和 Playground Gate 待统一验证。
