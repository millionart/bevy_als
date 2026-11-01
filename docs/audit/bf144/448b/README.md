# BF144.448b 实际Root与五Pose缓存

复用 `.447b` 原Playground输入与同六个target_game库，新增319帧before/after只读默认Root与实际updated节点缓存。after每个边界只有一个Default context；默认Root与根缓存逐值精确。未修改生产、输入、状态、图或资产。五组旧完整历史保持；初次before尚无输出，None原样保留。

| 源端读点 | 目标实际缓存 |
|---|---|
| Root Result | main Root `8191c2ea` |
| Rig Source | main Head linked `e4f33e44` |
| Head Base | Head ViewInput `316d5caa` |
| Head Additive | Head InitializeHead输出 `5a7796c6` |
| BaseProvider Input | main PostLocomotion slot `ccaab6a2` |

每条记录保留graph/context/state key、骨名/path、T/Q/S/weights的presence、timestamp/root_motion和同节点curve state。所有79个源骨均存在；默认Root有额外场景节点SKM_Als。它在44帧scale非恒等，首query100/ordinal21为0.9769455790519714，仍需证明实际consumer和根因，不新计D。Head Additive部分scale为None，保留其稀疏语义，不补成0/1。BAG Pose缺少custom-attribute容器仅记类型事实；当前source空性来自 `.448a` 实际五边界，未伪造target runtime count0。

最初checker误强制完整scale通道、假定额外节点始终恒等，已据原始输出改为验证实际optional合同并登记候选，未放宽任何数值容差。缺边界/错node/缺骨/非有限curve四反例拒；删除观察字段后原场景check缺字段失败，其余全部旧历史保持。三源码按原始bytes/SHA恢复，在同target六库重编后完整新旧数组、check报告精确恢复，两次公共functional Gate通过。NodeId显示接口编译失败与一次新label缺文件不算Gate证据。移除fact工具未使用的UE配置依赖后输出bytes不变，该冗余永久删除。

396身份/六库/原main不变；三份参数历史mtime=0无损gzip，解压raw SHA独立验证，原始读数、消融/恢复、脚本与manifest全部Git管理。复验用已有输入label：`pwsh docs/audit/bf144/448b/run_python.ps1 -Name check -Case restored`；新的真实运行才创建新Label，不覆盖封存。完整source-target Root余差归属依 `.448c`，本项只验读取/被动兼容。无新增编号，271未修，截图0/性能暂停；GPU周边未验收。
