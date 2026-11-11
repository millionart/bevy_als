# D170 加性旋转的全权重分支

`AnimTypes.h:672–675` 将 `weight >= 1.f - ZERO_ANIMWEIGHT_THRESH` 视为全权重。`AnimationRuntime.cpp:1288–1324` 选择 `AccumulateWithAdditiveScale`，其中 `TransformVectorized.h:1091–1105` 先用权重整体乘 delta 四元数，再与基础旋转相乘。局部/网格输出的既有归一化继续按原阶段执行。平移与scale消费原权重。

原生 `AccumulateAdditivePose` 重采大角度和 `.451b` 第90帧实际 Head `neck_01` 的近单位 delta。每组有普通部分权重、阈值前一float、精确阈值、0.999995和1。10组原生旋转方向、平移及scale通过，旧插值在阈值和0.999995共四组偏离。原生倒平方根造成的模长余量独立记录，最大约4.9e−11。Rust fixture 核对原始乘积及两个实际加性入口。原生请求的首次包前缀有误，修正后按相同输入重采，首次退出记录已保留。

```sh
python docs/audit/bf144/d170/replay_additive.py --request > docs/audit/bf144/d170/additive.request.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALSExtras/Environment/Meshes/SM_Als_Stairs_01 -GeometryQueryProbe docs/audit/bf144/d170/additive.request.json -Output docs/audit/bf144/d170/additive.native.json
python docs/audit/bf144/d170/replay_additive.py
```
