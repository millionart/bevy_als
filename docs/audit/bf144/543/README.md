# BF144.543 从平地进入可走坡

已 Walking 角色在 q40 以同一 50 ms 输入离开平地的平坦接触，向 40° 上坡前进。三案均保留同一平台和见证墙。40° 坡只切换跨步许可，60° 坡保留禁止跨步。所有读数来自原生角色或实际 Playground 的对应帧。

UE 首次命中 40° 坡后，把 72.44% 的剩余时间用于新坡面切向。按原生首命中的几何面法线与时间复算，第二扫掠请求前进 22.64 cm、上升 19.00 cm，与见证墙收到的实际扫掠请求相符。两种跨步许可的实际命中、终点和模式相同。60° 坡未触发第二扫掠，也未撞见证墙。

Bevy 在 40° 坡上的两种许可组有相同首命中。禁止跨步时，生产事务只保留首撞推进，q40 水平位置比 UE 少约 186 mm。允许时，事务返回抬升与下扫候选，并由物理解算消费剩余水平位移。这组终点接近 UE，但中间运动阶段不同。60° 对照两端均停在首撞位置附近。三个目标功能 Gate 均通过，截图为 0，性能验收暂停。该见证补到 BF144-D13。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/543/run_python.ps1 -Name verify`。重采使用空输出目录，原生依次运行 `prepare_request`、`prepare_reader`、`build.ps1 -Label ramp`、`native.ps1 -Label ramp -Request ramp`。目标依次运行 `observer`、`compile.ps1 -Label ramp`，再跑三个 `playground.ps1` 和各自的 `functional.ps1`。共用 profile、Rig、骨架和自测复用 522 的相同文件。
