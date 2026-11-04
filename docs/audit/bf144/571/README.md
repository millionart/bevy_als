# BF144.571 到期帧再次落地

q40 普通落地采用无输入制动因子 3。q41 到 q75 以每帧 31.25 ms 推进。q56 的 0.05 非零输入使 q57 的第二次落地采用 0.5，q57 当前输入已经松开。对照组在 q57 继续 Walking，原制动计时器照常到期。

两个引擎在 q56 达到 0.5 s 时均保留因子 3。q57 运动前仍为 3，帧末单落地组清零，再落地组改为 0.5。新的因子在 q73 刚到期限时仍有效，q74 在运动后清零。核对范围是普通落地计时及输入快照，新增差异 0。q39 只确认尚无输入与制动，跨引擎共同步长从 q40 开始。

125 ms 步长的全量输入和低量输入两轮均出现骨盆位移跳变，原生记录、请求和失败功能报告均保留。最终改用精确二进制步长 31.25 ms，以 16 步跨越同一 0.5 s 截止条件，功能阈值保持。 17 cm 间隙的短步对照在 q58 才落地，两端 q57 都先清零，再在 q58 设为 3。主测试只把第二次落地初始间隙缩到 15 cm，确认 q57 当帧落地。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/571/run_python.ps1 -Name verify`。重采用空输出目录，原生依次运行 `prepare_request`、`prepare_reader`、`build.ps1 -Label timer`、`native.ps1 -Label timer -Request timer`。目标依次运行 `observer`、`compile.ps1 -Label timer`，再运行 `playground.ps1` 的 `double`、`single` 和对应的 `functional.ps1`。截图 0，性能验收暂停。
