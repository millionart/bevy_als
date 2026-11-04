# BF144.556 空中极小水平残速的双子步阈值

同一 Air 角色在 q49 以 60 ms 无输入下落，分两个 30 ms 子步。原生和目标都没有碰撞，竖直初速为 −0.2 m/s。水平初速分别为单轴 0.20 mm/s、双轴各 0.22 mm/s、双轴各 0.23 mm/s 及零速。

UE 使用水平速度平方和阈值。单轴 0.20 mm/s 和双轴各 0.22 mm/s 只推进首个子步，帧末水平速度归零。双轴各 0.23 mm/s 的合速度超过约 0.316 mm/s，两步均保留。用实际浮点步长复算原生水平位移，符合各组真实终点。

Bevy 生产事务实际执行两个子步，并在两组低速中继续保留第二步的位移和帧末残速。按世界位置的 F32 运算逐步复算，可重建目标实际水平终点。阈值以上、零速和各组竖直结果与 UE 吻合。四组功能 Gate 通过，证据补入 BF144-D22。截图为 0，性能验收暂停。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/556/run_python.ps1 -Name verify`。重采使用空输出目录，原生运行 `prepare_request` 后用 555 同哈希包执行 `native.ps1 -Label residual -Request residual`。目标依次运行 `observer`、`compile.ps1 -Label residual`、四个 `playground.ps1` 和对应的 `functional.ps1`。通用 profile、Rig、骨架和自测复用 522。
