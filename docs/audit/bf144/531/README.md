# BF144.531 窄胶囊替代与合成 line

同 .530 的墙、挡板及 Falling 初态，低台仅向 X 正方向横移 15 cm。源常规胶囊命中不可走挡板，未生成 line，15 cm 窄胶囊独立命中低台且也为非 line。源 FindFloor 合并时把地面对象与法线替换成低台，保留常规命中的撞击点、位置、时间与 1.7118 cm SweepDist，将 LineDist 设为 1.9 cm，并置 bLineTrace。合并结果单独通过 IsValidLandingSpot，实际第 49、50 帧无 Landed，第 51 帧才落地。原生关闭观察的 52 帧运动、状态与姿态一致，状态引用仅规整临时世界序号。

目标三个位置的真实中心射线均未命中，宽胶囊无效后窄胶囊选低台。公开查询与生产函数重放逐值一致。目标保留常规距离，使用窄胶囊的低台撞击点，line 标志为 false。第 49 帧没有求解器接触，却发布 Grounded。D293 的真实中心射线控制不改变该状态；只加 perch 替代的消费限制后，同一事务和候选保持，第 49 帧 Air。无挡板的胶囊对照在相同控制下仍 Grounded。此处归 D294。

恢复后关键两帧一致，完整参数最早在第 56 帧分开。共用 Rig、骨架、配置和自测沿用 .522 的逐字节相同证据。截图为 0，性能验收暂停。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/531/run_python.ps1 -Name verify`，用 `ALS_UE_SOURCE_ROOT` 提供本地 UE 源码。原生复现依次调用 `prepare_request`、`prepare_reader`、`build.ps1 -Label perch` 和 `native.ps1 -Label wall -Request wall`。Playground 复现用 `observer` 和 `compile.ps1 -Label perch`，以 `-HalfWidthCm 100 -Frame 49 -WallCm 162` 运行 `perch`、`no-blocker`、`perch-line-only`、`perch-merge-guard`、`perch-restored`、`no-blocker-merge-guard` 及各自功能验证。重采用空输出目录。
