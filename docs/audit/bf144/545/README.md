# BF144.545 跨步内部的近地滑动约束

已 Walking 角色在 q40 以相同 50 ms 输入撞低凸台，再于抬升后撞同一俯倾墙。两场景只改初始中心高 3.5 mm。原生旧地面均为平坦阻挡面，间隙分别为 1.8 cm 与 2.15 cm，前向墙接触法线的竖直分量约 −0.5。

UE 在 1.8 cm 组按近地条件压平法线，滑动保持水平。2.15 cm 组保留法线，滑动向下 65.14 mm。用原生法线、首撞后的余量复算，两组均吻合实际下扫起点。两组都实际下扫命中凸台并保持 Walking。

Bevy 两组都保留负竖直分量。1.8 cm 组的生产下扫起点比原生低 65.13 mm，水平相差 37.56 mm。只读重放仅启用近地条件后，起点竖直余差约 0.058 mm，水平余差约 0.076 mm。2.15 cm 组的重放逐值相同。两组目标功能 Gate 通过，实际物理解算运行生产路径。本案验证平坦旧地面上的负法线约束，记录为 BF144-D301。截图为 0，性能验收暂停。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/545/run_python.ps1 -Name verify`。重采用空输出目录，原生依次运行 `prepare_request`、`prepare_reader`、`build.ps1 -Label overhang`、`native.ps1 -Label overhang -Request overhang`。目标依次运行 `observer`、`compile.ps1 -Label overhang`，再运行 `playground.ps1` 的 `tight` 与 `open` 及各自的 `functional.ps1`。通用 profile、Rig、骨架和自测复用 522 的相同文件。
