# BF144.564 Falling 的地面限速缓存

两端角色实际以 Running 在地面预热，分别保持蹲伏和站姿。q49 受控进入无遮挡 Air，统一为 2.0 m/s 初速、50 ms 和前向输入。源和目标在空中恢复站姿胶囊。原生和目标采集均使用真实角色运动阶段。

原生蹲伏组离地前的 MaxWalkSpeed 为 2.0 m/s，胶囊半高从 56 cm 恢复到 90 cm 后，Falling 的 GetMaxSpeed 仍为 2.0 m/s，连续记录至 q51。首空中帧速度保持 2.0 m/s。站姿组的缓存上限为 3.75 m/s，同帧升至 2.15 m/s。

Bevy 在蹲伏预热时实际使用 2.0 m/s；q49 的 Falling 读到当前站姿，重新选 3.75 m/s。生产发布 2.15 m/s，水平位置比原生多前进约 3.75 mm。只读重放保留同一目标初速、输入与 3.0 m/s² 空中加速度，只把上限换成 2.0 m/s，速度保持 2.0 m/s。站姿对照两端的水平结果吻合。此证据登记为 BF144-D311，独立于 D287 的空中加速度缓存。

两组功能 Gate 通过，截图为 0，性能验收暂停。封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/564/run_python.ps1 -Name verify`。重采使用空输出目录，原生依次运行 `prepare_request`、`prepare_reader`、`build.ps1 -Label speed`、`native.ps1 -Label speed -Request speed`。目标依次运行 `observer`、`compile.ps1 -Label speed`、`playground.ps1` 的 `crouched` 与 `standing` 及各自的 `functional.ps1`。
