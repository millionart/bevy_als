# BF144.541 不可走 StepDown 的条件

保 .537 侧凸台与真实 Walking 初态，第 40 帧 50 ms 单步。下扫坡为 60°。低处顺向、低处逆向、高处顺向使用相同的首凸台和前进输入。原生关闭观察的 43 帧运动、状态与姿态一致，状态对象引用只规整临时世界序号。

低处顺向实际提交下扫，ImpactNormal 与前进点积为 0，原始 Hit 高度 88 cm，低于旧位置 92.15 cm，也在边缘容差内。UE 保留完整前进与下扫，约 2.89050 m 转 Falling。低处逆向和高处顺向的同场景下扫重查分别命中逆向和高于旧位的拒绝条件。两案真实移动中不存在这次完整的下扫命中，后续命中来自回退后的路径。

目标三案都实际撞凸台，向下命中法线竖直约 0.5 的斜面，随后按角度回退。只关后置 floor 守卫无效，只关角度则走到后置 floor 拒绝；两个都关的独立重放才返回抬高候选。低处顺向实际目标约 2.99215 m 转 Air，归 D299。高处控制最终源 Walking、目标 Air，作为回退后滑动的独立待证边界。共用 Rig、骨架、配置和自测沿用 .522 的相同证据。截图为 0，性能验收暂停。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/541/run_python.ps1 -Name verify`，用 `ALS_UE_SOURCE_ROOT` 提供本地 UE 源码。原生重采用 `prepare_request`、`prepare_reader`、`build.ps1 -Label slopes`、`native.ps1 -Label wall -Request wall`。目标用 `observer`、`compile.ps1 -Label slopes`，以 `-HalfWidthCm 100 -Frame 40` 运行 `allow`、`opposed`、`high` 及各自功能验证。重采用空输出目录。
