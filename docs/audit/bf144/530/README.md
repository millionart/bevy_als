# BF144.530 中心射线 floor 的落地消费

同一 Falling 初态和 20.5 cm 墙。45° 薄挡板在角色对侧，阻挡向下胶囊，但中心射线可到低台。原生有挡板和无挡板的首墙 Hit 除后续 floor 查询字段外完全相同，法线及边缘资格通过。有挡板时低台是 line floor，独立 IsValidLandingSpot 为真，实际第 49、50 帧仍 Falling，第 51 帧才收到墙顶 Landed。无挡板时源第 49 帧收到低台 Landed。关闭原生观察的 52 帧运动、状态和姿态一致，状态对象引用只规整临时世界序号。

Bevy 三位置的公开查询和重放一致。实际状态同步前的胶囊扫掠命中不可走挡板，中心射线选低台，脚底仍距低台约 2.252 cm，无物理解算接触，目标第 49 帧发布 Grounded。消费端控制仅在当前选中结果来自 line 时抑制墙 contact。控制保持实际事务、候选结果及速度，第 49 帧仍 Air。无挡板胶囊对照继续 Grounded，关闭控制恢复原事件。诊断关闭中心射线会使查询另走 perch，属于另一个候选路径。当前差异是实际选中 line 时的资格，归 D293。

恢复后关键两帧一致，整段参数最早从第 56 帧分开。共用 Rig、骨架、配置和自测沿用 .522 的逐字节相同证据。截图为 0，性能验收暂停。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/530/run_python.ps1 -Name verify`，用 `ALS_UE_SOURCE_ROOT` 提供本地 UE 源码。原生复现依次调用 `prepare_request`、`prepare_reader`、`build.ps1 -Label line` 和 `native.ps1 -Label wall -Request wall`。Playground 复现用 `observer` 和 `compile.ps1`，以 `-HalfWidthCm 100 -Frame 49 -WallCm 162` 分别运行 `line`、`no-blocker`、`line-capsule-only`、`no-blocker-capsule-only`、`line-restored` 与各自 `functional.ps1`。重采用空输出目录。
