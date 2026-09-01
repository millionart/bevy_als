# AB_Als 结构化作者布局（已被实际图替代）

> 当前实际图已使用 runtime context node；本文件与 SVG 仅保留为早期预览记录。应直接打开 `assets/graphs/ab_als/ab_als.animgraph.ron` 检视实际布局。

结构作者源由以下三部分组成：

- `tools/animgraph_contexts/ab_als.animcontext.json`：11 条 context 的 pose、独立 time 和开放曲线继承合同；
- `assets/graph_sources/ab_als.animgraph.structure.ron`：保留节点、position、IO 和非 context edge 的非运行时模板；
- [ab_als.structural-layout.svg](ab_als.structural-layout.svg)：只显示 context 单线的可读布局预览。

预览和 ALS-Refactored `AB_Als` AnimGraph 使用同一主链与相对 position：Standing/Crouching → Grounded → Transition Slot → Locomotion → PostLocomotion Slot → Layering → Head → ControlRig → Ragdolling → Output；Overlay → Inertialization 从下方进入 Layering。

主节点位置保持源图对标值：Standing=(-2240,-144)、Crouching=(-2240,64)、Grounded=(-2000,-48)、Transition=(-1776,-48)、Locomotion=(-1552,-48)、PostLocomotion=(-1328,-48)、Overlay=(-1520,160)、Inertialization=(-1296,176)、Layering=(-1072,48)、Head=(-800,48)、ControlRig=(-560,64)、Ragdolling=(-240,48)、Output=(0,0)。

正式运行图仍是 `assets/graphs/ab_als/ab_als.animgraph.ron`，由模板和合同离线展开。它保留所有标准 edge，以便现有 runtime 和 editor 加载；结构预览承担单线可读视图，不能被当作 runtime graph。
