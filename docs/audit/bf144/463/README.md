# BF144.463：实际636 trace标量余差的因果分离

沿461完整实际target unit/查询输入与462资格化的source同参数API结果，本项保留636次原公开RigUnitContext调用、原两块Floor、原F32 RigQueryContext，全部请求/命中/10 after参数继续逐值当前。源参考为同一源实例/实际输入产生的world ray/hit、实际world transform和真实unit结果，不用VM output回填模型。

fresh源TransformPosition/InverseTransformPosition/InverseTransformVector使用cross形式旋转，当前glam使用另一种仅在恰好单位quat时数学等价的展开式。本批context含允许误差内的近单位yaw quat，scale非任意假设为1；源垂直轴scale为1、其余实际分量原样保留。独立从原生world hit.Z减实际world translation.Z重算全部636份局部高度，与native ToVM.Z精确，再按源厘米float输出精确真实unit。这里所有native normal为Up、坡度修正0，没有宣称一般pitch/roll或坡面算法都已验证。

| 控制（其余保持原target） | offset匹配source |
|---|---:|
| 原公开unit与原查询 | 48/636 |
| 仅source normal逆旋转 | 92/636 |
| normal和point逆旋转 | 600/636 |
| 上一行再用source forward旋转生成ray，保留原F32查询/两Floor | 636/636 |
| 原生同参数world ray输入同一F32查询，保留source逆旋转 | 636/636 |
| 原生world hit数据控制及source逆旋转 | 636/636 |

三个阶段分别新增44/508/36个匹配，原匹配全部保留，无反向退化。source forward只改变44个完整query结果，其中36个高度变化；它与原生world ray输入同公开F32查询时，636个完整hit结果全相同。由此本批588个原scalar差均落在已知D247的normal/point/forward向量算子链，最后36个高度差也不能只凭近零误差就归到D246。

世界精度边界独立保留：使用相同原生ray输入后，target1272个水平命中分量恰为F32量化的native ray origin，636个完整world point仍不等于原生world hit，垂直分量全相同。源native hit.X/Y少量并不恰等于其ray origin的F64数值，原始反例保留；没有将源单Box和target两Box的物理表示声称全等，也不将全部源F64内部求交余差归于F32转换。

对同一原生物理高度做厘米float/米float对照，本批636个source投影值全相同；将当前hit放入source厘米模型的结果又恰等于source point逆旋转控制。因此删除两个无独立失败信号的重复scalar控制输出，保留独立source厘米模型作API资格。没有为本批新增D165标量消费者或保留额外runtime分支。

必要性消融：把source forward换回原transform_point，完整source offset/normal匹配从636回600；去source point逆旋转回92，两者都保留全部636个当前公开unit结果，且最小合同退出101。源码按bytes恢复、同六库target重编后，完整636份报告与canonical逐字节相同。normal必要性已由461同一636输入的消融验证，本项未重复新增该能力。

复验从项目根目录：`run_python.ps1 -Name prepare`连接636输入和native表；`run_replay.ps1 -Label <new> -Build`重新编译校准；`run_python.ps1 -Name check`与`-Name finalize`核对已存控制/分区/恢复。以replay.no-source-forward.rs或replay.no-source-point.rs覆盖replay.rs重编，合同必须退出101；按replay.rs.final恢复后重编必须通过。facts/identity需要本机配置及ALS_UE_SOURCE_ROOT。所有自动流程复用有自测的固定wall/no-progress watchdog，编译/运行/检查的机器报告保留。

长期输入、完整输出、事实、源码/消融/恢复与身份清单均受Git管理；临时binary/普通日志不是唯一证据。395+462 reader与六库保持；生产、reader、转换器、引擎、原资产均未改。无新D，累计272（116P2/156P3）未修。截图0，性能暂停。

下一步仅把已资格化的trace向量控制接入完整连续public VM诊断，并沿实际spring与Rig output消费链量化影响；其它unit数学和初始数据仍分别保留。636个scalar/normal恢复不等于整个Rig/Root或ALS等价。
