# BF144.487 D277 足锁定与 VM

从实测世界矩阵、历史锁状态和设置恢复生产足锁定的输入。公开 helper 来自同一 target_game 库，私有约束和重定位函数以及 RigInput 发布语句从当前生产源码提取。318 帧的实际足状态和 RigInput 精确相等。

组件空间归一化开关使用 BF144.486 同一 TQS 算法的两组旋转，固定实际足位置，并与实测四元数保持相同符号约定。两组结果间的差异才计入 D277。两组 VM 均保留实测连续前态，替换当帧克隆的 RigInput 旋转，并各自用原场景几何重新求交。外部仿射采样误差与连续反事实传播仍是单独的边界。

在仓库根目录设置 ALS_UE_SOURCE_ROOT，执行 `run_python.ps1 -Name prepare` 与 `run_replay.ps1 -Label baseline -Build`。把 `let paired = [1, 0].map` 改为 `let paired = [1, 1].map` 后重编运行 `no-component-effect`，两组 RigInput 差异应归零并触发断言。恢复原字节重编后，`restored` 的三份报告应与基线字节相同。
