# D213 循环末点

原生 `FAnimationRuntime::AdvanceTime` 在严格越过边界后才回绕。Idle 轨迹包含恰好到达末点、从末点继续前进、反向恰好到零三种控制。`BlendSpace.cpp:814–851` 将无 marker 路径的共同当前值和前值映射到各样本长度。原生 Look 的普通循环从归一化 0.75 到 1.0，下一步才回到 0.125。

目标完整图夹具包含两份等长样本和被排除于同步组的播放器。仅移除共享归一化传播时，第二份样本取 0.0 秒，混合曲线为 0.75。恢复后，两份样本在该帧都取 1.0 秒。正式 Head 七段通过。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/View/BS_Als_Look -BlendSpaceProbe docs/audit/bf144/d213/exact-loop-endpoint/blendspace-end-0.75.request.json -Output target/blendspace-end.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Base/A_Als_Idle -SequenceClockProbe docs/audit/bf144/d213/exact-loop-endpoint/sequence-exact-end.request.json -Output target/sequence-end.native.json
```
