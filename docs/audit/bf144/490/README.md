# BF144.490 足目标选择和缺失标记

本地 UE/ALS 源码给出两套选择名、共享设置和世界 Socket 缺失分支。原生与目标正式资产记录均具备两套标记。三个实际目标姿态分别装入生产 FootIkApplyPlugin 和 ControlRigApplyPlugin 的加载态夹具，复制两套系统间实际使用的 Head/Provider 顺序。夹具预先由公开骨映射器产生骨映射，以便测加载态切换。初次未预置映射的运行在首帧足状态无效，未作为移植差异。

完整骨架依次跑默认、false、true 和删除设置。然后只把左侧虚拟标记改名，仍执行实际骨映射刷新和足锁定；UE 缺失结果来自本地实现，使用夹具同一 mesh world 作为比较输入。公开 VM 各运行一次独立构造，确认两种设置的足根。非默认原生角色运行和切换后已构造实例的 VM 生命周期待核。

在仓库根目录设置 ALS_UE_SOURCE_ROOT，运行 `run_python.ps1 -Name prepare` 和 `run_replay.ps1 -Label baseline -Build`。强制写 true 应使六次 false 请求失配。保留虚拟标记应让缺失反例失败。恢复字节并重编后，`restored` 输出应与基线完全相同。
