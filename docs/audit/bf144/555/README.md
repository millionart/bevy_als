# BF144.555 原生顶点切分与实际碰顶

同一 Air 角色在 q49 以 0.2 m/s 上升，步长 50 ms，顶板间隙 1.5 mm。UE 两个有顶板的案例只切换 `p.ForceJumpPeakSubstep`，退出每案时恢复开关及设置优先级。另有启用切分的无顶板对照。

启用时，UE 先向上扫约 2.039 mm 至顶点，实际命中顶板底面并拉回，随后用剩余时间向下积分。关闭切分时同一顶板没有命中，帧末比启用组高 1.541 mm。无顶板与关闭切分的帧末位置一致到微米内。两次原生采集在加入命中观察前后，完整角色位置、速度及公共运动字段逐值相同。

Bevy 生产整步请求向下约 2.263 mm。有顶板和无顶板的帧末位置、速度逐值相同，与 UE 关闭切分的结果吻合。从同一真实初态向上扫至顶点，会在约 1.480 mm 处命中顶板。证据补入 BF144-D28，覆盖了原生真实命中与开关因果。两组功能 Gate 通过，截图为 0，性能验收暂停。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/555/run_python.ps1 -Name verify`。重采使用空输出目录，原生依次运行 `prepare_request`，用 `prepare_unobserved`、`build.ps1 -Label apex`、`native.ps1 -Label apex -Request apex` 采无观察基线，再用 `prepare_reader`、`build.ps1 -Label observed`、`native.ps1 -Label apex-observed -Request apex` 采命中。目标运行 `observer`、`compile.ps1 -Label apex`、`playground.ps1` 的 `ceiling` 与 `clear` 及各自的 `functional.ps1`。
