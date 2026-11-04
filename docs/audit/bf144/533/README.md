# BF144.533 Walking 支撑撤销与余时

角色先在静态台站稳，第 40 帧只对齐位置、速度及 60 ms 步长。原生模式委托记录真实 Walking→Falling 的位置与速度。关闭观察的 43 帧运动、状态和姿态相同，状态对象引用仅规整临时世界序号。对切换位置以 Walking 范围重查宽/窄，合并结果则由已进入 Falling 的公开查询给出；两种范围的相对 Hit.Time 有差异，实际对象、撞击点和地面距离一致。

100 cm 台首步时宽命中可走、窄胶囊无命中，两端第 40 帧把余下约 30 ms 用于 Falling。108 cm 台首步保留支撑并调整高度，第二步宽命中仍可走但窄胶囊无命中，两端第 40 帧 Air、竖直速度仍为零。125 cm 台两步均保留支撑，第 41 帧才走离且该帧竖直速度也为零。目标第一步与事务末端由生产 floor 重放校准，前两案关闭窄胶囊的诊断会保留原宽命中。

第 40 帧三案的位置差均在微米级。125 cm 台第 41 帧的水平速度差约 0.01278 m/s，可由原生缓存 7.60424 m/s² 和目标 fresh 7.98731 m/s² 复算，补入 D07。共用 Rig、骨架、配置和自测沿用 .522 的相同证据。截图为 0，性能验收暂停。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/533/run_python.ps1 -Name verify`，通过 `ALS_UE_SOURCE_ROOT` 提供本地 UE 源码。原生复现依次调用 `prepare_request`、`prepare_reader`、`build.ps1 -Label walking`、`native.ps1 -Label wall -Request wall`。目标用 `observer`、`compile.ps1 -Label walking`，分别运行 `playground.ps1 -Label width-100 -HalfWidthCm 100 -Frame 40` 及 108/125 的对应标签和台宽，再运行 `functional.ps1`。重采用空输出目录。
