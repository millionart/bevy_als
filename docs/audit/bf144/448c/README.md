# BF144.448c 五边界完整余差与应用资格

同engine帧source318→target query1–318，以实际骨名匹配5×318×79=125610对骨骼。源raw Root/Head各边界经现行中性coordinate_basis的厘米→米、交换YZ与反射四元数合同投影F32，逐帧输出T/Q/S差异、曲线值/键集、额外scene节点。None scale原样单列；四元数原始分量与双覆盖符号对齐后的数值差分别统计，不单位化、不放宽容差。初始化包保留在.a/.b原始数据，此表只覆盖共同正步长帧。

这些是数据库存，不是把全部差归某个已知D。首Root pelvis source投影位置(0.00003921628012903966,0.9675060510635376,−0.030561532825231552)，target约(1.7007637193685825e−30,0.9675059914588928,−0.030561532825231552)。其它差异与完整状态/叶时钟史继续按直接producer核对，不凭相似字段归并。

额外SKM_Als的44次scale候选仍缺实际entity应用证据。fresh BAG skeleton遍历全部命名Transform，不能由Pose存在节点推断它具备AnimationTargetId/AnimatedBy；当前项目apply_animation_to_visible_targets还检查refresh_bones并按目标id匹配。现有.b未读这些组件与实际local Transform，故明确需要下一只读前置，不声称已写回、可见故障或新的ALS差异。源端本cohort五边界attribute_count全0，目标仅确认Pose类型没有该容器。

缺帧、错delta、错engine帧与缺target边界四负例拒；删除四元数基转换后最小基向量反例失败，源码SHA/字节恢复、同Python入口重跑后完整inventory与1590行table字节精确。删去无用math导入也保持原输出。396现行身份、六库、reader/final package与生产/资产未变，所有输入/输出/脚本长期存Git。本项不启动新runtime，复用.b真实入口functional证据。

复验：`pwsh docs/audit/bf144/448c/run_python.ps1 -Name compare`。截图0、性能暂停，无新编号，271未修；实际scene target读点与整体Root归因继续。
