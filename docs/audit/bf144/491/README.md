# BF144.491 原生虚拟足骨开关与构造生命周期

CharacterAnimationProbe 可选 `initial_use_foot_ik_bones`，要求已有的姿态边界观察。设置只写入当前动画实例拥有的瞬态副本。帧内可选 `use_foot_ik_bones`，并在该帧世界 tick 前写入副本。默认请求沿用原设置。两种起始设置各执行两次切换，读取 Getter、实际 RigInput、FootRootBone 和原生事件。Socket 由当时已发布的骨姿态和当前 mesh world 独立复算。

在仓库根目录设置 ALS_UE_SOURCE_ROOT，执行 `run_python.ps1 -Name prepare` 和 `run_python.ps1 -Name facts`。在迁移前读取 `legacy-before`。当前 reader 编译为 `build-baseline`，运行 `switch-baseline`，将当前源码字节备份到工具临时目录的 `reader.final.cpp` 后执行 `controls.ps1`。逐个删除初始或帧内写入，恢复后重建同一 Mac package。对 `switch-baseline`、`switch-restored` 与两个消融运行 `run_python.ps1 -Name socket_prepare -Case <label>`。以同库 `run_replay.ps1 -Label switch-baseline -Build` 和 `run_replay.ps1 -Label switch-restored` 复算原生 Socket。保留的原始 JSON 压缩文件可直接解压供分析复跑。

原生已构造的足根在四次开关切换时保持原值，Getter、外部 RigInput 和 Socket 则当帧变化。目标已构造实例的同输入连续 VM 对照是后续条目。缺少标记的原生运行也保留独立边界。
