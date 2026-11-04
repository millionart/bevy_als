# BF144.525 超过阈值的平墙与实际落地资格

同 .524 的空中初态、低台和 45° 偏航，墙 Pitch 改为 −0.03°。隔离原生读取包监听角色胶囊移动的 blocking-hit 委托，记录实际命中。监听开启与关闭的 52 帧运动、姿态和源状态相同。验证器只规整源状态对象引用的临时世界序号，原报告保存完整引用。

第 49 帧唯一原生阻挡命中是墙。竖直 Normal 为 0.000523598751，超过回查阈值，Normal 和 ImpactNormal 按 UE 的 Equals 相等。命中点也超出边缘容差。命中位置和帧末分别用公开 K2_FindFloor 重查，两处均返回低台的有效胶囊结果。该查询保存并恢复强制检查标志。墙命中的 Time 经源 pullback 为零，Hit.Location 仍保留查询命中位置。第 50 帧委托才记录低台实际命中，源进入 Walking。

Bevy 的独立命中及滑动与实际事务一致，第 49 帧无物理解算接触、无控制器地面位移，脚底离台约 1.36 cm，仍发布 Grounded。只清墙 contact 后，第 49 帧保持 Air，第 50 帧实际落地。恢复后 49/50 的观察和运动参数逐值相同，完整报告从第 55 帧有差异。此案同时不满足法线差异与边缘两项资格，尚需用真实边缘命中分别验证。共用 Rig、骨架、配置和自测与 .522 逐字节相同，沿用其证据。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/525/run_python.ps1 -Name verify`，用环境变量 `ALS_UE_SOURCE_ROOT` 提供本地 UE 源码。原生复现依次调用 `prepare_request`、`prepare_reader`、`build.ps1 -Label blocking-hits` 和 `native.ps1 -Label wall -Request wall`。Playground 复现依次调用 `observer`、`compile.ps1 -Label baseline`、`playground.ps1 -Label plane -HalfWidthCm 100 -Frame 49 -WallCm 162`、`functional.ps1 -Label plane` 和 `controls.ps1`。重采用空输出目录。截图为 0，性能验收暂停。
