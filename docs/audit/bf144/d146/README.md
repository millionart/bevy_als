# D146 隐藏期间的 montage 与节点相关性

UE `SkeletalMeshComponent.cpp:1883–1918` 在图不更新时以 `IsAnyMontagePlaying` 决定是否 tick pose。`AnimInstance.cpp:3613–3616` 用实例数组长度判断。`AnimInstance.cpp:709–725` 在推进 montage 前重置 proxy 更新计数，`2407–2427` 在 blend-out 事件后移除无效实例。`AnimNode_LinkedAnimGraph.cpp:92–98` 同步根与子 proxy，`AnimNode_CallFunction.cpp:29–39` 用不同步的计数触发 OnBecomeRelevant。

原生 Head 将 `InitializeHead` 放在 OnBecomeRelevant 上。`AlsLinkedAnimationInstance.cpp:58–70` 把两个调用转交父实例。`AlsAnimationInstance.cpp:523–526,628–647` 将首次 Head 更新设为当前角度，普通更新继续插值。安装图保留相同节点、调用顺序与 Look Blendspace 输入。

原生 Transition 继续运行时位置到 0.2 秒。终止帧前实例权重约 0.004672，推进后无效。另一组在末节停止播放后仍有效，权重从约 0.574750 淡出。静态重放覆盖空闲、运行、末节保持、淡出结束及已移除五种情况。只用播放标志会漏末节保持，只用推进后的资格会漏终止帧，移除调用重置会漏三种有实例的隐藏 tick。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -SelfTest
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/AB_Als_Head -Output docs/audit/bf144/d146/head.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Transitions/A_Als_Stand_DynamicTransition_Left -MontageClockProbe docs/audit/bf144/d146/montage.request.json -Output docs/audit/bf144/d146/montage.native.json
python docs/audit/bf144/d146/replay_relevancy.py
```

Rust 反例 `hidden_montage_ticks_restart_the_actual_head_relevance` 通过已加载角色覆盖空闲、正常 Transition 和最后一个隐藏帧结束的 Transition。等待统一运行此反例、消融恢复和 Playground 可见性场景。
