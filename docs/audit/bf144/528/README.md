# BF144.528 墙顶回查的二次 floor Hit

隔离原生读取包在 .526 的实际墙碰撞观察之外，记录公开 K2_FindFloor 的完整 Hit、IsValidLandingSpot 和实际 Landed 委托。独立回查保留查询时的模式，调用资格函数后恢复强制检查标志。启用与关闭观察的 52 帧运动、状态和姿态一致，状态只规整临时世界序号。通过案的 floor 重查发生在帧末的 Walking 状态，真正被接受的命中由 Landed 委托单独保存。

21.5 cm 墙顶案的第一墙 Hit 通过边缘条件。回查得到的墙顶 floor Hit 阻挡、可走、未穿透，且自身也通过边缘容差。撞击高度 21.49998 cm 高于该 Hit 的下半球上界 21.03889 cm。公开 IsValidLandingSpot 返回 false，实际第 49、50 帧均无 Landed。20.5 cm 案的实际 Landed 来自墙顶，22.5 cm 案来自低台。

把 21.5 cm 案的低台抬高 5 mm，第一墙 Hit 保持逐值，实际 Landed 改由低台满足条件，源同帧用 35.83426 ms 余时 Walking 约 23.29227 cm。目标碰撞事务逐值不变，地面查询间隙由 2.764 降至 2.263 cm 后同帧发布 Grounded，状态发布前后位置相同。归 D292。目标地面查询的胶囊或射线来源需要后续取证。共用 Rig、骨架、配置和自测沿用 .522 的逐字节相同证据。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/528/run_python.ps1 -Name verify`，由 `ALS_UE_SOURCE_ROOT` 提供本地 UE 源码。原生复现依次调用 `prepare_request`、`prepare_reader`、`build.ps1 -Label floor-hit` 和 `native.ps1 -Label wall -Request wall`。目标沿用 .526 observer，运行本目录 `playground.ps1 -Label top-215 -HalfWidthCm 100 -Frame 49 -WallCm 162` 和 `top-215-raised` 及各自 `functional.ps1`。重采用空输出目录。截图为 0，性能验收暂停。
