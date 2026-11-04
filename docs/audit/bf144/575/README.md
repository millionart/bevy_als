# BF144.575 台外侧向轨迹的共同输入

同 574 的实际 Playground 和前向/高度控制。旧原生请求使用纯 +X，目标自然组 q54 的控制 yaw 为 −π/2，运动实际采用已发布视角，产生非零 Z 输入。自然组 q54 的侧向速度和 q58 的位移可由该输入按原生有符号对照比例复算。

共同纯轴在两端 q52–58 产生零侧向位移和速度。正负组只在 q54 注入相反的 2⁻¹⁰ 侧向分量，空中段侧向速度差低于 10⁻⁸ m/s，至落地的侧向位置差小于 3 µm。落地帧仍有约 30–75 µm/s 的速度余差，作为交接待归因边界。原生请求与目标自然组的实际世界方向不同，已有数据足以解释自然组的毫米级侧偏。相机算法按任务范围排除。

四组公共功能报告保留 572 已归因的骨盆告警。此 Gate 只验证共同输入下的侧向离地、空中响应及残差边界。截图 0，性能验收暂停。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/575/run_python.ps1 -Name verify`。重采使用空输出目录。先运行 `prepare_request`、`native.ps1 -Label lateral -Request lateral`，使用 572 的原生包与身份。目标依次运行 `observer`、`compile.ps1 -Label lateral`，再运行 `playground.ps1` 的 `natural`、`axial`、`positive`、`negative` 及其 `functional.ps1`。
