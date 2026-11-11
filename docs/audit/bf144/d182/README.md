# D182 Roll 排队状态的连续身份

fresh Roll 的 RootScale 对象 `NotifyStateBehaviorFlags=0`。UE `AnimNotifyQueue.cpp:53–76` 按通知身份去重，`AnimInstance.cpp:1893–1938` 在此标志下沿用活动通知的 ID。原生队列和派发器处理来源 11、11+22、22、空队列、33。前三帧持有同一个活动通知，来源可从 11 交接到 22；空队列结束状态，33 获得新 ID。

目标以现有各播放实例的活动状态汇总同一 RootScale 通知。任一实例在帧间继续提供通知时不重复 Begin 或 End，同帧重复 Active 只派发一次。受控回放的外部倍率 2 在重入和最终 End 时保留；旧路径在重叠帧改回 1.3。分别移除 Begin 合并、End 合并和同帧去重都会失配。Rust fixture 覆盖两实例交接及实际 Roll 重入的倍率。

原生读取器第一轮链接发现 `FAnimNotifyQueue::Reset` 未导出；改用已有的 `UAnimInstance::ClearQueuedAnimEvents` 后构建及取证成功。原生读数覆盖通知身份；倍率与位移的消费由目标派发与角色夹具验证。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Actions/Roll/A_Als_Roll -NotifyDispatchProbe -Output docs/audit/bf144/d182/roll.native.json
python docs/audit/bf144/d182/replay_notify_identity.py
```
