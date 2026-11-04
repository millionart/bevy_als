# BF144.548 跨步的微推进门槛

已 Walking 角色在 q40 以同一 50 ms 输入撞到低凸台，抬升后撞到悬空的竖直墙。三案只改墙的前缘位置。墙面几何消除了有意义的切向滑动，能单独验证向前推进的撤销门槛。

UE 只在前向 Hit.Time 与 SlideAmount 均为零时撤销。零组的原生记录走跨步撤销后的外层滑动，使用同一墙面几何和原生拉回公式复算，内部推进为零。中间组原生实际拉回后推进约 0.198 mm，随后下扫命中凸台并提交。阈值以上组也提交。

Bevy 中间组的生产前向推进为 0.190 mm，平方约 3.61×10⁻⁸ m²，低于 f32::EPSILON，因此在下扫前撤销。只读重放仅对该轴向场景比较推进是否为零，继续下扫并通过后续落地检查。零组与阈值以上组的重放逐值相同。三组实际功能 Gate 通过，实际物理解算运行生产路径。差异记为 BF144-D303。截图为 0，性能验收暂停。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/548/run_python.ps1 -Name verify`。重采用空输出目录，原生依次运行 `prepare_request`、`prepare_reader`、`build.ps1 -Label stop`、`native.ps1 -Label stop -Request stop`。目标依次运行 `observer`、`compile.ps1 -Label stop`、`playground.ps1` 的 `zero`、`micro`、`advance` 及各自的 `functional.ps1`。通用 profile、Rig、骨架和自测复用 522 的相同文件。
