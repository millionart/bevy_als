# BF144.549 台阶高度与角色中心位移

已 Walking 角色在 q40 以 50 ms 输入撞到平顶台阶。台阶高度分别为 44.6 cm 和 44.9 cm，另取 44.9 cm 台阶、0.3 cm 起始贴地间隙作为紧间隙组。三组的原生下扫命中点均低于 45 cm。原生都提交跨步并保持 Walking。紧间隙组下扫拉回后的中心抬高为 44.70 cm，提交后的贴地调整把最终抬高增至 46.70 cm。

Bevy 三组的只读生产重放均找到合格下扫命中和后续地面。44.6 cm 组的候选中心抬高约 44.80 cm，通过守卫。44.9 cm 的普通与紧间隙组，候选中心抬高分别为 45.10 cm 和 46.95 cm，生产事务因额外中心位移守卫撤销。两组只读重放仅移除这项守卫后继续提交，保留 45 cm 命中点高度和其余落地检查。把目标的 2.4 cm 候选间隙单独减到源平均 2.15 cm 时，紧间隙组仍高于额外守卫门槛。证据记录为 BF144-D304。

三组实际功能 Gate 通过，物理解算运行生产路径。截图为 0，性能验收暂停。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/549/run_python.ps1 -Name verify`。重采用空输出目录，原生依次运行 `prepare_request`、`prepare_reader`、`build.ps1 -Label height`、`native.ps1 -Label height -Request height`。目标依次运行 `observer`、`compile.ps1 -Label height`、`playground.ps1` 的 `lower`、`near-limit` 与 `tight-limit` 及各自的 `functional.ps1`。通用 profile、Rig、骨架和自测复用 522 的相同文件。
