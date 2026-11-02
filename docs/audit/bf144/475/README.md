# BF144.475：local double 单位阶段归入 D232

474 的 source-all SIMD 在厘米求值后投到米，与先换米再执行相同 double 算法，有 693 个 double /691 个 F32 位置变化。本项先回读既有 453：early-unit 在加性运算前换米，变量仍为 F64，结果才投 F32；SC 已保存 36 个位置变化。因此 D232 已包含独立单位阶段，原来的早 F32 事实与单位阶段一直分开。本项明确该标题，追加足部层级消费者，不把它归 D165 的 float 状态或 D275 的 float 导出系数，不增编号。

为独立于 474 的手写 source kernel，直接调用 public RigTransform.relative：同一 source foot/parent 物理输入分别直接以 cm 与 m 求值，Q/S/算法及 F64 保持，二者结果逐值匹配已资格模型。696 共同输入的 current 方法有 694 double /692 F32 位置差，峰 2.7755575615628914e−16 m；source 已资格算法有 693/691，峰 1.6653345369377348e−16 m。极小分量的 F32 不同是数值合同，不构成已证可见故障。

1272 个实际 current local 逐值复现；单位阶段独立改变原/trace 域 635/636 个位置投影，但两域各 0 个完整向量恢复、0 个原匹配退化，仍各 636 与 native 不同。其它已证算法和上游 pose 余差保持，不把 unit-only 控制称为真实 pose 或 IK 修复。两个 clean source 样本按真实 dirty flag 保留输入，所有 source 厘米端点仍精确 native。

物理把厘米 public 路径合并到米路径，首例独立 public cm 合同失败 101；源码按 replay.rs.final 字节恢复、同 target 六库重编，完整报告 byte 恢复。analysis 的 no-unit-control 只是拒绝删除已有信号的诊断断言，另行保留，不冒作物理消融。复验从项目根用 run_python.ps1 -Name prepare、run_replay.ps1 -Label <new> -Build、run_python.ps1 -Name analyze / classify。facts/identity 使用本地配置与 ALS_UE_SOURCE_ROOT；所有自动流程沿用已 self-test 的固定 wall/no-progress watchdog。

长期请求/原生、输入、public 输出、事实、分类、失败恢复与 manifest 在 Git 目录；大文件 lossless gzip 并核解压原 SHA。396 含 474a reader 与六库保持；生产/reader/引擎/转换器/原资产不改。截图 0，性能暂停，累计 272。归属问题关闭，下一回查更早 actual IK pose producer 与 descendant 完整写回，完整 Rig/Root 未验完。
