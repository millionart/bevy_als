# BF144.509 General Lean 半衰期

原生 Grounded 和 InAir 读取同一 General.LeanInterpolationHalfLife，默认 0.2 秒，非正值直接采用当帧目标。Bevy 离线生成器读取了该字段，InAir provider 写入实际图输入。Grounded 使用 0.2 秒常量并仅在 pending 时立即取值。用新原生曲线重放离线转换，默认输出完整匹配现有资产，0 和 0.6 秒只改变半衰期字段。

原生诊断在实例中复制 Settings，分别以站立运动和自然下落验证 0.2、0、0.6 和负值。初次矩阵捕获姿态但缺直接 LeanState，补读后复查相同场景。原生负值通过诊断写入以覆盖非正值分支，编辑器字段约束从零开始。

可见 Playground 复用 .505 跳跃场景。在序号 32 和 44，将实际 InAir 半衰期输入替换为 0 或 0.6。Grounded 的连续更新保持相同输出，InAir 分别直接取目标或更慢接近。Grounded 的另一个单帧控制只使原 Lean 刷新走已有 pending 分支，并立即还原输入。恢复无控制入口后参数报告逐字节一致。

`verify.py` 核对两个原生分支、来源设置写入消融、实际目标输出与解压哈希。
