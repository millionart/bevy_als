# BF144.486 足部组件空间逐骨归一化

每侧按自己的骨骼索引从实际数据找到 root、ik_foot_root、ik_foot_l/r。原生样本复算有效前帧已发布姿态，目标样本通过同一锁定库的 TransformHelper 重放实际观察。两端各自的原输入和输出均保持独立。

四格对照固定世界变换和 TQS 算术，交叉切换组件空间归一化与 D276 发布归一化。目标对照将实际 F32 局部输入提升至源基，当前 mesh world 从原目标 armature 取得并在四格中冻结。世界旋转先同基投回 F32，再逐分量比较。同输入 TQS 数值对照与目标实际仿射结果分开统计。足锁定与连续轨迹传播留待后续审计。

在仓库根目录设置 ALS_UE_SOURCE_ROOT，然后执行 `run_python.ps1 -Name prepare` 和 `run_replay.ps1 -Label baseline -Build`。消融把 `if normalize` 改为 `if false && normalize`，同一 target_game 库重编应得到 634 侧原生失配（633 侧 Q、2 侧 T，合并去重）。恢复原源码字节并重编，`restored` 与基线应完全相同。最初使用跨引擎同索引的假设被真实骨骼表拒绝，最终以各自骨名和父链核验。
