# Tnua 运动适配

| 文件 | 职责 |
|---|---|
| `control_scheme.rs` | 用 Tnua 原生 Walk/Jump/Crouch 定义控制方案 |
| `motion.rs` | 控制器接线、地面扫掠、步阶、胶囊蹲起和运动/旋转响应 |
| `dynamics.rs` | CMC 参数映射、地面制动、加速度与运动基座 |

这些文件通过 Tnua/Avian 的公开入口补齐源端运动语义。当前仍消费 ALS 状态和参数，文件中同时有通用适配与 ALS 策略；目录归位不声称它们已经可以整体提交上游。后续提取以具体能力的依赖与测试为边界，ALS 状态、资产和角色生命周期仍归 als-core/项目。

`als-core/src/lib.rs` 保持 `als_core::motion`、`als_core::dynamics`、`als_core::control_scheme` 的模块身份。项目级物理时钟接线在仓库根目录 `src/tnua_ext/source_physics_timing.rs`。
