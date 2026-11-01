# BF144.459：真实 ControlRig 公开VM全状态重放

从456实际319 query取RigSource Pose、11 binding、405 registers、调用前骨、unit执行后参数及636个trace。重放只使用冻结public RigProgram/RigUnitContext/RigHierarchy，初始化definition和初次Construction一次，逐帧写入实际输入后连续保留working状态；没有向重放注入after寄存器或输出Pose。

全部319 query/129,195个寄存器值/25,201个局部骨输出、13,378次调用顺序和参数/调用前全局骨、636条trace的请求/命中逐值精确实际缓存。trace回调明确重放已采实际命中，不冒physics自产。该入口给后续源端共同输入差分提供完整目标校准，尚未证明源Rig算法等价。

最早失败在query0/pc54 Spring：用了帧后暴露的global delta1/30，使valid提前变true。fresh post_physics_animation:190–196确认初始化期间临时覆盖delta0，结束后恢复；按before实际gate与arena寿命推导首次0后全重放通过。保留初失败参数与错误时钟记录，不以after工作值解围。

三个必要性控制均失败：去首次delta覆盖在pc54失败；破坏首trace起点在query1拒绝；跳过Construction在query0/pc4调用前骨拒绝。source和输入按字节恢复、同target六库重编后完整重放原字节精确恢复。

fresh CR_Als原生静态记录由既有只读compiler重算；程序、local、拓扑、曲线和几何均与target相同，但71骨initial_global共365个double不符，精确复现已有D242边界。没有覆盖这些常量使目标重放假匹配。资产与真实runtime另17处JSON十进制文本差仅属F32最短格式，按源类型IEEE F32 bits全部相同；f64常量不按该规则缩精度。原失败、完整diff与fresh定义保留。

复验：run_python.ps1 -Name prepare，然后run_replay.ps1 -Label <new> -Build。controls.ps1依次运行三个控制、finally恢复源/输入并同target重编；facts/identity要求ALS_UE_SOURCE_ROOT。运行时输入JSON保留可直接消费，三个大报告无损gzip并验证原SHA。source_contract.py与facts.py支持压缩原生记录，finalize.py可在无构建缓存证据的情况下重验结果。

长期证据都在Git管理目录；binary/普通日志可重建且不是唯一证据。未启动新角色场景、未改生产/reader/引擎/原资产或converter；无新D、272项未修。下一源端共同Rig输入和真实world geometry资格，截图0、性能暂停，完整Rig/Root/ALS目标继续。
