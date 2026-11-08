# BF144-D10b 空中地面预测的几何面资格

fresh ALS `RefreshGroundPrediction` 对扫掠命中的 `ImpactNormal` 与可行走阈值比较，并明确允许初始穿透命中；源动画设置与预测曲线已重新通过原生 reader 提取。几何面选择、初始重叠规则及盒体边缘控制复用 `docs/audit/bf144/d10a/` 的 fresh 源证据。

动画图输入与 `AlsAirState` 的公开预测均接入相同几何面选择，初始重叠仍保留原接触法线。已写 30°/60° 凸体边缘的两个消费者对照，以及真实加载角色的动画输入对照。旧 `target/bf144/70/full_initial/` 目标报告已缺失；测试、消融、图审计和真实 Playground Gate 待全部静态修复及 Bevy/依赖升级后执行。
