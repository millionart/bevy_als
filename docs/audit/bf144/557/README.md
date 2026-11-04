# BF144.557 第二次空中碰撞的输入重算

同一 Air 角色在 q49 以 50 ms 向前斜落，先撞竖直墙，再撞 60° 不可走坡。两案只切换沿第一墙、朝第二坡的空中输入。第一墙法线竖直分量为零，两端第一次偏转保留沿墙输入。

UE 第二次碰撞以无空控速度重算第一墙偏转，再按两法线交线调整。该夹具中，受限空控朝回第一面墙，源的点积守卫阻止重新加入。原生实际两次命中、剩余时间、最终速度及终点可复算。有输入和无输入的第二次偏转速度近乎相同，竖直分量约 −0.981 m/s。

Bevy 生产路径继续用已含输入的速度计算第二次偏转，有输入组竖直约 −0.916 m/s。只读重放保留前两次命中及时间，第二次偏转单独换回无输入速度，与源最终速度向量余差小于 0.0004 m/s。无输入对照两条重放逐值相同。实际物理解算运行生产路径。证据记为 BF144-D307，覆盖第二次碰撞的重算。两组功能 Gate 通过，截图为 0，性能验收暂停。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/557/run_python.ps1 -Name verify`。重采使用空输出目录，原生依次运行 `prepare_request`、`prepare_reader`、`build.ps1 -Label corner`、`native.ps1 -Label corner -Request corner`。目标依次运行 `observer`、`compile.ps1 -Label corner`、`playground.ps1` 的 `toward` 与 `none` 及各自的 `functional.ps1`。
