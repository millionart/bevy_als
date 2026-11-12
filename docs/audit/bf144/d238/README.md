# D238 满权重组件旋转

`AnimationRuntime.cpp:2217–2257` 的满权重直接使用目标组件旋转。部分权重在 FastLerp 后归一化一次。有父骨骼时按源顺序转回局部，再归一化局部结果。

重新读取 Spine 与 Crouching Stop 模板，12个输入的948个骨骼与原生逐值一致，含132个部分权重。仅加回满权重的两次组件归一化，改变202个 double 结果和14个 f32 结果。Rust fixture 核对近满权重的直接组件复制，随统一验证执行。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/AB_Als_Layering -LayeredBoneBlendProbe docs/audit/bf144/d232c/spine.request.json -Output docs/audit/bf144/d238/spine.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/Stances/AB_Als_Crouching -LayeredBoneBlendProbe docs/audit/bf144/d232c/stop.request.json -Output docs/audit/bf144/d238/stop.native.json
python docs/audit/bf144/d238/replay_full_weight.py
```
