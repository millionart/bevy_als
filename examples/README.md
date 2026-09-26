# 运行入口与专项检查

日常运行使用根目录 `run_manual.bat`。它启动 `manual_playground`，自动验收也在同一个 Playground 中运行：

```powershell
pwsh tools/run_animation_visual_validation.ps1 -Case startup -CaptureCount 0 -FunctionalOnly
pwsh tools/run_animation_visual_validation.ps1 -Case grounded -CaptureCount 0 -FunctionalOnly
```

`-Case` 的完整选项见该 runner。功能验收仍检查动画图错误、姿态、生命周期、场景覆盖和进展；性能结果另行保留。

| 入口 | 用途 |
|---|---|
| `manual_playground` | 当前交互场景和真实运行验收 |
| `als_graph_editor` | 图编辑器 |
| `animgraph_spec_dump` | 静态图审计所需的真实节点规格 |
| `*_contract_gate`、`*_sync_gate`、曲线/IK/输入/所有权等小型 Gate | 直接断言边界与源合同，仍用于 focused 回归和 TODO 中的图审计 |
| `scene_gate` | 场景 actor、材质、文字和运动组件的专项断言 |
| `als_hud_gate`、`debug_display_gate`、`show_debug_hud_gate` | 输入与调试 UI 的专项断言 |
| `bf_char14_movement_gate`、`character_assembly_gate`、`cmc_adapter_gate`、`grounded_dynamics_gate`、`platform_gate` | 方向矩阵、角色装配、运动参数与支撑体的专项断言 |

专项检查不是当前真实场景 Gate 的替代品；没有逐条证明被覆盖的断言予以保留。现有 `main` 形式的 example 需用 `cargo run --jobs 2 --example <name>` 显式执行，`cargo test` 不会执行其 `main`。

## 已删除的旧演示

2026-09-26 清理的 18 个例程可从 Git 历史恢复；历史 Gate 记录保持不变。以下是当前功能的检查入口，不声称复用旧脚本的所有中间数值。

| 旧例程 | 当前覆盖 |
|---|---|
| `actions_gate`、`air_gate`、`locomotion_gate`、`manual_runtime_gate`、`stop_gate` | 公共 `grounded`、`locomotion`、`standing_stop` 场景及保留的图合同检查 |
| `foot_ik_gate`、`mantle_gate`、`ragdoll_gate`、`roll_gate` | 公共 `foot_ik`、`base`、`ragdoll`、`get_up`、`grounded` 场景 |
| `stance_gate`、`motion_gate`、`state_switch` | 公共 `crouching`、`standing_direction`、`grounded` 和核心状态/输入测试 |
| `playground`、`playground_parity_gate`、`skeleton_boot` | 当前 `startup`，保留 `scene_gate` 与骨架/资产合同 |
| `curve_gate` | 曲线与节点单元测试，以及保留的 cubic/cycle/extrapolation/slot 曲线检查 |
| `perf_gate`、`sprint_stress_gate` | 公共 `sprint` 及公共视觉报告中的帧时间、图求值、非有限姿态和速度检查 |

旧演示加载的目标资产可能仍被合同检查、工具或待办引用；这次不删除资产。
