# BF144.465：实际骨盆弹簧 ABI、历史与单位域

fresh 本地 CR_Als 的 SpringInterpV2 位于 pc54。实际 12 个参数来自已提取的 Work/Literal 与两个 slice0；根据 source pin 方向、Copy/Zero 写集和实际 visited，319 帧共3828参数值均无调用后 alias 写，才允许使用帧末读数。FFloatSpringState 的三个非反射成员来自前置465a的真实原生成员读取，未将空fields推作零值。

从实际初始化值分别运行三个独立的连续319历史：原米制输入、464的trace控制米制输入、实际source厘米输入。每帧只运输前8输入，Result/Velocity/SimulatedResult/FFloatSpringState均由公开unit连续计算；source after值仅用于核对。原与trace控制两个完整12参数历史各319精确；厘米域的全部公开输出、PreviousTarget/Velocity/valid全部319精确原生读数。fresh UE SpringInterpV2→Kismet GenericSpringInterp→FMath critical SpringDamper 与目标公开unit链已保存，实际配置是2Hz、临界阻尼1、force/target velocity amount0、use current false、initialize from target true，初始化delta0。

source前8输入按长度维度转为米后，全部319与trace控制域精确；仍有158 Result、272 Velocity与272内部状态不同。原域Result差318。首个trace控制结果差在query19：源−0.00689089298248291cm投影−0.00006890892836963758m，米域−0.00006890890654176474m。厘米域逐帧状态与原生完全一致，米域逐帧状态与实际目标完全一致，差异归既有D165单位域float算术；不能归作新算法缺陷，也不扩展到尚未核对的其它spring分支或完整Rig/Root。

必要性消融物理删除持续state，在query19完整公开结果合同失败；保留机器退出101和无本地路径的原始断言。按源码bytes恢复并在同一target、同六库重新编译后，完整三个域报告与基线逐字节相同。删除重复first-comparison摘要，完整差表由check.py直接重建，不保留额外抽象或运行时开关。

复验从项目根执行本目录`run_python.ps1 -Name prepare`，再`run_replay.ps1 -Label <new> -Build`；`run_python.ps1 -Name check`核已封存基线/恢复、共同输入及完整差表，`-Name finalize`核所有Gate。以replay.no-state-history.rs替换replay.rs并重新编译可复验history失败，最后必须以replay.rs.final按字节恢复、同target重编。facts/identity使用本机配置和ALS_UE_SOURCE_ROOT；rig原生完整读数以无损gzip保存，解压长度与SHA均校验。

reader沿465a迁移身份，其余395身份与六库均保持。所有长期源事实、原生读数、请求来源、完整结果、脚本、消融/恢复与身份记录均在Git管理目录；binary与含本地路径的普通日志只作临时产物。自动运行沿用已有自测的固定wall/no-progress watchdog。生产、reader、转换器、引擎与原始资产未改；无新差异，累计272，截图0、性能验收暂停。

下一条沿已证直接消费者核实际FootOffsetLocation的spring/clamp共同输入与历史，然后继续rotation和完整Rig剩余边界。
