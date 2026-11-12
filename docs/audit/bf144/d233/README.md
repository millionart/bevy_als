# D233 双姿态曲线插值

`AnimationRuntime.cpp:527–545` 的双姿态求值调用 `AnimCurveTypes.h:692–717` 的三参数 Lerp。部分权重使用两个曲线集合的并集，单边缺值以零进入 `A + (B - A) * Alpha`。阈值端点直接取对应集合。

TwoWay 与实际 gait 快路径共用曲线逻辑，姿态与 Update 权重仍各自消费原字段。Standing SprintBlock 第2帧与第4帧的同值端点原生读数分别为 0.24301831424236298 和 0.2274143546819687。替换为旧加权求和分别变为 0.24301832914352417 和 0.2274143397808075。另核单侧值、混合值和两个端点，共七个原生控制。D216 的26帧 gait 回放逐值相同。TwoWay 的求值权重回算由 D234 验收。

```sh
python docs/audit/bf144/d233/replay_curves.py --request
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/SK_Als -BlendProfileProbe docs/audit/bf144/d233/curves.request.json -Output docs/audit/bf144/d233/curves.native.json
python docs/audit/bf144/d233/replay_curves.py
python docs/audit/bf144/d216/replay_pose_branches.py
```
