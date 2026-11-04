# BF144.524 微倾墙面落地资格

复用 .523 的 Air 初态、输入、台高和 45° 偏航，只把墙面 Pitch 改为 −0.003° 或 +0.003°。同输入 Chaos sweep 返回真实正、负竖直法线。正值约 0.00005236，低于源落地回查的 0.0001。UE 两案均在第 49 帧保持 Falling，第 50 帧实际触地转 Walking。

正微倾在 Bevy 的实际命中竖直分量与解析值一致。第 49 帧只撞墙，脚底仍距低台约 1.35 cm，求解器无接触，控制器无地面位移。目标因正法线置位并在状态同步发布 Grounded。反向微倾保持 Air 到第 50 帧。仅清正微倾墙 contact 的独立控制保持入射、命中、滑动、位置和有效低台，角色到第 50 帧实际触地才落地。这个见证的正号由几何本身决定。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/524/run_python.ps1 -Name verify`，通过 `ALS_UE_SOURCE_ROOT` 提供本地 UE 源码。原生复现依次运行 `prepare_request`、`prepare_reader`、`build.ps1 -Label pitch`、`native.ps1 -Label wall -Request wall` 和 `geometry.ps1`。Playground 复现运行 `observer`、`compile.ps1 -Label baseline`，分别以 `-HalfWidthCm 100 -Frame 49 -WallCm 162` 运行 `positive` 和 `negative` 及对应功能 Gate，再运行 `controls.ps1`。重采用空输出目录。截图为 0，性能验收暂停。
