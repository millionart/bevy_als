# BF144.443 当前物理反馈审计封存

复验：在项目根运行 `pwsh docs/audit/bf144/443-final/run_python.ps1`。沿用现有 reader 的已测试 watchdog，进展为逐子项核验，30 秒无进展退出。四个负例拒绝缺子项、失败状态、其他范围的 PASS 与坏哈希。

本批仅合并 `.443a`（目录 `443`）、`.443b`、`.443c` 的独立提交。253 份 manifest 文件逐字节核对长度、SHA 与 Git 跟踪；manifest 和 summary 也要求已跟踪。全部长期原始证据、请求、脚本、消融和恢复结果在这些 Git 目录。构建产物和临时 stdout 不是唯一证据。

240 帧实际目标计划、motor、完整物理和后置 floor 已由真实输入逐值解释；fresh source 240 帧的 world input 与重力 −981 cm/s² 已核。不同 world origin、Y 前态及各自产生的参数没有被当作共同初态。既有 D253/D254/D264 新消费者有直接证据，新增 D273（带内 floor 仍前置抬升）、D274（存储 TQS scale 被仿射分解 getter 替代）。累计 271 项（116 P2、155 P3）；生产未修。

限定默认 grounded、无 parent、单位 scale 的原 Playground cohort。完整 source 世界/Root 反馈、player clock、非零 view、custom attributes 以及任意 parent/nonuniform/shear 仍未闭合。旧 ignored 证据丢失没有被本批文字恢复；只复核当前 396 文件身份链和六个运行库，不声称恢复旧全量身份 corpus。

公共动画 functional-only baseline/restored 已过。未改的公开 startup 同样出现 TAA pass 超时，失败报告保留在 `../443b/startup-control/`；仅诊断复制 main 按用户明确排除周边而跳过 GPU pass 验收，原 renderer、场景、角色、相机及生产配置不变。GPU pass 未通过验收，截图 0，性能验收继续暂停。本批 PASS 表示范围内审计与差异登记完成，不代表实现等价。
