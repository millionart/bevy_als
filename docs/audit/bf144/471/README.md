# BF144.471：完整源足旋转与同数据public primitive因素

fresh本地源链为FindBetweenVectors→calf/foot/initial-relative quaternion组合→Quat4d Rotator→三组float angle限位→Rotator3d SIMD构造→组合/ARM64 Normalize。使用469独立normal历史、470独立initial-relative和469a精确消费骨姿态，684个完整最终FootRotation全部native逐double精确，包含受控倾斜；最终native只作oracle，没有回填任何中间值。

每条记录initial-relative、offset/current/target calf-space quat、current/target Euler、float limited angles、final-space quat、pre-normal/result。初版源公式的完整阶段与输出已有资格。另以同一source物理输入/初始global、连续公开unit state运行684次current public，独立current模型逐值重现全部最终输出，current/initial骨getter均先逐值核对；不能把该单元夹具称作完整Rig运行。

mask 31为全部源路径、0为全部当前路径；bit1是UE基product，bit2是source normalizer，bit8是source constructor，bit16是source FindBetween路径。全部当前同数据有677 double/1 F32最终差；只换current product有633 double/0 F32；只换current normalizer有466 double/1 F32；只换current constructor有346 double/0 F32；只换current FindBetween有1 double/0 F32。FindBetween current与保源原始构造仅换current normalizer的offset阶段全684相同，因此该小差不另编号。因素不按数量相加。

actual q141/pc98给出独立于D242输入数据的F32反例：源quat.x=0.03631163202674857，同数据public/current normalizer为0.036311632028529606；其它分量和全部阶段保留。物理只将source NEON reciprocal改标准1/sqrt（保source Dot4）也在该行改变F32，源精确合同失败；Dot4/方法相消的double数量不同，不能混作同一修复。归D248实际FootRotation消费者，不声称当前真实输入或最终local已因此恢复。

设计消融删除重复实现：源Euler代码改现有rig_euler的基映射后，684 native及全部primitive因素的所有阶段/公开common记录逐字节相同；源手写product改现有库UE基求值后同样全报告字节相同。最终删去这两重复函数和无效Euler因素，保留4104有效阶段记录。只资格本次输入/因素域，不撤销D163其它Euler半转或乘法输入域。source constructor改库构造只有338/684 native精确，所以保留有明确失败边界的源分组。

必要性控制：删除source constructor、NEON reciprocal均使完整原生合同失败101；去current angle扩展上下限使真实public重放合同失败101，保留动画现态边界。恢复最小源码bytes、同target六库重编，保留的所有阶段/公共调用与原型精确；仅删去已证无效的Euler variant，未删任何数值失败行。

复验从项目根：本目录run_python.ps1 -Name prepare、run_replay.ps1 -Label <new> -Build、run_python.ps1 -Name compare；-Case current-f32-contract重现当前同数据F32失败。保存的三个必要性消融入口可替换重编，最后按replay.rs.final bytes恢复并同target重编。无效实现的两次删除实验完整报告和原型均保留。facts/identity用本机配置及ALS_UE_SOURCE_ROOT；自动流程沿用已有self-test的固定wall/no-progress watchdog。

source facts、fresh原生、输入/全部阶段/当前public结果、最小反例、消融恢复及身份存Git目录。大静态报告无损gzip，解压长度/SHA通过；396含469a reader/六库保持。只补D163/D248/D251实际消费者，无新D，累计272，生产/reader/引擎/转换器/原资产不改，截图0、性能验收暂停。

后续仍需当前实际输入的数据/算法组合、weighted SetRotation、IK/global→local写回与完整Rig/Root反馈。本项完整源unit校准不证明它们通过。
