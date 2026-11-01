# BF144.446e 目标prior曲线真实输入与App帧

从原.446观察器追加真实FrameCount、Virtual elapsed、provider global PoseStanding/PoseGrounded、source property覆盖和AlsRootAnimationContext投影值。before在全部Inputs完成/Evaluate前；after明确在RootAnimationContextSet发布后，避免将未排序的Final后采样当作已发布。未修改projection或其他生产代码，原Playground/角色/相机/驱动保持。

实际初始化在App frame5，global Standing/Grounded为0且无prior context；初始化后投影二者为1。首普通求值在App frame6，global/projection均1，source property无覆盖。319连续App帧的采样合同与四负fixture通过；旧.446五完整数组扣新字段后精确。

源.d已有相同prior值1，但初始化和首world tick均engine100，native回调已在100执行，所以首world不再RefreshPose、StandingAmount仍0。下一engine101才刷新。候选当前差异是不同native callback帧边界，不是map丢键或目标prior值丢失；不能改projection来补偿，也暂不编号。后继需新engine-frame的source控制，才能判断匹配帧条件下资格差是否仍在。target初次查询global time约0.166666675s亦保留，尚未声称world-time输入全相同。

消融移除新增观察字段使合同失败；两源码SHA恢复、同target六库重编后，完整新旧runtime数组及独立分析精确恢复，baseline/restored公共animation functional-only通过。所有长期原始读数、脚本、阶段/消融/恢复、身份与manifest在Git。截图0、性能暂停，GPU周边不在验收范围。

复验：`pwsh docs/audit/bf144/446e/run_python.ps1 -Name check -Case restored` 与 `-Name analyze -Case restored`；重跑真实场景使用新Label，不覆盖封存目录。整体Root/完整内部历史未等价。
