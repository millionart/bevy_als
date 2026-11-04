# BF144.553 普通 Walking 向下贴地限幅

已 Walking 角色在 q40 以同一 50 ms 输入走离高台。两案只改宽低台深度，分别为 44.7 cm 和 45.05 cm。水平移动没有阻挡，也未进入 StepUp。原生两案都从 Walking 范围的宽扫找到可走低台，实际下移并保留约 2.15 cm 地面间隙。

Bevy 在两案的物理后地面查询中同样找到低台。较浅组实际向下贴地。较深组的有效地面间隙约 47.20 cm，所需调整约 45.045 cm，超过普通地面同步的 45 cm 位移守卫。角色在 q40 保持 Grounded，但位置仍停在旧高度，比 UE 高约 45.05 cm。

受控组保留相同的物理后位置、地面查询和运动模式，仅在原同步之前补做无遮挡调整扫掠。Bevy 最终位置与 UE 相差约 0.05 mm。关闭干预后，旧高度和下一帧接触记录恢复基线。该守卫位于普通 Walking 的物理后同步，差异记为 BF144-D306。两端地面查询均有效，跨步候选高度守卫属于另一个阶段。

四组功能 Gate 通过，截图为 0，性能验收暂停。封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/553/run_python.ps1 -Name verify`。重采用空输出目录，原生依次运行 `prepare_request`、`prepare_reader`、`build.ps1 -Label lower`、`native.ps1 -Label lower -Request lower`。目标依次运行 `observer`、`compile.ps1 -Label lower`、`playground.ps1` 的 `within`、`beyond`、`beyond-adjusted` 与 `beyond-restored` 及各自的 `functional.ps1`。通用 profile、Rig、骨架和自测复用 522。
