# BF144-D276 Mesh local 发布前的旋转归一化

`source-facts.json` 固定本轮重读的 UE 动画/PostProcess 求值、有效紧凑 Pose 逐骨归一化与 Mesh 写入，以及目标角色刷新门控、共享 writer 和 query99 已验源投影。fresh 骨架、网格、Rig 原生读取复用 D279，递归图闭包见 `tools/animgraph_authoring/bf144_d276.scope.json`。

对拥有角色网格刷新门控的播放器，写入现有旋转通道前转换到源轴、提升至 F64 并复用 `rig_normalize`，再投影回运行时轴和 F32；原 Pose、平移、缩放、缺失通道、禁刷状态和未设置角色门控的播放器均保留。该门控由 `AlsAnimationVisibilityPlugin` 在角色 avatar 的动画播放器建立。此处补的是调用阶段；源 NEON reciprocal-sqrt 精确算术仍属 D248，后置 physics readback 未列入本项已证边界。

已写 query99 真实值、禁刷、其他播放器、缺旋转通道及原 Pose 保持的 focused 用例。旧 `.483` 重放的目标断言记录修复前原样写入，须在对应旧提交复验；本项不改写历史证据。全部条目实现后执行测试、图静态审计与公共视觉 Gate；消融时把发布旋转改回输入，确认 query99 反例失败，恢复源码字节后在同一 target 重编复验。以上测试及消融尚未运行。
