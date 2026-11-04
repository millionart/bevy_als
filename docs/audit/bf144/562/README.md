# BF144.562 侧移仍受阻后的强制落地

q49 无输入，角色在 50 ms 内先撞两面直墙，再向下扫到 60° 不可走坡。两案只改变坡面间隙。原生前两次命中及第三次下扫均拉回原位；随后侧扫向第一墙外侧请求 1 cm，仍命中同一陡坡。

阻挡组的原生侧扫 Time=0，尽管原生地面查询无可走面，仍在该帧进入 Walking。间隙稍大时侧扫有正推进，角色保持 Falling。Bevy 同胶囊诊断在两案分别测得侧扫零推进和正推进，正式地面查询均无有效支撑；生产两案均保持 Air。本证据补入 BF144-D308，覆盖同一侧扫分支的另一种状态结果，新增差异为零。

首轮的原生初始斜面重叠改变了碰撞起点，等距近墙轮次则使两端首墙顺序不同。保留两轮原报告和几何。最终错开原始墙距，确认两端首墙顺序一致，两个功能 Gate 通过。截图为 0，性能验收暂停。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/562/run_python.ps1 -Name verify`。重采使用空目录，原生依次运行 `prepare_request`、`prepare_reader`、`build.ps1 -Label side`、`native.ps1 -Label side -Request side`。目标依次运行 `observer`、`compile.ps1 -Label side`、`playground.ps1` 的 `blocked` 与 `escape` 及对应 `functional.ps1`。
