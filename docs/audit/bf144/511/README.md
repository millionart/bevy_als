# BF144.511 D285/D286 的实际 Standing Lean 消费

新读取的 Standing 原生图把 LeanState 的前向与右向绑定到 BS_Als_Lean。BlendSpace 输出连接 Apply Additive，并读取运行步态权重。Sprint 加速混合绑定独立属性。两组普通跑步见证的实际 Sprint 值为零，Sprint 混合节点未更新。

D285 复用 .510 的相同角色输入和阈值对照。序号 20 的 Lean 坐标变化传到 BlendSpace、叠加、Rig 输入和最终 Root，最终 Root 有 72 个骨骼记录改变。D286 将 .510 的八个限幅目标分别带入原阻尼，只有序号 61 和 63 保留单帧差。可见控制只在序号 61 替换 Standing 前向 Lean 读数一个 F32 步长，保持属性状态和原始物理输入。最终 Root 有 71 个骨骼记录改变。序号 62 的直接图与 Rig 输入姿态精确合拢，最终 Root 仍有 6 个骨骼记录改变。

`verify.py` 核对原生绑定、实际缓存、两组对照和恢复。五次原可见 Playground 功能通过，恢复后参数报告逐字节一致。初版观察器的缓存值生命周期错误已修正。记录的姿态差是逐值比较。
