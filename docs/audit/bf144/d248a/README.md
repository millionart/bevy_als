# BF144-D248a Rig 双精度 ARM64 归一化

本地 UE 的双精度 quaternion 归一化使用 NEON Dot4：先分别以 FMA 组合源 X/Z、Y/W 两对，再相加；随后执行倒平方根估算和两轮 refinement。源四元数到 runtime 的映射是 `(-x,-z,-y,w)`，目标实现据此在 runtime 坐标保留相同 Dot4 分组。ARM64 使用对应 NEON 指令，其他架构保留原路径；零长度回退不变。

`normalize.request.json` 和 fresh 原生输出记录 identity 及四个实际足旋转，identity 的原生 w 为 `0.9999999999509513`。`publication.request.json` 另重采 D276 足骨发布输入，对应的原生结果投影到 runtime F32 与已有公开 writer fixture 一致。角色 Pose 发布之前会先转为源坐标；由于更新后的 helper 在 ARM64 已自行处理 runtime 轴序，该直接调用改为使用 runtime 四元数，非 ARM 路径保留原先的源坐标往返。此迁移维持既有发布阶段，不复开 D276。

完整原生请求、输出、源代码摘录与 SHA-256 在本目录；较大的读取结果已逐字节核对解压。新增 macOS ARM64 primitive 和层级调用 fixture，`.471/.480/.481` 保留独立 Rig/IK 消费者及历史消融。UE `Quat::Normalize` 默认阈值来自 float，而 `Transform::NormalizeRotation` 的 double 阈值为 `1e-8`；当前共享 helper 保持原阈值，极小边界尚无原生反例。非 ARM 源后端、D248b 的 F32 Pose 及所有目标执行尚未验证；focused 测试、消融恢复、图审计和公共 Playground Gate 待全部静态修复及 Bevy/依赖迁移后进行。
