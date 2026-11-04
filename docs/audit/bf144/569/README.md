# BF144.569 Walking 成功脱困后的实际速度方向

从 6.5 m/s 向右墙移动。单个 50 ms Walking 子步开始时右墙穿透 2.5 cm，左侧空间为空。UE 成功后退 2.625 cm，重扫命中右墙但已无穿透，bJustTeleported 保留该步向前的 6.25 m/s。

真实 Playground 的前向事务在原点被挡。物理阶段反向移动约 1.251 cm，地面同步又反向校正约 1.224 cm。发布的 −0.250225 m/s 等于第一段位移除以 50 ms。实际速度方向相对源反转 180°，旋转目标仍朝前。正时撞墙和无墙控制都保持向前。本项补入 BF144-D314，同一源速度回写保护的成功分支。旧 R5E-07 已隔离物理后的额外贴地校正，这次量化的是此前的物理解重叠。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/569/run_python.ps1 -Name verify`。重采用空输出目录，原生依次运行 `prepare_request`、`prepare_reader`、`build.ps1 -Label recovery`、`native.ps1 -Label recovery -Request recovery`。目标依次运行 `observer`、`compile.ps1 -Label recovery`，再运行 `playground.ps1` 的 `overlap`、`wide`、`clear` 及各自的 `functional.ps1`。截图 0，性能验收暂停。
