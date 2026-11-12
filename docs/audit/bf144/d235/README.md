# D235 强制 raw 的共同加性参考

`AnimSequence.cpp:1895–1925,1938–1972` 先抽取绝对姿态，再依 `ABPT_AnimFrame` 和参考帧请求 raw base。Lean 与 Stand 的 frame 0 分别以实际 SKM_Als 读回，合计 158 个骨骼 TQS。离线编译核对动画标识、精确采样时点、网格和父骨骼顺序，产物存于 `assets/ALSRuntime/Animations/als_raw_additive_references.json`。

需要现场差分的 Reference 节点沿实际参考 pin 查找 Clip 和固定时间，Sequence 的现场差分按原参考 Clip 与时点查找。精确参考直接进入 double 差分。安装的 51 条播放器参考及 23 个 BlendSpace 包装绑定至 D224 已证实的压缩 delta。普通参考播放仍按其公开采样合同执行。

新 raw 参考重放 Lean 与 Look 的 474 个原生加性骨骼全部精确。以普通压缩参考替换后，Lean 和 Stand 消费者分别改变 210 和 225 个 f32 delta。Rust fixture 核对共同 Lean 参考的源帧与 Clip 身份。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Grounded/Lean/A_Als_Lean_Pose -SequencePoseProbe docs/audit/bf144/d232d/reference.request.json -Output docs/audit/bf144/d235/lean.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Base/A_Als_Stand_Pose -SequencePoseProbe docs/audit/bf144/d232d/reference.request.json -Output docs/audit/bf144/d235/stand.native.json
python tools/import/compile_als_raw_additive_references.py --output assets/ALSRuntime/Animations/als_raw_additive_references.json docs/audit/bf144/d235/lean.native.json.gz docs/audit/bf144/d235/stand.native.json.gz
python docs/audit/bf144/d235/replay_references.py
```
