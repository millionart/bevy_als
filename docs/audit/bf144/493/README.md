# BF144.493 虚拟足骨的真实角色姿态和足锁定

原生按 BF144.457 的前 158 个普通帧输入运行，虚拟足骨设置仅作用于当前动画实例。第一轮 80 帧还未进入运动段，其初始化和 80 帧记录与扩展运行的前缀完全一致。扩展原始报告保留全部证据。按 BF144.491 的 TQS 复算所选虚拟足骨世界目标，并核对实际最终姿态、Getter 和 RigInput。

目标使用 BF144.484 的可见 Playground，保存预热和 80 个受控运动帧。虚拟模式仅在角色添加时写选择，默认模式完整记录与旧场景同长度前缀一致。独立 FootIk 重放固定各自实测前态与发布姿态。满权重 Root 对照使用实际公开的 blend_source_poses，虚拟足骨的额外旋转归入 D245。

在仓库根目录运行 `run_python.ps1 -Name prepare`。原生使用 `native.ps1 -Label virtual-motion -Request virtual-motion`，目标用 `compile.ps1 -Label observed` 后对 `virtual` 和 `default` 分别运行 `playground.ps1` 与 `functional.ps1`。复跑分析时解压对应原始报告，执行 `socket_prepare.py`、`target_prepare.py` 和 `run_replay.ps1`。将生成夹具里的足选择固定为 true 时，虚拟运行的 158 个有效帧都触发反例。将满权重控制改为直接传输入时，已测目标 Root 旋转不再匹配。恢复字节并用同库重编后输出一致。
