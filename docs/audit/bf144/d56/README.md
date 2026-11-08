# BF144-D56 自动 Mantle 安装帧

fresh 源角色 MovementComponent 的 `bTickBeforeOwner=True`；CMC 完成本帧运动后，角色 Tick 才自动检测并安装 Mantle RMS。新 RMS 初始时间为零，先加入 Pending，下一次 CMC Prepare 才推进。fresh InAir 所用 High Sequence 的 `bEnableRootMotion=False`。原生请求、输出、reader 身份及源码闭包保存在本目录。

目标把自动检测放在本帧 CMC 运动之后；成功安装时先准备根运动但不消费时间，下一帧再推进。手动入口保留原时序；已写倍率 0/1/2、手动入口、落地后不自动启动和暂停对照。测试、消融和动画 Gate 待统一验证。此实现仍沿用既有 ActualMotion 快照计算探测方向，快速方向变化未在本项复测；旧清单所引 `target/bf144/17/initial/` 已不存在，其原始报告缺失。
