# BF144-D10a 地面扫掠的几何面法线

fresh UE 源闭包包括 ALS `ComputeFloorDist`、UE `IsWalkable`、碰撞结果转换、Chaos 面选择和初始重叠判断。正距离扫掠对命中点附近的几何面选与扫掠方向最相反的面；可行走性使用 `ImpactNormal`，而初始重叠保留接触法线。fresh 原生样本的同一盒体边缘在前向碰撞时返回侧面 `ImpactNormal=(-1,0,0)` 且不可行走，在向下扫掠时返回顶面 `(0,0,1)` 且可行走；其胶囊接触法线分别另有斜向分量。原生 request、报告、输出、源片段、身份和可重建 reader 见本目录。

目标现有 `shape_impact_normal` 已覆盖盒体、凸体、三角网格、高度场及复合形状；本项将其接入地面扫掠，单独保存几何面与接触法线，移除原先按接触法线触发的短射线猜测。新增 30°/60° 凸体边缘对照及初始重叠控制，复用已有多形体面选择和侧面穿透测试。历史目标报告 `target/bf144/04/normal_loaded/` 与 `normal_query/` 已缺失；运行测试、消融、图审计和真实 Playground Gate 均待全部静态修复及 Bevy/依赖升级后执行。GroundPrediction 和 StepUp 首次滑动分别留给 D10b、D10c。
