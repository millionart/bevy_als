# BF144-D100b4b2a1：源线段最近点

fresh 源 `Utilities::NearestPointsOnLineSegments` 以 double 厘米计算两线段参数和点，按源顺序处理退化、平行及两端的夹取；capsule/triangle 对三条边逐一调用该函数并复用结果。源首个退化判断是 `A <= EpsilonSq && B <= EpsilonSq`，其中 B 是两线段方向点积，不是第二条线段的长度；首线段退化、次线段非退化时仍可能直接返回两个起点。目标保留这个源分支，不以其他几何库的通常最近点替换。源码及哈希见 `source-facts.json`。

已写交叉、平行、夹取和首尾退化的 focused 测试，并扩展 reader 直接调用源工具。`segment.request.json` 包含八组源输入，其中 `nearly_point` 覆盖首线段很短但 B 不落入首个退化分支。旧 reader 不输出新字段；统一 Gate 须先重建 reader、保存原生报告及身份，运行 `tools/tests/test_physics_segment_nearest_probe.py`，再设置 `ALS_SEGMENT_NEAREST_NATIVE_REPORT` 运行 Rust 原生对照。编译、测试、消融与 Playground 均未执行；capsule 消费留 a2–a4。
