# D239 Fall 速度映射

`RefreshInAir` 在蓝图字段发布前将垂直速度转换为 float 厘米每秒。共享属性单位规则保留源图的厘米区间，Fall、Jump 和 Land 的消费者使用相同字段。Playground 的速度诊断继续以米每秒展示。

实际 Fall 节点的 13 帧原生 alpha 逐值一致。旧米制计算有 6 帧不同。原反例 −0.3269999623298645 m/s 的差异保留在更新 alpha，经 D234 补数回算后两边得到相同姿态权重。受控速度 −0.16349999606609344 m/s 使求值第二权重从 0.016349971294403076 恢复到 0.01635003089904785。固定实际 Fall_Fast 和 Fall 首帧的 79 骨原生 TQS，恢复旧权重改变 70 骨的 double 结果，其中 29 骨改变 f32 投影。

全量转换涵盖 15 个安装根和递归依赖。会话中断时主根已写出完整转换报告及安装清单，恢复时先核对这两个产物，再运行其余安装根。完整产物差异见 `corpus-diff.json` 和 `full_diff.patch`。

```sh
python docs/audit/bf144/d239/replay_fall_alpha.py --request
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/AB_Als_Locomotion -BlendListProbe docs/audit/bf144/d239/fall.request.json -Output docs/audit/bf144/d239/fall.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/AB_Als_Locomotion -BlendListProbe docs/audit/bf144/d239/complement.request.json -Output docs/audit/bf144/d239/complement.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Air/Fall/A_Als_Fall_Fast -SequencePoseProbe docs/audit/bf144/d239/sequence.request.json -Output docs/audit/bf144/d239/fast.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Air/Fall/A_Als_Fall -SequencePoseProbe docs/audit/bf144/d239/sequence.request.json -Output docs/audit/bf144/d239/slow.native.json
python docs/audit/bf144/d239/replay_fall_alpha.py --weights
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/SK_Als -BlendProfileProbe docs/audit/bf144/d239/weights.request.json -Output docs/audit/bf144/d239/weights.native.json
python docs/audit/bf144/d239/replay_fall_alpha.py
python tools/animgraph_authoring/regenerate_installed_corpus.py --content-root "$ALS_CONTENT_ROOT" --output target/d239-corpus
```
