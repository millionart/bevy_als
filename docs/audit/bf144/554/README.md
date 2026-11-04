# BF144.554 悬留低台的离地马达

复用 553 的 45.05 cm 低台。原生从 q40 贴地到 q47，q48 走过远端并由 Walking 切到 Falling，竖直速度仍为零。q49 才在重力下达到 −0.327 m/s。

Bevy 悬留组从 q40 到 q47 保持旧高度、Grounded 和零竖直速度。q47 与 q48 的实际移动基座和传感器支撑都是低台。q47 的水平事务仍找到候选地面，因此抵消垂直马达。q48 从悬留高度无法在该事务的距离内扫到低台边缘，留下原马达加速度。Tnua 的支撑偏移约 0.448 m、弹簧系数 400、重力补偿 9.81 m/s²，得到约 −169.39 m/s² 的马达输入。计入重力及实际时间步后，复算与物理后及发布的 −5.9733 m/s 相符。

仅在 q40 补做贴地的实跑对照，在 q48 仍有水平事务的临时低台边缘候选，因而抵消垂直马达。物理后的正式地面查询仍丢失地面，同帧进入 Air，竖直速度为零。关闭干预后，q48 的下冲在 0.00003 m/s 内复现，竖直位置在 0.001 mm 内复现，侧向位置有约 0.32 mm 的重复余差。证据补入 BF144-D306 与 BF144-D288，三组功能 Gate 通过。截图为 0，性能验收暂停。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/554/run_python.ps1 -Name verify`。重采用空输出目录，原生先运行 `prepare_request`，再用 553 的同哈希包执行 `native.ps1 -Label departure -Request departure`。目标依次运行 `observer`、`compile.ps1 -Label edge`、`playground.ps1` 的 `beyond`、`beyond-adjusted` 与 `beyond-restored` 及各自的 `functional.ps1`。
