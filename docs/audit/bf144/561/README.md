# BF144.561 第三次碰撞落地的余时

同一 Air 角色在 q49 以 50 ms 无输入斜落，先撞竖直墙。原生通过改变地面高度得到两类路径。低地面时，第二次撞不可走坡，第三次正时间命中平地。较高地面时，第二次直接命中平地。

UE 第三次命中时仍有约 21.6 ms 几何余时，但实际只提交落地。落地通知与帧末位置和速度相同，水平偏转速度约 −0.57504 m/s。第二次直接落地时，角色在落地通知后又走约 20.98 mm，水平速度由约 2.0 m/s 降到 0.544 m/s，证明实际继续消耗余时。

Bevy 的第三次命中返回约 21.64 ms 给 Walking。同查询重放先逐值重建生产事务。两组受控实跑固定偏转速度、碰撞和落地位置。保留余时组执行 Walking 制动并发布零速。清零余时组保留约 −0.57524 m/s，两组位置相同，清零余时组的最终速度向量与 UE 相差小于 0.0003 m/s。本案隔离余时规则，登记 BF144-D310。三组功能 Gate 通过，截图为 0，性能验收暂停。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/561/run_python.ps1 -Name verify`。重采使用空输出目录，原生依次运行 `prepare_request`、`prepare_reader`、`build.ps1 -Label time`、`native.ps1 -Label time -Request time`。目标依次运行 `observer`、`compile.ps1 -Label time`、`playground.ps1` 的 `baseline`、`deflected` 与 `no-time` 及各自的 `functional.ps1`。
