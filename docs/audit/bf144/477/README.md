# BF144.477：实际自动骨长与完整求解消费

承接476已证明时点、完整参数及三骨before/current/initial输入。fresh同CR_Als IR、wrapper与FVector实现表明：B的初始求解变换继承A旋转/scale、仅换B位置，自动骨长两个scale分子均取current A。1332源调用中2个weight0在骨长前返回，其余1330调用初始scale平方都满足非零分支；不声称覆盖零scale。

现有原生scaled_distance对2660源骨长及5088同current输入的厘米/米长度直接调用UE算子；长度float投影精确。1324满权重调用把独立长度送同native SolveBasicTwoBoneIK，A/B完整旋转及B/C位置全部与实际source post精确；6 partial只验收长度。core后续内部求解算术、partial与最终层级传播继续审计。

1272当前实际调用：Rust从公开hierarchy初始globals和current A scale计算的骨长，逐值等于独立native同米输入；显式传这些长度后完整三骨T/Q/S与实际VM结果保持。固定相同current初始数据，只改为native厘米float骨长再转米，512个长度标量/452次完整姿态改变，452均有F32骨字段变化；double厘米先转米再cast的控制全部保持。归既有D165，不混合source初始数据D242或声明最终source结果已恢复。

物理移除厘米float阶段后，原始1272公共合同仍通过而452因素信号全部消失，必要性断言101失败。源码按final字节恢复、同target六库重编后，完整报告与基线byte一致。只有诊断/文档，生产及reader/引擎/转换器/原资产均未改。

项目根复验：run_python.ps1 -Name facts / prepare / prepare_solver / check_solver / analyze / identity / finalize；native用run_native.ps1 -Label <new> -Request lengths或solver，不覆盖原生source record；run_replay.ps1 -Label <new> -Build重放完整四域。facts/identity需要ALS_UE_SOURCE_ROOT及既有gitignored配置。全部自动流程复用固定wall/no-progress watchdog，自验见441b/watchdog.self-test.json；原始报告压缩后解压SHA与bytes保持，读数/请求/脚本/失败/恢复/身份与清单都在Git目录。

截图0、性能暂停；6 partial/solver内部数学及完整Rig/Root未在本项验收。
