# BF144.536 实际 StepDown 命中复用

角色先在 100 cm 主台上稳定 Walking，第 40 帧用同一初态执行 30 ms 单步，18 cm 高台的前缘分别在 130 与 137 cm。原生关闭观察的 43 帧运动、状态和姿态一致，状态对象引用仅规整临时世界序号。

近案原生 StepDown 的真实可走阻挡 Hit 随 FloorResult 被 Walking 复用。实际 CurrentFloor 的 TraceStart、TraceEnd 和 Hit 与下扫一致，距离从约 1.0498 cm 调到 2.15 cm。目标重查通过后提交跨步，并在状态同步调整高度，帧末与原生相差约 0.6 µm。

边缘案原生有效阻挡 Hit 经窄胶囊失败后仍为 blocking，walkable 为 false。源保留跨步，第 40 帧从约 3.00885 m 的真实 StepDown 停止点转 Falling。目标重查同向下扫候选时返回 None，退到较低位置并保持 Grounded。帧末宽扫掠仍选高台边缘，窄胶囊由原台支持。独立重放只关掉目的地 None 的拒绝，恢复目标计算的高处跨步候选；它用于隔离拒绝点，实际运动报告保留原结果。归 D295。

首次观察器生成因断言换行样式不符中止，已保留报告并按实际文本更正。共用 Rig、骨架、配置和自测沿用 .522 的相同证据。截图为 0，性能验收暂停。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/536/run_python.ps1 -Name verify`，用 `ALS_UE_SOURCE_ROOT` 提供本地 UE 源码。原生复现用 `prepare_request`、`prepare_reader`、`build.ps1 -Label step-down`、`native.ps1 -Label wall -Request wall`。目标用 `observer`、`compile.ps1 -Label step-down`，以 `-HalfWidthCm 100 -Frame 40` 运行 `near` 和 `edge` 及各自功能验证。重采用空输出目录。
