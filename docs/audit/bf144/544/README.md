# BF144.544 StepUp 内部的第二次滑动

已 Walking 角色在 q40 以同一 50 ms 输入撞到低凸台。抬升后，前向扫掠撞第一面斜墙，胶囊随即沿墙滑动。双墙案只在首次滑动的路线上加第二面斜墙，两案的原生凸台命中和第一墙命中相同。

原生双墙案在首次滑动撞到第二墙后，根据实际法线和剩余时间计算新的位移。用引擎的正法线点积路径复算，结果等于第二墙停止点到实际 StepDown 起点的 64.42 mm 水平位移。两案都实际下扫命中凸台并保持 Walking。

Bevy 的实际事务也在抬升后扫到第一墙，再用一次滑动撞第二墙。它随即以该停止点下扫并成功提交，水平起点距原生 64.29 mm，q40 仍 Grounded。只读重放在同一停止点加入第二次扫掠，保留相同地面守卫，水平起点距原生缩到 0.026 mm。单墙案移除第二面，两个重放逐值一致，实际下扫水平起点距原生约 0.053 mm。此对照验证下扫起点，真实物理解算只运行生产路径。

差异记为 BF144-D300，发生在成功跨步内部。D15 的滑动发生于跨步撤销后，D297 的水平重放发生于落低后。功能 Gate 两案通过，截图为 0，性能验收暂停。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/544/run_python.ps1 -Name verify`。重采用空输出目录，原生依次运行 `prepare_request`、`prepare_reader`、`build.ps1 -Label corner`、`native.ps1 -Label corner -Request corner`。目标依次运行 `observer`、`compile.ps1 -Label corner`、`playground.ps1` 的 `one-wall` 与 `two-walls` 及各自的 `functional.ps1`。共用 profile、Rig、骨架和自测复用 522 的相同文件。
