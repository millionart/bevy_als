# BF144.566 Walking 两个子步间更换基座

100 ms 的 Walking 帧从静止旧台出发，第一步到窄台，第二步走离。两组只切换窄台的横向速度。原生额外运行同输入的 50 ms 单步，确认实际基座已设为窄台。Playground 记录相同首步水平位置的 floor 查询及真实双步末端。

两端双步末端位置在 2 μm 内一致。移动台使 UE 的 Falling 继承 1 m/s 横向速度，静止台控制为零。Bevy 两案发布的横向速度都约为零。ALS 的 Walking 循环逐步更新 MovementBase，目标只在循环外捕获初始支持速度。归 BF144-D313。D33 的速度来源和 D312 的跳键时机分别有独立记录。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/566/run_python.ps1 -Name verify`。重采时先备好空输出目录。原生依次运行 `prepare_request`、`prepare_reader`、`build.ps1 -Label bases`、`native.ps1 -Label bases -Request bases`。目标依次运行 `observer`、`compile.ps1 -Label bases`、`playground.ps1` 的 `moving` 与 `still` 及各自的 `functional.ps1`。截图 0，性能验收暂停。
