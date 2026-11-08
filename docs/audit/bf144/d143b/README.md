# BF144-D143b Box 核心 GJK 法向

源 box/convex 查询按 D143a 的有效 margin 缩小动态 box 核心，静态凸包保持完整。目标继续使用已有 Parry support-map GJK，只改变 box 的输入核心，再把方向映射回 Avian manifold；这针对 BF144.60 已隔离的输入差异，不宣称两套 GJK 的所有迭代或 EPA 行为相同。源算法及 fresh 原始探针见 `../d143/`，`target-facts.json` 保存目标查询及 shape 代码。

`../d143/bench-foot.native-sample.json` 是 fresh GJK 原始输出中 Bench/foot 的摘录。窄相位先保留原始子形体身份，再计算同一凸包的核心法向，最后按源约束组顺序分配完整 manifold；单凸包也接入。已编写原生法向、零 margin、共同刚体变换、普通 body 和多凸包排序交互测试。

当前 margin CVar 使用源码默认并由已观察的手脚样本核对；统一 Gate 须先重建 reader、重采三个 CVar 并核对重新生成的设置。测试、消融和 Playground 均尚未运行；源支持顶点和参考面选择仍属于 D100b3b3。
