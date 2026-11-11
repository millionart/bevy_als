# D100b4b2b2 箱体三角面裁剪

源 mode 1 以 SAT 所选轴裁剪箱体面或三角面。裁剪用未归一化边平面，只有边法线平方长度超过源小数阈值时才调用共享裁剪。四点以内保留裁剪原序，超过四点按分离轴旋转后约简。按参考面及接触类型选择面法线投影或带点积除数的斜轴投影，严格 `phi < cull` 后输出支持点、逐点类型和深度。目标复用共享平面裁剪与四点约简，保留本分支的原序。

`prepare_request.py` 从 D104b3b、D100b 的原生 Cart、Stairs02、Rock 足部姿态生成 `box-triangles.request.json`，加 Cart 988/989 背面对照。reader 已扩展 `triangle_indices`，按形体调用源胶囊或箱体逐面算法，并输出同一局部空间的顶点和支持点。负缩放翻转与源 mesh 采集一致。当前二进制只支持胶囊读取，统一 Gate 重建后用 `tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/PA_Als.PA_Als -PhysicsContactProbe <request> -Output <native>` 重采并保存报告、哈希和原始读数。设置 `ALS_BOX_TRIANGLE_NATIVE_REPORT` 运行逐面 Rust 对照。四点原序、严格 cull、六点约简的 focused 测试、编译、消融和 Playground 均待统一 Gate。
