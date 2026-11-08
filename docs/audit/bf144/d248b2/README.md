# BF144-D248b2 Layered Pose 归一化方法

fresh 读取 `AB_Als_Layering` 的 5 个图，识别实际 LayeredBoneBlend 节点；其中脊柱节点使用 mesh-space 旋转及三级骨过滤。`layered.request.json` 以已封存的 current/source Head base pose 作受控输入，原生输出有 24 个零权重骨、两个 1/3 和 2/3 部分权重骨、53 个满权重骨。它证明该源节点的输入和分支，不把这组受控输入称作原始 Layering 运行历史。完整图、请求、原生输出和哈希均在本目录；压缩数据逐字节验证解压。

UE 在 mesh-space 部分混合后和转回局部时归一化。当前项目的 glam `Quat::lerp` 内部先归一化，调用者再归一化，最后局部重建再归一化；`.427` 已隔离方法与次数、乘法顺序和精度。目标 ARM64 对这三个现有阶段复用 D248b1 的 F32 estimate/refinement，同时保留现有短弧及权重逻辑；非 ARM 的 lerp 保留原实现。项目本地空间部分权重的 `BonePose` 也改用同一归一化 helper，其他 Pose 节点不在本项展开。

新增公开分层输出的共享归一化控制和反号四元数短弧控制；D248b1 的真实骨链 fixture 独立约束共享方法。旧 `.427` 的 target 原始报告缺失，保留其清单结论而不伪称重新取得。D232 精度、D238 次数、D163 乘法顺序、其他 Pose 消费者、非 ARM 源后端与完整 Root 仍未在此验证。focused 测试、消融恢复、图静态审计和公共 Playground Gate 待全部静态修复与 Bevy/依赖迁移后执行。
