# BF144.581 Falling 近同法线二次命中的离墙微移

50 ms Falling 帧以相同初速撞第一面竖墙。双墙组沿首墙偏转后再命中第二面。0.5° 与 1.25° 分别位于源 `1 − dot < 10⁻⁴` 门槛两侧，另有单墙控制。两端都保持 Falling。

0.5° 组中，UE 第二碰后沿命中法线实际移动 0.1 mm，并发布约 5.824 mm/s 的法线速度。用发布速度乘原生第二碰余时 17.172 ms，逐值复算出同一微移。Playground 对应速度约为 0.000011 mm/s。1.25° 组的两端法线速度都接近零，单墙组没有第二次命中。两端扫掠时间有差别，归因用各端各自的命中法线，原生位移另以自身余时复算。

旧 BF144.06 已记下这个分支尚无独立触发证据。现行、归档和 Git 历史的相关条目核对见 [查重记录](history-review.md)。本次新增 D316（P3）。

封存复验从项目根运行 `pwsh -NoProfile -File docs/audit/bf144/581/run_python.ps1 -Name verify`。重采使用空输出目录。原生依次运行 `prepare_request`、`prepare_reader`、`build.ps1 -Label nudge` 和 `native.ps1 -Label nudge -Request nudge`；目标依次运行 `observer`、`compile.ps1 -Label nudge`、三组 `playground.ps1` 和对应的 `functional.ps1`。截图 0，性能验收暂停。
