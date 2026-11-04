# BF144.523 斜墙接触提前发布落地

第 49 帧以 .521 的 Falling 起点、速度和步长初始化两端角色。低台是静态可行走矩形，台顶约低于该帧脚底 1.35 cm。原生有台与无台两案的第 49 帧位置和速度完全相同，均为 Falling；有台第 50 帧为 Walking，无台继续 Falling。

Bevy 的独立同输入查询及生产 slide 精确复现实际事务。第 49 帧只有斜墙命中，低台只出现在状态同步的地面查询中，求解器接触为空，Tnua 无地面位移。正常路径已发布 Grounded、清除竖直速度并更新 Root 标签。只在状态同步前清除墙 contact 的控制保留同一命中、位置和下落速度，当帧 Air，第 50 帧实际触地才 Grounded。无台保持 Air。恢复观测源码字节并链接同一批库后，关键两帧的完整观察及运动参数相同，同一恢复二进制复跑时，运动参数最早从第 55 帧的方向与计划速度起不同。五案共用的 Rig、骨架、配置和状态读取自测与 .522 逐字节相同，沿用其 Git 证据。

早期夹具中旧地面观察钩子会读取已经按 Air 路径清空的传感器，故未在此案安装。诊断器在 Last 请求退出，公共 Gate 也在 Last 写报告。一次无定序采集缺报告，一次直接配置私有系统类型 set 被 Bevy 拒绝；最终在注册退出系统时令其先于已有报告刷新系统。最终各案均用公共功能 Gate，截图为 0，性能验收暂停。

使用 `pwsh -NoProfile -File docs/audit/bf144/523/run_python.ps1 -Name verify` 复验归档，以环境变量 `ALS_UE_SOURCE_ROOT` 注入本地 UE 源码。原生复现依次调用 `prepare_request`、`prepare_reader`、`build.ps1 -Label deck`、`native.ps1 -Label wall -Request wall`。Playground 复现依次调用 `observer`、`compile.ps1 -Label baseline`，以 `-HalfWidthCm 100 -Frame 49 -WallCm 162` 运行 `wall-deck` 和 `wall-only`、各自的 `functional.ps1`，再运行 `controls.ps1`。重采使用空输出目录，封存数据可直接验证。
