# BF144.465a：非反射FFloatSpringState原生只读能力

465 fresh本地KismetMathLibrary.h确认FFloatSpringState的PrevTarget/Velocity/bPrevTargetValid没有UPROPERTY，旧CaptureRigMemory只能导出空fields。旧记录缺状态的合同先失败，不能把空反射补作动态零值。本项是465的读取前置，尚未证明pelvis spring数学等价。

RigNodeHistory请求增加严格bool `capture_float_spring_state`，默认false。只有true时，CaptureRigMemory对实际UScriptStruct类型恰等于FFloatSpringState::StaticStruct的work数组element直接读取原生三个成员，追加独立native_float_spring_state对象。反射fields和原始exported_text保持不变；没有虚构UPROPERTY，也不写source memory。数组外类型和未请求的静态导出保持旧路径。

实际两组连续319历史共638帧、1276个原生struct state。636个有效帧的previous target精确实际Spring Target、velocity精确公开Velocity输出、valid位与0初始化/delta生命周期一致；初帧状态为0/false。去掉仅新增native字段后，完整旧RigNodeHistory probe匹配（实例缓存ContainerVersion按462已证this哈希语义比较，其余完全相同）。

必要性消融：把原生成员getter改成0/false，首有效帧validity合同失败；原输出字段没有变化。去严格类型解析，字符串true的非法请求被接受；源码bytes恢复并完成同package全构建后，最终类型错误返回7。恢复后的完整638份原生状态表与基线逐字节相同，project/package源码SHA一致。

首getter消融的mutation和build曾有启动重叠；在二者终态后，额外核project/package与精确消融源码bytes相同，才接纳对应二进制。最终恢复顺序为源码写回→构建终态→核包→原生读数；不以mtime回滚或默认target二进制代替证据。

整个旧Rig history、default角色和三轴CurveVector完整probe精确，新增字段不扩其它读取协议。Reader从462身份显式迁移，其余395身份/六库保持；生产/转换器/引擎/源资产/源算法均未改。反射source record原样保留，原生读数是新增明确字段。

从项目根目录复验：本目录prepare.py生成两319请求和坏类型；run.ps1 -Label <new>原生读取，run_python.ps1 -Name check -Case <new>核状态/完整旧域。ablate.py zero-state/no-type-guard后build.ps1完成构建，可重现getter失败/坏请求接受；ablate.py restore按reader.final.cpp字节恢复，构建终态后必须重验。default/character-default/curves三个旧入口和compatibility.py验证完整兼容。facts/identity需要本机配置和ALS_UE_SOURCE_ROOT。

所有自动命令复用有自测的wall/no-progress watchdog，机器报告写明最后进展/时限/退出原因。长期源事实、原生报告、请求、脚本、消融/恢复源码bytes和身份清单存Git；大报告无损gzip/raw SHA，普通含本地路径日志仅临时保留，必要错误已去路径持久化，不在target中保留唯一证据。

父465仍待此Gate后消费真实状态，继续source/public SpringInterpV2共同输入与厘米float历史分离。无新D，累计272（116P2/156P3）；截图0，性能暂停。
