# BF144.559 第三次扫掠零时间命中后的侧移

q49 无输入下落，50 ms 内依次撞两面直墙，再下扫水平地面。两案只改地面高度。原生前两次碰撞同值，墙法线的竖直分量均为零，地面是可走面。

较高地面案的第三次命中拉回为 Time=0。UE 朝第一面墙的外侧实际横移 1 cm，高度与第三次停止点一致。侧扫无遮挡后仍 Falling，竖直速度约 −1.491 m/s。较低地面案第三次命中时间为正，原生当帧 Walking，竖直速度归零。

Bevy 同初态查询在较高地面也得到第三次扫掠零推进、有效地面及无遮挡的 1 cm 侧向查询。生产路径在物理解算前已 Grounded，最终竖直速度为零，X 保留在第三次停止点附近。较低对照两端都落地。Bevy 两案落地后仍观察到额外的沿第二面墙方向位移，该后续速度交接单独保留待审。本案以 X 侧移、落地模式和竖直速度验证 BF144-D308。

首次目标预热在测试帧前触发自动 Mantle，进程和诊断保留。改用静止预热后的两组实际功能 Gate 通过，q49 空中初态与几何保持。截图为 0，性能验收暂停。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/559/run_python.ps1 -Name verify`。重采使用空输出目录，原生依次运行 `prepare_request`、`prepare_reader`、`build.ps1 -Label third`、`native.ps1 -Label third -Request third`。目标依次运行 `observer`、`compile.ps1 -Label third`、`playground.ps1` 的 `stuck` 与 `positive` 及各自的 `functional.ps1`。
