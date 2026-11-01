# BF144.445 叶播放器当前审计封存

复验：项目根运行 `pwsh docs/audit/bf144/445-final/run_python.ps1`。复用既有watchdog及最终聚合模式，逐子项核验Git跟踪、长度/SHA与精确Gate；缺子项、错范围PASS、坏哈希三个负例必须被拒。164份实际manifest文件均在Git，manifest/summary也要求跟踪。

`.445a`提交1806bb4e提供可选原生tick/read-buffer/normalized与秒字段，旧完整240包兼容；`.445b`（目录445）提交24da0391提供85声明映射及目标完整2805 key/1315 Clip子节点key、319/240 Idle独立时钟核与消融恢复。当前真实Temporary0，仅focused fixture覆盖该分支。新增差异0，D201扩当前consumer，累计271未修；生产实现/原资产未改。

两端各自产生的输入及初始化历史不同，621份paired秒差、source1560/target1490 tick和49目标未实例化声明均保留，而非作无差异结论。下一直接边界是同初态/实际pre-ready输入及状态历史资格，完整Root/非零view/custom attributes继续开。此PASS仅表示当前审计表和证据封存完成。截图0、性能暂停、GPU周边验收未通过且不在本轮验收范围。
