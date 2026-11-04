# BF144.568 Walking 前向初穿透后的速度

同一角色从 6.5 m/s 开始 Walking，100 ms 帧分两步。55 cm 缝隙在两侧各造成 2.5 cm 初始穿透，站立地面仍有效。UE 的阻挡委托记录两个零时间穿透，原生警告记录脱困失败，bJustTeleported 由 false 变 true。位置保持，速度保留为首步计算的 6.25 m/s。

真实 Playground 也在两侧重叠，所用前向扫掠在右墙零距离停止，保持位置及 Grounded。目标第一步计划速度同为 6.25 m/s，末端发布为 0。64 cm 缝隙的正时间命中不触发源保护，两端都按位移给出低速度。移除两墙后前向速度与水平位置吻合。

本项归 BF144-D314。D41 处理无可走地面时丢弃的穿透，D305 处理位置无进展后仍执行的子步。该反例的地面始终可走，差异为首次失败后的速度回写。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/568/run_python.ps1 -Name verify`。重采用空输出目录，原生依次运行 `prepare_request`、`prepare_reader`、`build.ps1 -Label stuck`、`native.ps1 -Label stuck -Request stuck`。目标依次运行 `observer`、`compile.ps1 -Label stuck`，再运行 `playground.ps1` 的 `stuck`、`wide`、`clear` 及各自的 `functional.ps1`。截图 0，性能验收暂停。
