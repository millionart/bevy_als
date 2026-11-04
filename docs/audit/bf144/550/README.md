# BF144.550 跨步提交后的贴地扫掠受阻

已 Walking 角色在 q40 以相同 50 ms 输入撞 44 cm 台阶。两案只改顶板底面高度。两组 UE 都完成抬升、水平前进和同一次下扫，且实际提交并保持 Walking。

UE 随后把下扫停点向平均地面间隙调整。高顶组执行完整的 11.00 mm。低顶组的向上扫掠命中顶板底面，实际只抬高 4.90 mm，仍保留台阶支撑和水平终点。

Bevy 两案的下扫、后续地面检查和限高均通过。低顶组在预置候选高度时与顶板重叠，生产路径因而撤销跨步，水平终点比 UE 落后 292.84 mm。只读重放使用下扫拉回停点，保留原重叠检查，得到合格位置并返回跨步候选。高顶组生产路径提交，最终位置与 UE 相差小于 0.01 mm。证据归入 BF144-D298 的贴地时序。两组功能 Gate 通过，实际物理解算运行生产路径。截图为 0，性能验收暂停。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/550/run_python.ps1 -Name verify`。重采使用空输出目录，原生依次运行 `prepare_request`、`prepare_reader`、`build.ps1 -Label roof`、`native.ps1 -Label roof -Request roof`。目标依次运行 `observer`、`compile.ps1 -Label roof`、`playground.ps1` 的 `limited` 与 `clear` 及各自的 `functional.ps1`。通用 profile、Rig、骨架和自测复用 522 的相同文件。
