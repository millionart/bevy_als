# BF144.573 台外落地前的前向偏移

沿用 572 已封存的原生轨迹及同一可见 Playground 夹具。125 ms 的 q54 从相同 5 m/s 开始。原生用旧缓存 20 m/s²，Walking 50 ms 后以 6 m/s 进入 Falling，余时 75 ms，末速 6.225 m/s。目标默认消费本帧刷新值 9.6741 m/s²，末速 5.7087 m/s。两端空中段增加约 0.225 m/s，竖直速度一致。

控制组只在 q54 的真实 Walking 事务使用其自身上一轮缓存，事务后恢复刷新值。q54 至 q58 的前向位置与原生相差小于 5 µm，移除了原 q58 约 15.7 cm 偏移。归并 D07。31.25 ms 的 q59 也首次暴露同一输入阶段差异，当时两端仍 Walking，源缓存 20、目标刷新约 13.4136 m/s²。短步组仅定位首次差异，本轮控制只应用于长步组。

公共功能报告沿用原阈值，仍保留 572 已归因的姿态告警。该 Gate 验证前向位置和速度，其他轴和完整姿态仍按各自证据核对。截图 0，性能验收暂停。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/573/run_python.ps1 -Name verify`。重采目标时先备空输出目录，运行 `observer`、`compile.ps1 -Label cached`，再运行 `playground.ps1` 的 `baseline`、`cached` 及对应的 `functional.ps1`。原生报告及请求由 572 的封存清单绑定。
