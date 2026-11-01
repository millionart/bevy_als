# BF144.446a 实际初始化到 ready 的输入/状态表

原 Playground 的319查询均记录求值前后实际actor状态、Root gameplay tags、desired状态、有效/实际world input、actor local/world及provider所用外层avatar TQS。四个负fixture拒缺tag、错tag、非有限quat与blocked时仍非零输入。LocomotionState的serde变体与as_str返回值不同，验证器按真实映射核对，不把它误报为生产差异。

ready前79查询全Grounded、actual Walking/desired Running、零有效及world输入，root原始quat不变，raw view恒为−0.7853982448577881。79号原驱动才切raw view至−π/2。世界位置与非零heading均不同于旧原生固定初态；完整正步数为318，源入口仅240帧且不接收actor XY/yaw，因此不能只加79个零输入帧声称同历史。

新源初始化读取进一步确认：UE初次普通world tick前已有Idle tick_delta0/clock0初始化，pending仍True。目标Absolute0本身不是新增差异；把source首普通tick的正delta与target初始化查询配对会误报。pending提前清除的根因已是D149，本项不重编号，也不通过删除零初始化去“修复”阶段配对。

移除history_input字段后合同失败；源码SHA恢复并用同target六库重编，原场景全部阶段数组/独立分析精确恢复。旧.445五数组扣新增字段后精确，baseline/restored公共动画functional-only通过。截图0/性能暂停，GPU pass按用户排除周边未验收。source318完整前缀、正确初态及Root匹配仍需后继只读能力；此Gate只封存实际输入资格，不声称完整等价。

复验用 `pwsh docs/audit/bf144/446/run_python.ps1 -Name check -Case restored` 与 `-Name analyze -Case restored`；真实运行用新Label，不能覆盖封存目录。所有长期证据在本Git目录，source原始初始化报告引用已跟踪的`../445a/restored.native.json.gz`。
