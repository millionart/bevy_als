# BF144.472：真实足旋转数据/算法组合与SetRotation入口

471仅资格同source输入；本项回到原/trace控制真实636调用。actual current输入和连续normal预测来自469，精确source数据与数学来自471，四组数据分别为pre-unit calf/foot pose、initial globals、FootTarget/limits、normal历史，另分current/source数学。原两个域1272个模型端点逐值实际public输出，全部source数据/数学的1272个端点逐值native；完整64组合40704条结果与必要角阶段保留。

原/trace域F32差139/128；只换source数学仍139/128。只换source pre-unit pose，current数学两域均剩1，不能把这个替换当成pose差异根因或生产修复。较早的实际IK/层级pose生产者仍待追；其它单组无F32信号不代表输入无影响，完整矩阵与相消/恢复集合保留。source全部数据及数学同时成立才全native匹配，不将common输入Gate冒称真实等价。

为避免把471限定域的实现简化滥用于新域，先将含显式源Euler/乘法的原型与简化源核在40704个新组合全部阶段比较，差异0、完整报告字节相同，才在本项删除重复reference module。仅本次源证实域，不声称所有可能四元数都等价。

source SetRotation pc100/130的Value与FootRotation pc98/128输出使用同一真实register，Item同一foot key；7参数逐项无调用后alias写，Space=Global、initial=false，636个实际Weight都为1。源Value与完整源unit输出精确；只闭消费入口，不以权重1覆盖部分权重Slerp、source Equals跳过或global→local写回。

当前实际F32合同失败；物理强制不使用source pose组、或将source数学改current数学，source全端点均在q1/pc98失败101。最小源码bytes恢复、同target六库重编后，40704条矩阵完整报告与baseline及实现简化结果逐字节相同。没有改变生产/reader/转换器/引擎/原资产。

复验从项目根：本目录run_python.ps1 -Name prepare生成source/current数据及setter入口；run_replay.ps1 -Label <new> -Build重编矩阵；run_python.ps1 -Name compare核全表，-Case current-f32-contract重现当前失败。保存的no-source-pose/no-source-math文件替换replay.rs可复验，最后按replay.rs.final bytes恢复并同target重编。facts/identity依赖本机配置和ALS_UE_SOURCE_ROOT；所有自动流程复用现成已self-test固定wall/no-progress watchdog。

全部长期事实、fresh native、输入/输出/完整阶段、失败恢复及身份随本项存Git目录；大静态native无损gzip并验解压SHA。396含469a reader和六库保持，累计272、无新增，截图0、性能验收暂停。

下一按source/target SetRotation的权重、Equals早退和层级写回独立核，先保已证source pose界限；再回查造成当前pre-unit pose差异的真实IK生产者和完整Rig/Root反馈。
