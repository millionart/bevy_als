# BF144.522 原斜墙 Falling contact 消费

同一可见 Playground 162 cm 墙、100 cm 台与序号 49。实际扫掠在物理后仍保留 `airborne_sweep_contact=true`。状态同步前对同一位置、缩放、胶囊与碰撞过滤调用生产 floor 查询，无合格地面，Avian 接触集合为空。状态同步消费并清空标志，发布事务速度并保持 Air。序号 49 仅发出 Grounded→Air，序号 50 无模式事件。原生同案两帧均为 MOVE_Falling，无可行走 floor。

控制在 PhysicsSystems::Last 之后、状态同步之前，只清序号 49 的标志。三次功能验收均通过，完整运动参数与 .521 逐字节一致。位置、速度、已发布速度、移动基座、模式及事件在两帧保持一致。恢复源码字节、同 target 库重编，四份观察快照与基线一致。这里验证的是无合格地面的原见证，有合格地面的同类斜墙命中需要单独验证。

封存复验使用 `pwsh -NoProfile -File docs/audit/bf144/522/run_python.ps1 -Name verify`，以环境变量 `ALS_UE_SOURCE_ROOT` 注入本地 UE 源码路径。重新采集使用 `observer`、`compile.ps1 -Label baseline`、`playground.ps1 -Label baseline -HalfWidthCm 100 -Frame 49 -WallCm 162` 和 `functional.ps1 -Label baseline`，然后执行 `controls.ps1`。采集脚本要求各运行的输出目录为空，现有 Git 证据可直接通过验证脚本复验。截图为 0，性能验收暂停。
