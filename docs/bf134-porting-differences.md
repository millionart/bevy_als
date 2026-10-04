# BF134 移植差异台账

本轮起始版本 `9105cdf`；对标本地 ALS-Refactored 与 UE 5.8 的当次 source。严重级与任务边界见 `CHECKLIST.md` BF134。仅登记有源事实、可达 target consumer 与可重复触发条件的 P2 及以上差异；旧结论不代替 fresh 审计。

当次覆盖 inventory：146 个源 C++/header（ALS 112、ALSCamera 12、ALSEditor 16、ALSExtras 6），181 个 target Rust，15 个 installed AnimBP root、359 个生成依赖文件和 15 份安装 manifest，文件缺失 0。源 revision 为 ALS `b754d6f0f2bb03741d301f8fb88077ebfe561e17`、UE `396c9f059903aed5fec78ecd3d437a40c6415368`；完整相对路径和 SHA-256 在本地证据 `target/bf134/inventory.json`。B_Als_Character 原生 reader 39.050 秒成功；watchdog self-test 通过。Editor/AI/网络与刻意差异按执行清单排除。

## 覆盖

| 移植闭包 | 本次 source/target 核对 | 尚未核销边界 |
|---|---|---|
| Character/CMC、状态、输入、出生、运动基座 | 实际源码位置与入口清单正在核对 | 算法/配置/坐标/生命周期 |
| Roll/Mantle/Ragdoll/GetUp/Stop 与 montage | 待执行 | 动作门槛、时钟、根运动、退出与重建 |
| AnimBP/FSM、cached/linked Pose、BlendSpace、曲线与同步 | 待执行 | 全递归 source 与当前 runtime consumer |
| Notifies、脚步与贴花 | 待执行 | 消息区间、过滤、所属实例、实际 shader |
| Control Rig、骨架与 IK | 待执行 | 源 RigVM/坐标/当帧 Pose/生命周期 |
| PhysicsAsset、关节、CCD、时钟与回读 | 待执行 | 实际求解器配置与 consumer |
| ALSCamera、相机动画、shake 与 trace | 待执行 | 实例、时钟、坐标、碰撞与 view |
| Overlay、Extras 场景/材质/UI | 待执行 | 资产/装配/输入与显示的源合同 |

## 已确认差异

### BF134.01A — P2：地面刹车低速未归零

- 源事实：当次本地 UE `UCharacterMovementComponent::ApplyVelocityBraking` 在非零刹车后将不超过 10 cm/s 的速度归零；仅摩擦时仍按源长度平方的小数阈值归零。无刹车/摩擦和小于 MIN_TICK_TIME 的 Delta 不触发归零。ALS 的 CalcVelocity 调用该源函数。
- Target：`grounded_velocity_step_with_braking_friction` 仅限制本次减速度，不应用源停止阈值；`grounded_dynamics_motor_system` 与 stop-distance prediction 直接消费，松开输入的低速尾帧仍移动。
- 漏检：已有刹车 Gate 检查 3.75–4 m/s 与超速转向，未检查源 0.1 m/s 停止边界及实际 motor。
- 反例：实际 motor 在 0.08 m/s、0.001 秒、源非零刹车下仅降低 0.015 m/s，仍输出 0.065 m/s；定向 Rust 测试稳定失败（exit 101）。修复后 15 项 dynamics 回归通过；阈值来自当次原生角色和 UE 源常量，经既有离线工具生成，其他已生成 movement 常量 diff 为零。
- 修复：只消费离线生成的源停止阈值和 MIN_TICK_TIME，按源处理无刹车/纯摩擦/超速保速，并让模拟循环处理精确 minimum Tick。实际 motor、0/微小/精确 Tick、停止门槛两侧、纯摩擦/零刹车和超速控制通过；精确 Tick 旧值 `(0.08, 0 step)`、源期望 `(0, 1 step)` 的二次漏检先 red 后修复。
- 消融/静态：移除停止阈值与 minimum Delta 条件分别稳定 red，逐字节/mtime 恢复；15 项最终回归通过。fresh 全量 15 根/39 source/374 产物完整 diff=0，345 当前节点规格与全部递归 source closure 通过；原始导出未改写。
- 真实入口：`target_game/play` 最终版 146.507 秒构建；公共 startup、Grounded 15/15、StandingStop 10/10 功能 Gate 均通过，graph/primary failure/fault/temporal 异常为零。证据：`target/animation_visual/bf134/01a/{startup,grounded,standing_stop}`。截图关闭，完整性能的 max_eval_p95_ms 失败保留，不宣称性能验收。
- 状态：已修复并验收。工具/profile/摘要 schema 的中途失败已保留，不计为语义反例；报告仅提取判定/覆盖，逐帧 trace 留在原始证据。
