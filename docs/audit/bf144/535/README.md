# BF144.535 合格 perch 的平均地面距离钳制

有效窄胶囊要求 AboveBase 加窄距离不超限；钳制要求 AvgFloorDist 减宽距离加窄距离达到上限。两式同时成立要求 AboveBase 加宽距离不超过 AvgFloorDist。对默认半径 30 cm、perch 半径 15 cm 的常规正距离圆胶囊扫掠，宽命中边缘最小弧高约 4.019 cm，已超过 2.15 cm 平均间隙。此界限限定常规圆胶囊查询。

实际见证使用 0.3 等比胶囊，半径 9 cm、半高 27 cm。脚底间隙为 1 mm，中心距主台边缘 1 cm，两案均无水平输入、宽/窄命中可走且有效。47.0 cm 低台时两端保留约 0.156 cm 原宽距离，角色实际抬升约 1.99 cm；帧末高度差约 34 µm，与原宽查询距离差相符。47.35 cm 时两端保留同一宽命中并将距离钳为 2.15 cm，角色实际保持原高度和 Walking。目标仅关闭钳制的重放返回原宽距离，窄命中和资格不变。原生关闭观察的 43 帧运动、状态和姿态相同，状态对象引用只规整临时世界序号。

首轮目标在第 40 帧缩放模型，公开验收捕获骨段长度按 0.3 跳变。保留失败报告，改为角色生成时已经缩放后两案均通过。共用 Rig、骨架、配置和自测沿用 .522 的相同证据。截图为 0，性能验收暂停。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/535/run_python.ps1 -Name verify`，用 `ALS_UE_SOURCE_ROOT` 提供本地 UE 源码。原生复现用 `prepare_request`、`prepare_reader`、`build.ps1 -Label clamp`、`native.ps1 -Label wall -Request wall`。目标用 `observer`、`compile.ps1 -Label clamp`，以 `-HalfWidthCm 100 -Frame 40` 运行 `clamp-off`、`clamp-on` 及各自功能验证。重采用空输出目录。
