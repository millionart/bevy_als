# BF144.537 StepDown 完全扫空

角色先保持 Walking。第 40 帧用同一初态与 50 ms 单步前进，实际胶囊撞到 20 mm 偏侧凸台。主台半宽分别为 100 和 145 cm，凸台保持相同。原生关闭观察的 43 帧运动、状态与姿态一致，状态对象引用仅规整临时世界序号。

100 cm 案中，UE 仅发出凸台的首个阻挡 Hit，完成 31.25 cm 前进并耗尽约 49.8 cm 下扫，从约 2.85133 m 转 Falling。同一场景以移动组件使用的 ComponentSweepMulti 复验原下扫端点，返回零 Hit。目标真实 StepUp 重放到相同的下扫区域，胶囊扫空，提前返回旧进度与滑动。跳过 D295 的目的地 floor 守卫仍返回同一结果，因为尚未进入该守卫。实际第 40 帧目标约 2.92582 m 转 Air，横偏约 5.17 cm。归 D296。

145 cm 对照中两端都命中可走主台并保持 Walking。目标 CMC 候选有效，后续实际横向偏移约 2.88 cm，原始证据保留供单独归因。本批结论限定扫空拒绝路径。共用 Rig、骨架、配置和自测沿用 .522 的相同证据。截图为 0，性能验收暂停。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/537/run_python.ps1 -Name verify`，用 `ALS_UE_SOURCE_ROOT` 提供本地 UE 源码。原生复现用 `prepare_request`、`prepare_reader`、`build.ps1 -Label miss`、`native.ps1 -Label wall -Request wall`。目标用 `observer`、`compile.ps1 -Label miss`，运行 `playground.ps1 -Label miss -HalfWidthCm 100 -Frame 40` 与 `-Label supported -HalfWidthCm 145 -Frame 40`，再分别运行功能验证。重采用空输出目录。
