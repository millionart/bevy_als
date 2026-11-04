# BF144.551 Walking 零位移后的子步终止

已 Walking 角色在 q40 以同一 60 ms 输入正对一面禁止跨步的直墙。两案只改墙距。停滞组中，胶囊到墙仍有正距离，但第一次 30 ms 子步的拉回把实际推进减为零，完整位置保持不变。UE 在该子步后退出 Walking 循环，只留下首次命中，帧末速度为零。

Bevy 停滞组的第一次子步也精确无位移，地面间隙保持在有效带内。目标继续运行第二子步，从零速度加速到约 0.225 m/s，扫掠请求缩短到 6.75 mm。相同原始命中距离约 2.457 mm，第二次拉回约 1.007 mm，生产事务实际推进约 1.450 mm。同几何重放的完整终点与末次输入逐值重建生产读数，按首个位置未变的前缀截断时与 UE 起点吻合。墙距较大时两端都实际运行第二子步，保留两端各自的碰撞数值差。

本案验证位置逐值相等时遗漏的子步终止，登记为 BF144-D305。两组功能 Gate 通过，真实物理解算运行生产路径。截图为 0，性能验收暂停。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/551/run_python.ps1 -Name verify`。重采用空输出目录，原生依次运行 `prepare_request`、`prepare_reader`、`build.ps1 -Label stall`、`native.ps1 -Label stall -Request stall`。目标依次运行 `observer`、`compile.ps1 -Label stall`、`playground.ps1` 的 `stalled` 与 `moving` 及各自的 `functional.ps1`。通用 profile、Rig、骨架和自测复用 522 的相同文件。
