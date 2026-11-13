# BF102.03 步态与动画层复验

ALS `CalculateActualGait` 用双向最大步行速度加 10 cm/s 判定 Walking。测试配置为正向 175、反向 100 cm/s，因此分界是 185 cm/s。反向移动 120 cm/s 时为 Walking，190 cm/s 时为 Running。

CMC 方向速度仍以反向 100–200 cm/s 映射到 GaitAmount 1–2。原生 `FMath::GetMappedRangeValueClamped` 分别得到 1.2000000476837158 和 1.8999999761581421。加载层回归独立验证角色步态、方向量和实际权重发布。

原生请求可通过项目 `run_ue_source_reader.ps1 -Asset /ALS/ALS/Data/Character/Movement/MS_Als_Normal.MS_Als_Normal -GeometryQueryProbe <request> -Output <output>` 重跑。目标用例为 `gait_layer::tests::direction_dependent_gait_amount_reaches_animation_layer`。
