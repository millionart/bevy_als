# BF144.447b Head实际参数、历史与Look pin

原.e Playground319查询新增Head全部输入/state的global、source property、source-frame-input三通道与当前resolved值。before在Inputs后/Evaluate前，after在求值/投影后；首次state尚未分配的None保留，不伪装成已读0。before缓存可能属于上一轮，只有after的当帧updated Look两个pin作消费证明。

319前后state与actual Look pin通过四负fixture，全部旧五阶段数组扣新增head_values后精确。首Look时钟差source78/target79，native View.YawAngle45、Head.YawAngle3.5451583862304688/YawAmount0.5098476409912109；target actual view44.999996185302734、Head3.5451622009277344/YawAmount0.5098477005958557。Look yaw/pitch pin精确等于实际Head输出。输入并非逐位相同，不能从最终clock判断source kernel遗漏；下一在共同输入与完整状态史中分离输入/算序。

删除新增观察字段必须失败；源码原始字节/SHA恢复、同target六库重编/原场景复跑后，完整数组及analysis精确恢复，公共animation functional-only通过。未修改任何原输入、状态、图或生产逻辑，未新增差异编号，271仍未修。原Head/Settings资产及CPP/RON证据在`.447`，本目录保留实际target、消融/恢复、身份与manifest。

复验 `pwsh docs/audit/bf144/447b/run_python.ps1 -Name check -Case restored` 与 `-Name analyze -Case restored`。新runtime另用新Label，不能覆盖封存。完整共同输入kernel/Root/非零view/custom attributes仍未闭；截图0、性能暂停、GPU周边继续排除。
