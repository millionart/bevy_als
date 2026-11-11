# D100b4b2b1 箱体三角面 SAT

源 `ConvexTriMeshMode=1` 使用 `ConstructConvexTriangleOneShotManifold2`。源按三角面投影、箱体六面、有效 Minkowski 棱边的次序剔除或选轴。棱边保留源半边遍历及正向顶点编号，邻面顺序与边方向匹配。源三角边剔除表按 `(2,0)`、`(0,1)`、`(1,2)` 取最小距离，候选边按 `(0,1)`、`(1,2)`、`(2,0)` 枚举。资格保留 float `1.e-2f` 提升到 double 的严格门槛，比较使用有符号平方。选轴前只给三角面分离值增加 double `0.01 cm`。最终按质心方向翻转分离轴，独立保留裁剪轴、参考面和点类型。

`tools/import/import_box_triangle_sat.py` 从 `source-facts.json` 生成 `src/avian_ext/source_mesh_box_sat_data.rs`。同一工具从 D104b3b 的 `restore.native.json.gz` 生成 `cart-sat-cases.json`，保留实际箱体局部范围、Cart 988/989 顶点、类型和源法线。988 是 `VertexPlane`，989 是 `PlaneVertex`。这里复用已保存的原始接触结果核对分离轴，逐面裁剪读取合同由下一子项准备，重采待统一 Gate。focused 合同还覆盖距离、背面、棱边绕序及相等轴分量。编译、测试、消融和 Playground 待统一 Gate。
