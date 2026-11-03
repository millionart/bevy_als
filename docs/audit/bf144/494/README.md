# BF144.494 虚拟足根的正非均匀缩放

从 BF144.493 恢复两端实际虚拟足骨数据。源端使用 UE TQS 复算，目标用真实 TransformHelper。保持局部 T/Q 和其余缩放，仅将虚拟足根替换为其竖直分量的等轴缩放。左虚拟足骨自身的微小非均匀值始终保留。

固定实际足位置和逐帧前态，比较仅单位化四元数，以及替换父骨等轴对照所得的旋转。先逐值重放实际足状态和 RigInput，再追踪控制差异。目标仿射旋转变化和消费者结果归入 D278。

在仓库根目录运行 `run_python.ps1 -Name prepare` 和 `run_replay.ps1 -Label baseline -Build`。生成的 scale.rs 取消等轴化时，`no-scale-toggle` 的世界样本变化归零。将消费者取值从 uniform_q 改成 actual_q，`no-rotation-toggle` 的 RigInput 差异归零。恢复字节和同库重编后，两个 `restored` 报告应与基线完全相同。VM 下游另核。
