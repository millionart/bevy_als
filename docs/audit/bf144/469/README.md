# BF144.469：足旋转精确输入、hidden state与法线历史

fresh CR_Als 的FootOffsetRotation位于pc98/128。14个实际参数/hidden pin按source方向与真实visited整register写集核验无后续alias写；External8/10 normal直接读原生FVector字段。两脚current calf在后续层级写入中保持，initial quat没有后续initial写。foot则在pc100/130之后改写，曾明确拒绝frame-end输入资格。

前置469a以原生函数返回后、下一VM指令前的准确层级补齐foot输入；该source unit只读Hierarchy，因此此处等于其消费姿态。平地两319值虽与末帧相同，四条受控倾斜路径有8个foot quat不同；本项只消费精确边界，不拿相同数值冒充生命周期证据。

原636调用加四路径48调用共684，9576参数值与全部必需骨输入已具。源normal按已取证的float Delta/HalfLife/LN2/InvExpApprox和double FVector Lerp独立保留每脚历史，684个after逐值native精确，其中8个受控动态更新。首调用以输入normal初始化；去该阶段在实际q1失败，清持续历史在受控q6失败。源after-normal仅作为oracle，未回填状态。

原与trace控制两个目标域各636完整14参数/状态/最终rotation精确原冻结调用。每次只运输前8输入，initialized/offset normal/initial relative quat/输出持续保留；current calf/foot通过公开setter按实际调用前global设置并逐值核对，initial global逐值保持原定义。夹具为这两个只读骨提供输入，不作完整Rig层级/真实运行验收。

源normal输入/过滤状态对原目标各636不同；trace控制两者全636精确source，闭合D247 producer到该历史消费者的输入边。initial-relative与最终FootRotation两域各636仍不同，包含已知初始数据和quaternion路径；本项不把它们归因于normal，也不称整个旋转算法或Rig/Root等价。

两次物理消融失败101后，Rust源码按保存bytes恢复，同target六库重编，完整两域及全部normal输出与baseline逐字节相同。读取缺口的旧失败与初版resolved检查遗漏新控制计数的失败均保留；最终684计数和实际/受控拆分检查通过。没有改生产、reader、转换器、引擎或原资产。

从项目根复验本目录run_python.ps1 -Name prepare和run_replay.ps1 -Label <new> -Build；run_python.ps1 -Name compare核完整表，check_input_gap检查已补齐边界。两个保存的消融Rust文件替换入口后重编可重现，最后必须按replay.rs.final bytes恢复并同target重编。facts/identity使用本机配置与ALS_UE_SOURCE_ROOT；所有自动步骤沿用已self-test的固定wall/no-progress watchdog。

所有长期事实、fresh native、输入/输出、脚本、失败/恢复及身份放Git目录；大静态报告无损gzip/raw SHA。396含469a reader与六库保持，无新D，累计272，截图0、性能验收暂停。下一核source初始relative quaternion及calf空间角/限位/最终quat的独立源算子，继续保留完整Rig/Root反馈目标。
