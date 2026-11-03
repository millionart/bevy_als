# BF144.488 足部世界旋转与父骨正非均匀缩放

源端独立按本地 UE 正缩放 TQS 算法复算 634 个 Socket。目标端通过锁定版本的 TransformHelper 复算 636 个真实世界矩阵及足目标状态。源端保留逐骨归一化。对照只把 ik_foot_root 的 scale 替换为原竖直分量的等轴值，保留世界、局部 T/Q 和其余 scale。源 Q 不变，目标的最大方向差使用单位化后的两个四元数计算。

在仓库根目录设置 ALS_UE_SOURCE_ROOT，执行 `run_python.ps1 -Name prepare` 和 `run_replay.ps1 -Label baseline -Build`。将 `controlled_scale` 改为返回输入 scale 后，`no-scale-toggle` 同库重编应使源位置与目标旋转的对照差异归零并触发断言。恢复原源码字节并重编，`restored` 输出与基线字节相同。连续轨迹和锁定消费者另核。
