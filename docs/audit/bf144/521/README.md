# BF144.521 斜置墙 Falling 法线

复用 .520 的 162 cm 墙、100 cm 台、长帧 49 和步行前态及 Air 缓存控制。在实际 Playground 事务后独立重放首个 Walking sweep 及唯一 Falling 命中，位置和速度与实际事务逐值一致，完整参数报告与 .520 对齐 Air 案逐字节一致。读取器使用这些查询的胶囊尺寸、起点、位移和同一墙体几何在 Chaos 中取原生法线与距离。45°墙的原生法线竖直为零，实际 Avian Falling 命中为 +0.00002726。两者均低于源空中输入限制的 0.001 阈值。

用生产滑动函数固定入射速度和余时，仅替换法线。对整帧原生 X 速度的差由 −0.00024117 降至 +0.00000107 m/s，剩余竖直差 −0.00002302 m/s 与 2.343 μs 的空中步长差相符。只替换命中距离时速度变化小于 0.0000003 m/s。该控制只作用于同一次滑动求值，保留了原事务前态与后续查询，结论限定于该实际见证。

运行 `pwsh -NoProfile -File docs/audit/bf144/521/run_replay.ps1 -Label baseline` 重算生产滑动，使用 `-Label ablated -Ablate` 移除法线替换，预期失败 101 并恢复源码字节，再运行 `-Label restored` 重编。三个构建均链接现有 `target_game/play` 库。复验主证据运行 `run_python.ps1 -Name verify`，需要用环境变量 `ALS_UE_SOURCE_ROOT` 指向本地 UE 源码；`identity` 核对六库和 396 份源、目标及工具身份。原生取证入口为 `prepare_geometry` 和 `native.ps1`。截图为 0，性能验收暂停。
