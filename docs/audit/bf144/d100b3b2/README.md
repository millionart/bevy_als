# BF144-D100b3b2 凸包面裁剪与归约

源 `ConstructConvexConvexOneShotManifold` 在参考面局部空间按边裁剪另一个面的顶点：使用 Chaos 双精度厘米，短边 `SafeNormalize` 回退到源 X 轴，输出上限 32；四点时交换第 2、3 点，更多点按深度、平面距离和三角面积归约。源旋转到 Z 的零叉积特殊分支也已保留。`source-facts.json` 包含算法、精度、阈值、旋转及 box 面顺序的源片段。

`bench.native-samples.json` 摘录自 `../d100b3a/shapes.native.json.gz`：Bench shape 1 是环境凸包参考面，shape 2 是 box 参考面。`face-evidence.json` 保存两侧接触点到各候选平面的距离，最接近与次近面之间有明确间隔。测试按源面序在参考面局部空间重建对应面，并逐点对照原生输出；另有四点交换、五点归约、相交和近零分母测试。

运行 `python3 tools/import/import_contact_polygon_data.py --facts docs/audit/bf144/d100b3b2/source-facts.json --output src/avian_ext/source_contact_polygon_data.rs` 可重建 box 面索引及源阈值。函数消费由目标数据推导的米坐标，在计算中还原源轴和厘米；参考面选择及实际求解接入留给 D100b3b3。编译、测试、消融和 Playground 待统一 Gate。
