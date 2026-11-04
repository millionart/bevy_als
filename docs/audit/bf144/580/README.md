# BF144.580 Falling 虚拟夹缝脱困

默认胶囊从向下 2 m/s 开始，50 ms 帧先碰 40° 窄台，再碰右侧竖墙。原生第一碰的斜面法线允许行走，15 cm 窄支撑失败。紧墙组两次碰撞的拉回时间为零，沿交线只移动约 1 mm，仍保持 Falling。

UE 两次独立原生运行都把竖直末速度设为 1.05 m/s。水平随机结果不同。一次向上尝试被竖墙挡住，下一帧上升 2.41 cm；另一次在本帧直接上升 5.25 cm。Playground 紧墙组查询也看到同一斜台和竖墙，末速度竖直为零，下一帧没有上升。

放宽墙缝时第二碰有正时间且进展超过门槛，移除墙时只有第一碰，两组原生都未触发向上速度。三组 Playground 功能验证通过。差异登记为 D315。D19 和 D308 的强制落地分支另有记录。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/580/run_python.ps1 -Name verify`。重采时先备空输出目录，原生依次运行 `prepare_request`、`prepare_reader`、`build.ps1 -Label ditch`，再用同包运行 `native.ps1 -Label ditch -Request ditch` 和 `native.ps1 -Label repeat -Request ditch`。目标依次运行 `observer`、`compile.ps1 -Label ditch`，再运行三组 `playground.ps1` 和对应的 `functional.ps1`。截图 0，性能验收暂停。
