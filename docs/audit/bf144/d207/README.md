# D207 别名转场保留实际前态

UE `AnimNode_StateMachine.cpp:1293–1337` 在接受规则后将当前实际状态保存为 PreviousState。`EvaluateTransitionStandardBlend` 在没有更旧的混合姿态时从该实际状态采样。`GetStateWeight` 使用相同的前态计算权重。

fresh Grounded 原生编译表及 6 条双向返回轨迹共 222 帧。110 个旧混合完成后的活动帧仍从实际 Standing 或 Crouching 更新，别名回退会漏掉最高 0.9956371 的前态权重。两条无旧混合控制也核对前态。原 Custom 曲线、时钟与权重逐值匹配。

目标激活转场时保存实际前态，将该状态用于缓存请求、权重、尾更新与姿态输入。Rust fixture 使用同一 Custom 曲线和两份带骨架的可区分姿态，验证清理旧栈后仍有连续权重、混合位置及原骨架句柄。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/AB_Als_Grounded -StateMachinePlayers -StateMachineEntryProbe docs/audit/bf144/d207/alias.request.json -Output docs/audit/bf144/d207/alias.native.json
python docs/audit/bf144/d207/replay_alias_source.py
```
