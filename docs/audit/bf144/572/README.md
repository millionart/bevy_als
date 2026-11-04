# BF144.572 台外落地姿态报告归因

571 的长帧夹具在 q51 结束后没有恢复手动时钟，随后 q52 开始向前跑出低台。原始两份骨盆告警发生在 q58 和 q59 台外落地。新对照保留旧单落地前缀，随后分别用 10×125 ms 与 40×31.25 ms 保持 1.25 s 输入。Playground 使用原地面和角色，UE 增设同高度的平面供原生落地。两端各自计算运动与姿态。

125 ms 组两端都在 q58 台外落地。Rig 前骨盆局部位移为 UE 0.398886 m、Bevy 0.398231 m，均超过 0.35 m。最终姿态分别为 0.401566 m、0.400848 m。31.25 ms 组两端都在 q79 落地，三帧骨盆位移均低于 0.25 m。q80→81 右腿平面法线的 Rig 前点积为 UE −0.95086、Bevy −0.95526，Rig 后分别为 +0.96949 和 +0.97212。UE 膝局部角速度 32.07 rad/s，超过退化跨越豁免的 30 rad/s 上限。

公共功能报告仍按原阈值记录两组失败。此处通过的是两条报告信号的原生归因，运动轨迹和姿态逐值等价仍需各自合同支持。新增差异 0。截图 0，性能验收暂停。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/572/run_python.ps1 -Name verify`。重采使用空输出目录。原生依次运行 `prepare_request`、`prepare_reader`、`build.ps1 -Label pelvis` 和 `native.ps1 -Label pelvis -Request pelvis`。目标依次运行 `observer`、`compile.ps1 -Label pelvis`，再运行 `playground.ps1` 的 `coarse`、`fixed` 以及对应的 `functional.ps1`。两组公共功能校验保留非零退出。
