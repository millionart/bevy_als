# D221 按父到子顺序计算同步权重

UE `AnimNodeBase.h:281–286` 的 FractionalWeight 从当前父 Context 乘下一层倍率。状态机、MultiWay、TwoWay 和 LayeredBoneBlend 用该 Context 更新子项，SaveCachedPose 在绝对请求权重中选最大值。

fresh Standing 父图 32 帧的 Sprint 子调用与缓存请求逐值匹配。反向结合改变 22 个终端权重。首个源乘积为 0.05908052623271942，旧结合顺序为 0.05908053368330002。

目标播放器在 Update 时记录有效绝对权重。缓存预热从当前父 Context 开始逐级乘法，并以胜出请求的绝对值更新。Rust fixture 覆盖直接 TwoWay、MultiWay、共享缓存和实际同步出口。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/Stances/AB_Als_Standing -BlendListProbe docs/audit/bf144/d221/weights.request.json -Output docs/audit/bf144/d221/weights.native.json
python docs/audit/bf144/d221/replay_parent_weights.py
```
