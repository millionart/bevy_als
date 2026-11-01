# BF144.461：实际足部 trace ABI、生命周期与法线因子

沿460的两实例连续319帧和459已校准目标输入，fresh读取CR_Als。165条指令及183 literal布局与当前相同。按源pin的Input/Output/IO/Hidden方向、Copy/Zero目标建立写集合，逐帧按实际visited顺序检查全部trace参数后续写入：636次调用/6360参数均没有后续寄存器别名覆写，因此可以使用既有work_after读取这些特定参数。该结论不推广到其他寄存器，也不把任意帧末值当作消费值。

初帧source Forwards Solve的visited快照长90，前15项与同帧Construction逐项相同，余75项对应正常Forward；这与UE按first event重置visit记录的生命周期一致。没有把90冒作75或删除源事件。初帧无足trace，后318帧每帧两个trace各只调用一次。

按实际F32/F64声明、坐标基和单位投影后，8个输入在636次调用中全部匹配。OffsetLocationZ有588次不符，OffsetNormal有636次不符。source物理厘米float与target米float的原始计算域仍不同，类型投影匹配不证明单位阶段等价。source局部TraceStart/End公式及实际MeshToWorld已具，内部世界起终点和FHit尚未捕获；不把独立reference ray当内部查询。

公开RigUnitContext调用实际FootOffsetTrace，使用460已校准的两块原场景地板和原RigQueryContext，636条start/end/命中及完整10个after参数逐值精确当前真实记录；没有回填已采hit。受控post-hit核先完整复现全部636个当前输出，然后只将normal逆旋转换为source cross公式，保留同一实际命中点、point逆变换、厘米/米现有域、float舍入及坡度修正。636个normal全部恢复source实际Up，576个offset改变；原48个source offset匹配均保留，另44个恢复，无反向退化，余544仍不符。仅扩展既有D247的当前真实消费者，不新编号、不称全Rig恢复。

两项物理消融均保留：

- 删除source normal分支，当前636个公开unit/模型结果不变，source normal恢复变0，最小反例退出101。
- 构造同帧trace后的真实Copy writer26覆写Work23。保留alias检查时退出1；物理删除检查后该坏fixture被接受，并在报告中留下非空overwriting_writes。首轮消融标签没有注入坏数据，被finalizer发现并拒绝；修正后两轮都明确记录注入writer26。

Rust/Python源码及完整input/lifetime证明按字节恢复；同六库target重新编译，完整636份输出与typed-source基线逐字节相同。恢复过程中首辅助重编与同值input重写有重叠，不作为最终恢复证据；最终restored-final在输入恢复并校验bytes后独立重编。初版错误单位scale假设和JSON integer/float序列化比较造成的假阴性均保留：实际scale=(0.9999999403953552,1,0.9999999403953552)，按实际scale安全倒数；source结果按RigValue声明类型比较，不对数字加容差。

复验从项目根目录执行：

- `pwsh -NoProfile -File docs/audit/bf144/461/run_rig.ps1`重读源ABI；`run_python.ps1 -Name prepare`可使用压缩ABI，重建source参数和写集合证明。
- `run_replay.ps1 -Label <new> -Build`重编并验证完整实际unit与单因素控制。
- `run_python.ps1 -Name prepare -Case bad-lifetime`必须退出1。用prepare.no-lifetime-guard.py覆盖prepare.py后，同一个bad-lifetime参数必须接受且overwriting_writes非空；最后按prepare.py.final恢复并重建输入。
- 用replay.no-source-normal.rs覆盖replay.rs、同target重编，必须退出101；按replay.rs.final字节恢复并重编，完整报告必须恢复。
- `run_python.ps1 -Name finalize`重验封存结果。facts/identity需要本机配置与ALS_UE_SOURCE_ROOT。所有自动命令复用现有有自测的watchdog，wall/no-progress上限写入各机器报告。

长期证据全部存Git管理目录及summary列出的5份外部证据；大原生报告无损gzip/raw SHA，普通日志位于忽略临时目录，必要错误已去本地路径持久化。移除仅打印schema的临时inspect步骤，其信息已由fresh ABI、完整参数和生命周期证明覆盖。生产、reader、转换器、引擎及原始资产均不改，395+457 reader和六库身份不变。

下一步是源API同参数world ray/查询及厘米float offset的独立资格，之后继续完整连续Rig/Root差分。当前D247已证边界不反推544个offset余差，也不把查询能力缺口当作新差异。累计272（116P2/156P3）未修；截图0，性能暂停。
