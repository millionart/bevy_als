# 扩展源码分区

`als-ext` 是现有 Cargo/资产注册入口。文件按能力归位，`lib.rs` 用 `#[path]` 保持原来的 Rust 模块和反射类型身份；RON 中的 `als_ext::...` 无需迁移。

| 源目录 | 职责及后续归属 |
|---|---|
| `src/animation_graph_ext/` | `bevy_animation_graph` 的节点、求值上下文、FSM、同步、曲线、root motion、slot 与资产加载扩展；评估动画图上游贡献从这里开始 |
| `src/control_rig/` | UE RigVM/Control Rig 的程序、层级、数学、IK 与空间查询语义；独立于 ALS 的动作选择 |
| `src/avian_ext/` | Avian 物理扩展：UE 关节/角驱动、PhysicsAsset 配置与穿透修正 |
| `src/bevy_ext/` | 材质/贴花、文本布局、时间缩放等 Bevy 支撑能力 |
| `src/movement/` | 水平运动计算、胶囊姿态事务等辅助类型，不直接依赖 Tnua |
| `src/als/` | ALS native refresh、HipsDirection、自定义层信号、overlay 骨骼选择和 ragdoll tag 默认值 |
| `src/playground/` | ALS Playground 场景与环境材质 |

Tnua 的实际接线在 [`als-core/src/tnua_ext/`](../als-core/src/tnua_ext/README.md)，完整帧角色积分与普通物理时钟的项目适配在 `src/tnua_ext/source_physics_timing.rs`（仓库根目录）。

这里区分的是源码职责，尚未拆成独立发布的库。当前 Control Rig 数学复用 als-core 的阻尼公式，query 测试使用项目碰撞层；ALS 和 Playground 适配仍共享注册入口。反哺上游时应选取所需能力及其最小依赖闭包，保留对应 focused 反例；ALS 动作策略、骨骼/资产选择和 Playground 不属于通用插件能力。
