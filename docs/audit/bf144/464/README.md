# BF144.464：完整连续VM的trace控制与消费者影响

在459的真实319帧输入上，两个域分别初始化同一实际RigDefinition、Construction一次、每帧实际79骨Pose/curve/11 bindings与实际delta，working state连续保留。原域全部405寄存器×319=129195值、25201局部骨、13378次完整call参数/调用前骨、636个真实查询逐值当前。查询使用原两Floor的公开F32 RigQueryContext，不再回放已采hit。

受控域仅在FootOffsetTrace用463合格的source forward/normal/point旋转，其余全部仍调用冻结public units。每次trace的8个实际输入必须和463相同，完整10个after参数必须逐值合格控制；全部636通过。没有把source offset、after registers、working state或Pose直接注入VM。两域的完整call顺序、帧步骤数相同，输入和构造寿命相同。

直接消费者共1272份（每域636）逐项核验：pelvis SpringInterpV2 target恰为两脚trace offset的min；FootOffsetLocation的offset等于本脚trace output、pelvis offset等于同帧spring Result；FootOffsetRotation的normal恰为本脚trace normal。受控normal全部Up，原来的近单位quat长度误差不再进入该消费者。

首query1 Spring/pc54 Target从1.4901162970204496e−8变为5.960464477539063e−8；同帧左脚Location/pc83的FootOffset/PelvisOffset同步变化；Rotation/pc98输入normal从(0,1.0000000758761658,0)变为Up。Source代码确认这些值经过有初始化与历史的spring/damper、clamp和足旋转计算。其余单位域、初始globals、quat/IK算法都保留原target，不因trace控制而补偿。

完整319帧影响：15440个寄存器行变化；最终F32投影有1163个translation骨行、823个rotation骨行变化，scale不变。对462同输入source Rig输出，原T/Q/S差1324/1845/0变为1273/1847/0；位置51恢复无退化，旋转8恢复、10失去原匹配，余1837仍不同。这是独立因素穿过仍有残差的其它算子的实际结果，不能把所有变化叫作恢复，更不能据此称完整Rig/Root通过。source primitive正确也不消除其它阶段原有相消。

必要性消融：把第二域控制开关从true物理删除为false后，两域完整working/call/pose全部回到原域逐值相同；消费者检查必须拒绝缺失控制语义。源码bytes恢复、同六库target重编后，完整两个319域报告与基线逐字节相同。未新增通用适配层、生产状态或运行时开关。

复验从项目根目录：`run_replay.ps1 -Label <new> -Build`重新编译运行完整两域；`run_python.ps1 -Name compare`生成完整source residual/consumer表，`-Name consumer_check`核直接连线与已存消融恢复，`-Name finalize`核封存。工具均位于本目录，使用有自测的固定wall/no-progress watchdog；facts/identity需要本机配置和ALS_UE_SOURCE_ROOT。以replay.no-trace-control.rs替换replay.rs重编可复验第二域回原域，最后必须以replay.rs.final bytes恢复并同target重编。

长期原始输入来源、完整输出、脚本、source facts、消融/恢复和身份清单全在Git管理目录；临时binary/普通日志不是唯一证据。其余395+462 reader和六库不变；生产、reader、转换器、引擎、原始资产均未改。无新D，累计272（116P2/156P3）未修；截图0，性能暂停。

下一沿实际source共同输入的pelvis/foot spring、clamp和rotation历史拆剩余算法/数据边界，再继续完整Rig/Root。该单因素只扩展D247实际消费者，不用其它unit残差复开已完成trace资格。
