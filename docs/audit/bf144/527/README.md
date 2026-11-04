# BF144.527 合格墙顶边缘与 2.4 cm 发布门槛

以相同 Air 初态、墙底、墙面偏航及低台运行 19、20.5、21.5、22.5、23.5 和 24.5 cm 墙顶。矩阵使用 .526 已验证的原生包和 Playground 二进制。19 与 20.5 cm 两案的实际源边缘都合格，胶囊 floor 来自墙顶，源第 49 帧继续 Walking。目标实际墙顶间隙约 2.072 与 2.556 cm，前者第 49 帧 Grounded，后者第 51 帧 Grounded。22.5 cm 案中两端的相应 floor 查询都返回低台，源也在 49 Walking，目标仍在 51 才 Grounded。

两次控制仅在物理结束和状态同步之间改变角色高度。20.5 cm 案下移 3 mm 后，重查墙顶间隙为 2.213 cm，当帧转 Grounded。19 cm 案上移 4.5 mm 后，重查间隙为 2.532 cm，当帧保 Air。碰撞事务报告、命中重放、计划速度、墙 contact、支撑对象、物理解算接触和控制器地面位移保持相同。关闭控制后 20.5 cm 案的完整参数逐字节一致。结果归入 D292。

21.5 cm 原生第一 Hit 通过边缘容差，独立查询找到非 line 的可行走墙顶，但角色仍在 Falling。该见证的后续 floor Hit 资格需要单独记录。共用 Rig、骨架、配置和自测沿用 .522 的逐字节相同证据。截图为 0，性能验收暂停。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/527/run_python.ps1 -Name verify`，通过 `ALS_UE_SOURCE_ROOT` 提供本地 UE 源码。重采先按 .526 重建其原生包和未启用控制的 observer，再用本目录 `prepare_request`、`native.ps1 -Label wall -Request wall`。矩阵运行 `playground.ps1 -Label top-190 -HalfWidthCm 100 -Frame 49 -WallCm 162`，依次替换标签高度的十分之一厘米整数并运行对应 `functional.ps1`。两向控制另运行本目录 `observer` 与 `compile.ps1 -Label gap-control`，使用 `top-205-lower`、`top-190-raise` 和 `top-205-restored`。重采用空输出目录。
