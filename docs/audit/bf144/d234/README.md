# D234 TwoWay 的求值权重

`AnimNode_TwoWayBlend.cpp:86–108` 用内部 alpha 更新 B，双边求值把 float 补数作为 A 的权重。 `AnimationRuntime.cpp:527–545` 从 A 的 float 权重回算 B，姿态、曲线和属性均消费求值权重。子节点的 Update、同步和资格沿用原 alpha。

源 Sprint Acceleration 第59帧的 0.4052223861217499 回算为 0.40522241592407227。以该 alpha 和 0.25 控制分别评估真实 WalkRun 的 79 个骨骼，158 个原生 TQS 逐值一致。使用原 alpha 做几何求值改变 79 个 double 与 57 个 f32 结果。扩展缓存及链接的实际 TwoWay fixture，核对同帧的曲线和精确姿态。

```sh
python docs/audit/bf144/d234/replay_weights.py --request
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/SK_Als -BlendProfileProbe docs/audit/bf144/d234/weights.request.json -Output docs/audit/bf144/d234/weights.native.json
python docs/audit/bf144/d234/replay_weights.py
```
