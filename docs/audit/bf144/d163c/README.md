# D163c 组件与局部的单精度旋转乘法

`AnimationRuntime.cpp:1949–1977` 分别以 `parent * local` 和 `inverse(parent) * component` 构造组件旋转及还原局部旋转。`UnrealMathNeon.h:2318–2327` 在源基中按 W→X→Y→Z 累加。`glam` 的 NEON 实现按两对求和。保留目标 F32 精度、权重和各归一化点，只统一源基与每步累加顺序。

以 `.451b` 已记录的四帧骨旋转构造 12 个双骨输入，用 `FQuat4f` 原生重采每组组件和局部乘积，共 24 个精确对照。旧求和在 6 个组件及 11 个局部输出失配，单独恢复旧组件或旧局部路径分别导致 5 和 9 个最终局部乘积失配。Rust fixture 在真实 Layered Pose 入口验证组件与局部输出，保留现有归一化器。它核验单精度乘法，完整源姿态还涉及 `FQuat` 精度及其他阶段。

```sh
python docs/audit/bf144/d163c/replay_layered_products.py --request > docs/audit/bf144/d163c/layered.request.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALSExtras/Environment/Meshes/SM_Als_Stairs_01 -GeometryQueryProbe docs/audit/bf144/d163c/layered.request.json -Output docs/audit/bf144/d163c/layered.native.json
python docs/audit/bf144/d163c/replay_layered_products.py
```
