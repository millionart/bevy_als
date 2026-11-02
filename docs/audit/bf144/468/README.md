# BF144.468：完整FootLocation与最终腿向量clamp

fresh本地ApplyFootOffsetLocation→TVector GetClampedToMaxSize/SizeSquared→Mac ARM64/Clang/Generic double InvSqrt源链已提取。源先以float计算LegLength×Stretch，再按double厘米构造leg，Scale为Max×(1/sqrt(SizeSquared))。当前public unit在米制域执行，以Max/sqrt计算Scale。当前glam dot与source都左结合XYZ；切换坐标基后累加顺序也随轴交换。

source完整636次FootLocation独立逐double精确，其中121次clamp激活。输入使用466已资格17 pin、独立连续预测的厘米offset，以及467已证明在消费点可用的thigh translation；不使用其已改变的rotation/scale，也不注入观察到的FootLocation after。两组native hierarchy的所需global数据相同。fresh CR instruction保持当前定义。

四个独立连续历史：原域/trace控制域的公开unit完整17参数共1272次精确实际记录；相同source厘米数值、upZ和真实delta下的公开unit636次scalar/initialized/完整state精确native；共同米域的636完整state精确466独立域。厘米/共同米域的首调用状态均为真实构造零值，不运输after-state。夹具仅为该unit提供实际消费的pelvis/thigh global translation，其余骨的完整pose不作为运行场景证据。

相同厘米数字排除D165单位因素后，当前公开clamp仍有37个double向量差，所有F32投影相同。源式倒数乘法单因素全部636恢复。首q1/pc113 native Z=13.793534620321694cm，当前13.79353462032168cm；dot累加、输入、scalar和长度值均相同。独立当前公式也636精确公开输出，因此归已有D249向量倒数复用的新消费者，不新增编号，不误归D248的NEON quaternion normalizer。

原/trace控制输出对native投影的F32差470/121；相同source物理输入的米域差123。只换厘米float限长结果的物理值，121恢复无退化、余2；offset与max组合122恢复、余1；使用源实际prepoint形成和max/dot/倒数路径后123恢复、F32余0，仍有439个double差。q88/pc113中，单max原已匹配，加offset后反而失去该匹配；完整表保留这种相消，不能把各因素次数相加或称整体修复。这里扩展D165 float长度/位置输入的实际消费者，保留厘米/米制double算术边界，不称整个Rig/Root或所有double等价。

当前共同厘米精确合同在首q1右脚失败；物理删去源倒数乘法后独立native资格同处失败101，删clamp则q1左脚失败101。按Rust源码bytes恢复、同target六库重编后，四组完整报告与baseline字节相同。当前只有诊断夹具和证据变动，生产、reader、转换器、引擎、原资产均未改。

从项目根复验：本目录run_python.ps1 -Name prepare生成有来源绑定的输入；run_replay.ps1 -Label <new> -Build重编四域；run_python.ps1 -Name compare核全差表、算子控制与state，-Case current-contract重现当前精确合同失败。以保存的no-reciprocal/no-clamp文件替换replay.rs可复验两消融，最后必须按replay.rs.final字节恢复再同target重编。facts/identity使用本机配置和ALS_UE_SOURCE_ROOT；自动步骤沿用已有self-test的固定wall/no-progress watchdog。

所有长期原始事实、fresh native、脚本、输入/完整输出/差表、消融/恢复及身份在Git管理目录；大静态报告无损gzip，解压长度/SHA核对。396含467 reader和六库保持；截图0、性能验收暂停。无新D，累计272，已证D249/D165继续未修。

下一先核实际FootOffsetRotation参数/hidden state/当前calf与initial quaternion的字段资格，再续该旋转链及全Rig/Root反馈；保留本项米域double算术未恢复的边界。
