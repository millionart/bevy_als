# BF144.526 墙顶边缘与 Walking 余时

初态、输入、墙底和 45° 偏航沿用此前见证。UE 命中监听记录实际胶囊 Hit，并按 TraceStart、TraceEnd 和 pullback 后 Time 取得移动停止位置。打开和关闭监听的 52 帧运动、状态和姿态相同，状态对象引用仅规整临时世界序号。

16 cm 墙顶的第一 Hit 落在边缘容差内，胶囊 Normal 与几何 ImpactNormal 不等，独立 floor 查询为墙顶的有效胶囊结果。第 49 帧随后执行纯水平 Walking sweep 和 StepUp。源剩余 30.85581772 ms，Walking sweep 为 20.05628152 cm，与该余时乘最终水平速度相符。Bevy 在真实事务中只有一次 Falling 墙顶碰撞和滑动，PostUpdate 发布 Grounded，位置不再变动。只清 contact 保持这一帧位置和速度，状态仍 Air。此处是 D292。29 cm 墙顶两法线也不等，但实际移动停止位置在边缘容差外。源第 51 帧才 Walking，目标第 49 帧发布 Grounded，归 D291。

加深低台的对照仍支撑墙顶，49/50 两帧运动、命中与 floor 读数一致。原生姿态有变化，完整原始报告保留。这组对照未建立 2.4 cm 地面距离的独立见证。消融恢复后两关键帧一致，完整参数最早从第 54 帧分开。共用 Rig、骨架、配置和自测沿用 .522 的逐字节相同证据。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/526/run_python.ps1 -Name verify`，用环境变量 `ALS_UE_SOURCE_ROOT` 提供本地 UE 源码。原生复现依次调用 `prepare_request`、`prepare_reader`、`build.ps1 -Label edges`、`native.ps1 -Label wall -Request wall`。Playground 复现依次调用 `observer`、`compile.ps1 -Label edges`，分别用 `-HalfWidthCm 100 -Frame 49 -WallCm 162` 运行 `edge-in`、`edge-out`、`edge-gap` 及对应 `functional.ps1`，再运行 `controls.ps1`。重采用空输出目录。截图为 0，性能验收暂停。
