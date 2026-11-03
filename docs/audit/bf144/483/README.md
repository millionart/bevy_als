# BF144.483 Rig、Root 和骨骼发布

目标重放使用 459 独立 VM 的初始化 local 与 482 独立复算的后续 local。按实际骨路径从 Rig Source 重建 pose，把 79 骨的 F64 变换交给公开 BonePose 构造并保留旧权重。未映射的场景节点、骨骼路径、时间戳和 root motion 依照已采集的输入保留。输出逐值匹配 319 帧实际 Rig pose。使用实际 [1,0] 权重调用公开 blend_source_poses 后，按 BoneId 比较的 Root 逐值匹配已有图输出。骨骼资产身份用原始包核验。

源端 319 份已捕获的独立 Rig 输出与 Root 全包一致，Rig Source 的曲线原样保留。源 Mesh 在普通帧的发布路径中先归一化旋转，再按紧凑骨索引写入 BoneSpaceTransforms。对 318 帧原生 Root 的 25,122 个变换按这个顺序复算，全部等于实际 mesh local。源的 17,363 个旋转改变，其中 40 个改变 F32 投影。初始化包的 mesh 缓冲和已求值 Root 处于不同阶段，独立保留。

目标通过 PostPhysicsAnimationPlugin 的生产注册入口建立写入系统，在隔离 schedule 中执行同一公共入口。读回 319 帧共 25,520 个实体变换，全部匹配输入 Root。关闭首帧 refresh gate 时，实体保留原值。此夹具只执行写入，输入取既有真实 Root，不重跑角色或查询世界。

D276 的对照固定同一份目标 Root F32，把四元数提升到源基与 F64 后执行已原生校准的末端归一化，再投回 F32。11,552 个已发布旋转与该对照不同，最大分量差 1.1920928955078125e-7。ik_foot_l 有 79 个，ik_foot_r 有 75 个。query99 左 IK 脚发布 w 为 0.7065211534500122，源末端对照为 0.7065210938453674。D245 已包含在冻结 Root 中，本次验证的是其后实体写入。D248 的源算术已由原生最终 mesh 校准。当前证据到 local 发布，foot 世界采样、锁定和后续反馈仍待核。

源端 318 份下一帧 proxy 曲线与前帧 Root 全包及 engine-frame 时点一致。目标依照 Root context 投影顺序检查 319 帧已观察的曲线数、站立和接地值。误用当前帧曲线时，首个有反馈的帧数值计数从实测 14 变为 26。目标的其他曲线消费字段留待补全观测。

初版夹具把两种 HashMap 当成同一类型，并尝试直接构造含私有字段的 context。改用公开默认构造和迭代收集。Root 校验初版以内部 Vec 顺序比较，改按 BoneId 核完整通道。保留失败报告。分别消融源 mesh 归一化、固定 Root 的末端对照、前帧反馈时点，三个最小反例均失败。源码和输入按字节恢复后，同 target 重编得到完整报告字节一致。

从项目根运行 run_python.ps1 的 facts、prepare、identity、finalize。独立复跑前用 ablate -Case restore 恢复源码及输入，再用 run_replay.ps1 -Label restored -Build。消融名为 native-final、held-final、prior-feedback。Git 目录保留压缩原始 SHA、全部对照、最小反例和身份记录。截图关闭，性能验收暂停。
