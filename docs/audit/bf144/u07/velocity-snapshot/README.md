# D280–D282 运动快照复验

`GlobalTransform::rotation()` 分解实际 90° 目标四元数，分量为 `[0, 0.7071067094802856, 0, 0.7071067690849304]`。同输入换入源身体坐标系后调用 UE 双精度逆转、F32 厘米归一化、绝对分量和倒数及限幅。原生四分之一转前向为 `0.99999988079071045`，与已发布目标快照一致。此断言使用对应的 ARM64 原生值，其余平台保留一 epsilon 的比较。原测试的单位前向断言仍用于恒等旋转。

原生请求同时保存恒等、四分之一转、三轴对角与停止输入。可用项目 `run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/SKM_Als.SKM_Als -GeometryQueryProbe <request> -Output <output>` 重读。目标用例为 `actual_motion::tests::snapshot_is_character_local_and_isolated`。这组输入分别核对实际局部方向和前向混合，其他 F32 局部分量的算序差由现有 D282/502 路径继续核对。
