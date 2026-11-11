# D187 隐藏活动 Montage 的零步长图更新

UE `SkeletalMeshComponent.cpp:1677–1684,1883–1918` 允许有 Montage 的隐藏组件 TickPose，OnlyTickMontages 快捷路径要求正步长。`AnimInstance.cpp:709–765` 在零步长进入完整图更新。`Actor.h:4877–4895` 用组件所有者的 CustomTimeDilation 缩放组件步长；`SkeletalMeshComponent.cpp:440` 禁止暂停时的组件 tick。

fresh `USkeletalMeshComponent::ShouldOnlyTickMontages` 读数覆盖最近可见/隐藏及 0、0.5µs、1µs、1/60 秒。结合 TickPose 资格得到 16 个控制，旧目标只在隐藏、有 Montage、零步长的一组少更新图。移除 Montage 资格会增加隐藏空闲更新；暂停控制保持冻结。隐藏时的骨骼刷新沿 `SkinnedMeshComponent.cpp:1805–1808` 的可见性条件。

实际隐藏 Roll 夹具覆盖零步长、正步长、所有者倍率为零及暂停，检查图帧计数和零步长播放位置。原生探针读取快捷路径条件；角色图计数由目标 fixture 核对。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Actions/Roll/A_Als_Roll -NotifyDispatchProbe -Output docs/audit/bf144/d187/roll.native.json
python docs/audit/bf144/d187/replay_component_tick.py
```
