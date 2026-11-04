# BF144.552 停滞子步结果的实际消费

复用 551 停滞组。q40 时 UE 有输入，角色速度为零，Moving 与 MovingSmooth 都为 false，VelocityBlend 四方向为零。q41 才观察到原生移动。UE 首次前向请求的轴向速度为约 6.35 m/s，原生代码在退出 Walking 循环前保存这次请求。

Bevy 生产基线继续执行第二子步，实际发布 0.04832 m/s，Moving 与 MovingSmooth 都为 true。VelocityBlend 前向权重约 0.2166、右向约 0.1235。事务末次碰撞前输入是约 0.225 m/s。

在实际 Playground 中，干预仅替换该事务发布的位置、反馈速度和碰撞前输入，采用原生产首步的零位移、零反馈与约 6.35 m/s 输入。马达和原碰撞事务的读数保持相同。截断组保持输入，真实发布速度归零，两项 Moving 标志变为 false，VelocityBlend 四方向归零。关闭干预后，q40 及下一帧的接触读数恢复基线。该轴向场景验证缓存值、运动状态和速度混合消费者，证据补入 BF144-D305。

三组功能 Gate 通过，截图为 0，性能验收暂停。封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/552/run_python.ps1 -Name verify`。重采使用空输出目录，依次运行 `observer`、`compile.ps1 -Label handoff`、`playground.ps1` 的 `baseline`、`stop` 与 `restored` 及各自的 `functional.ps1`。原生复用 551，通用 profile、Rig、骨架和自测复用 522。
