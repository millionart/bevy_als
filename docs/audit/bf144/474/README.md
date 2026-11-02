# BF144.474：足部 global→local、原生 dirty 与数值阶段

fresh CR 与本地 UE C++ 事实确认，实际 FTransform 使用 TransformVectorized.cpp 的 GetRelativeTransform，位置通过 NEON cross、multiply-add，local 末尾 NormalizeRotation，再由 PoseAdapter CopyBonesFrom 输出。最初使用非向量实现、强制父 scale=1、厘米↔米额外往返的三个诊断候选分别被源/public 精确合同拒绝，失败源码与日志保留，不作通过证明。

636 实际和 60 权重控制共有 696 足部样本；474a 已提交原生 setter 之后、global getter 之前的真实 local dirty。694 true 使用精确 parent/global 重算，2 false 使用本帧 before local；完整 T/Q/S 全 696 native 精确。分支读取 native flag，不以输出、case 名称或 requested 权重推断。clean 路径还用 public Hierarchy 的当帧输入/local getter 验证，无 global 往返。

两条 319 连续实际 VM 的 1272 local 用当前 public relative 全部逐值重现。四个 foot/parent source-current 数据组合、16 个 product/normalizer/SIMD vector/单位阶段组合，共 81408 记录；完整 source 数据/算法恢复 1272 native 端点。11136 共同输入记录保留 raw primitive 与最终 cache 结果。当前两域 local 均 636 F32 骨行不同，不能把共同数据恢复称为实际 IK/Root 修复；源数据下 current 算法仍有 694 条 F32 local 差，其中旋转只有 1，位置差多为近零分量。

组合顺序与 NEON normalizer 为既有 D163/D248 消费者，SIMD vector 与 scalar 候选的区别保持独立。显式 source product 参考在全部新 actual/mixed 数据域精确，加入它的完整报告 byte 不变，故最终删除重复参考 module。物理移除 clean cache、vector FMA 或源厘米阶段均失败，恢复 final 源码字节并同 target/六库重编后，完整 81408/11136 报告与基线和 reference 校准逐字节相同。

单位阶段保持 double，无 float cast 或导出系数降精度：其它源 primitive 固定，厘米运算后换米与先换米再运算仍有 693 double /691 F32 位置差。首 q1/foot_l 的极小 Y/Z 分量有变化，完整 witness 在 unit-phase-boundary.json。该边界已确认且保留，须与既有数值空间记录核对归属，未先归 D165 的 float 量化或 D275 的导出系数，也未新增编号。

从项目根通过 run_python.ps1 -Name prepare 重建输入，run_replay.ps1 -Label <new> -Build 重编完整矩阵，compare.py 校验基线并通过 -Case current-f32-contract 重现当前失败。replay.no-clean-cache.rs、replay.no-vector-fma.rs、replay.no-source-unit-phase.rs 为物理消融；最后按 replay.rs.final 字节恢复并同 target 重编。初版无效候选另存，所有自动流程复用已 self-test 的固定 wall/no-progress watchdog。

长期原生/输入/全矩阵、失败恢复、source facts、身份和 manifest 全部在 Git 目录；大报告无损 gzip 并核原 SHA。396 含新 474a reader 与六库保持，生产/引擎/转换器/原始资产不改，截图 0，性能暂停。全 descendant、此前真实 IK pose 生产、完整 Rig/Root 及单位阶段差异归属仍待后续闭合。
