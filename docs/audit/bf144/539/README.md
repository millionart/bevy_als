# BF144.539 成功跨步的带内高度余差

同 145 cm 主台及 50 ms 第 40 帧，侧凸台顶为 17.3 cm。UE 真实首 Hit 命中凸台，抬高前进后真实下扫命中主台，拉回位置到地面 1.0498 cm。实际 CurrentFloor 复用该 Hit，并在 Walking 调整到 2.15 cm。原生关闭观察的 43 帧运动、状态与姿态一致，状态对象引用只规整临时世界序号。

目标自然跨步成功，凸台碰撞层始终启用。六个解算子步的速度等于入速，横向和竖直速度近零。目标在 CMC 预置 2.4 cm，实际重查间隙约 2.3785 cm，处于贴地容许带，状态同步保持高度。相对原生帧末高约 2.5158 mm。仅在目标物理结束后把间隙设为源真实拉回的 1.0498 cm，同 CMC、水平、解算和地面对象保持，目标正常调向平均间隙，余差降至约 0.2484 mm。关闭控制后完整参数逐字节相同。结果归 D298，剩余重查中心数值另作取证。

首轮观察器遇到 SpatialQuery 与 Position 写查询冲突，改用读取加调度内组件提交，并以独立目录复验。共用 Rig、骨架、配置和自测沿用 .522 的相同证据。截图为 0，性能验收暂停。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/539/run_python.ps1 -Name verify`，用 `ALS_UE_SOURCE_ROOT` 提供本地 UE 源码。原生重采用 `prepare_request`、`prepare_reader`、`build.ps1 -Label grazing`、`native.ps1 -Label wall -Request wall`，之后 `prepare_gap` 提取源间隙。目标用 `observer`、`compile.ps1 -Label grazing`，以 `-HalfWidthCm 145 -Frame 40` 运行 `grazing-verified`、`source-gap`、`restored` 及其功能验证。重采用空输出目录。
