# BF144-D246b2 其余实际碰撞形状

Playground 的盒体、TriMesh 和 compound convex，以及角色/动态碰撞的球体和胶囊，均可能进入 Rig 碰撞快照。fresh UE 对 `SM_Als_Barrel` 的同一 F64 世界射线分别查询 simple convex 和 complex mesh：两种命中均保留输入水平坐标，高度分别为 `96.0999984741211` 和 `92.21902507820052` cm。源结果和形状标记、请求及资产哈希在本目录；这两种几何不同，不把其中一种结果替代另一种。

目标对球、胶囊、圆柱使用现有 Parry F64 primitive，对 convex 直接借用当前 F32 顶点作 F64 support-map 求交，对 compound 递归其现有子形状和局部变换。盒体和 TriMesh 保持前两项路径，所有支持形状共同参与过滤和最近命中。未识别形状或包含未识别子形状的 compound 继续使用原 F32 查询；Cone 的 fallback fixture 明确记录此公开边界，避免把其他形状伪称已覆盖。

已写曲面 F64 坐标、compound convex、混合最近命中及 fallback fixture，尚未执行。目标已有碰撞顶点与变换仍为 F32，Chaos/Parry 的接触边界和 D162 网格选面仍有独立差异。完整源场景形状回归、性能、消融/恢复、图审计和公共 Playground Gate 待全部静态修复及 Bevy/依赖迁移后执行。
