# BF144.466：脚位置标量 Target、弹簧与历史

fresh CR_Als 中两个 ApplyFootOffsetLocation 位于pc83/113。首帧q0不执行，q1–318两脚合计636次；17参数共10812个pin均按源读写方向、真实visited及整个register alias核验，无调用后覆盖。整数cached index来自原始整数导出，FAlsSpringFloatState三个UPROPERTY来自真实numeric/boolean字段。

源pelvis pc66以权重1写global translation；其后实际IK/SetRotation不写root/pelvis，读取类只读层级。层级SetTransform可能在0.0001cm范围内跳过写入，故Work60只给出高度区间。按source float顺序对区间两端计算高度上限、min足offset、max骨盆offset，636次全部收敛到同一个Target，并精确真实PreviousTarget或首次Offset；没有冒充精确源高度。44次上限可能活动，最小上限余量−22.032164573669434cm；第一版“全inactive”假设在q106被拒绝。临界构造区间不收敛，资格检查拒绝这种未知高度，不能放宽精度。

四组独立连续历史：原/trace控制两域调用冻结公开FootOffsetLocation，完整17参数和最终当前FootLocation共1272次精确。夹具仅为该unit提供实际调用前所读的pelvis/thigh两个global，逐值核对；其它骨的local/global一致性不作为场景或完整Rig证据。两脚的初始化、offset、spring state分别持续，不注入观察到的after-state。另两域以独立Target运行同一公开spring kernel与源初始化包装：厘米域636次initialized/offset/Velocity/PreviousTarget/valid全部精确native；投影后的共同米域隔离单位算术。

source为12Hz、阻尼2、target velocity amount0；首次调用只初始化，第二次建立valid，随后持续更新。原域offset差635、trace控制差367；共同米域offset仍差290、velocity/state差565，确认既有D165的脚弹簧因素。trace控制的239次Target差来自465骨盆D165：trace offset输入636均精确源投影，骨盆输入直接接465受控输出；保持高度上限表达式及全部其它实际输入，只换源骨盆投影，全部636 Target恢复。错误简化为max(foot,pelvis)在q106失败，保留真实先min上限再max的表达式。不同因素不按差异次数直接相加。

必要性消融：把首次初始化与valid建立合并，源首有效调用的valid合同失败；清空持续spring state，原公开unit在q19/pc83失败；去骨盆下限失败；注入源调用后external11写入，生命周期资格在q1/pc83拒绝。Rust/Python源码按保存bytes恢复。初次恢复重放与输入再生成有调度重叠；只接纳输入再生成和所有先前进程终态后重新构建的restored-sequential作为最终Gate，完整四域报告与baseline逐字节相同，六库不变。

复验从项目根：本目录run_python.ps1 -Name prepare生成当前输入与资格，run_replay.ps1 -Label <new> -Build运行四域，run_python.ps1 -Name check核完整差表与465直接链接，-Name finalize核封存。run_python.ps1 -Name prepare -Case bad-lifetime复验覆盖写拒绝；用保存的no-pelvis-floor或两个Rust消融文件替换入口可重现失败，最后按*.final字节恢复、顺序重建输入再同target重编。facts/identity需要本机配置与ALS_UE_SOURCE_ROOT；所有自动步骤复用已有自测的固定wall/no-progress watchdog。

长期原始读数、事实、脚本、差表、失败/恢复、身份与压缩哈希均存Git；大静态记录无损gzip，解压长度/SHA校验。生产、reader、转换器、引擎及原资产未改；396含465a reader和六库保持。无新D，累计272；截图0、性能验收暂停。

仍未核对最终source腿向量clamp和完整FootLocation，下一先证明所读thigh translation的字段生命周期并补齐实际原生层级读数；再继续rotation及全Rig/Root反馈。
