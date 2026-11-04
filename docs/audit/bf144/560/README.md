# BF144.560 落地后 Walking 使用的速度

复用 559 第三命中时间为正的场景。q49 两端都实际落地。UE 在第二面墙后将空中速度投影到竖直方向，第三命中落地时水平速度为零。

Bevy 空中扫掠重放在第三命中前得到接近零的水平偏转速度。生产落地分支保留偏转前的 [4, 0, 4] m/s 水平输入，随后运行约 37.58 ms Walking。用当前源文件提取相同的地面扫掠和贴地算法，逐值重建生产物理前位置与反馈。原路径在物理前沿 Z 多走约 117.9 mm，并发布约 3.137 m/s。

对照只替换交给 Walking 的速度，保留相同碰撞、落地、余时和运动输入。重放中的 Walking 速度与反馈归零，位置保持在落地处。实际 Playground 发布这组结果后仍 Grounded，速度为零，终点距原生小于 0.3 mm。关闭干预逐值恢复基线。证据记为 BF144-D309。原生第三命中落地的时间预算与目标交接仍有独立差异，另行核对。三组功能 Gate 通过，截图为 0，性能验收暂停。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/560/run_python.ps1 -Name verify`。重采使用空输出目录，依次运行 `observer`、`compile.ps1 -Label handoff`、`playground.ps1` 的 `baseline`、`corrected` 与 `restored` 及各自的 `functional.ps1`。原生复用 559 正时间报告。
