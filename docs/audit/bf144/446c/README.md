# BF144.446c 完整318外部输入史与实际producer边界

此项复用已封存`.446` target319和`.446b` source318，无新runtime。初始化单列，source普通frame i对应target query i+1。全部318 delta、world方向、desired stance/gait及实际controller setter消费逐值核对；源setter按fresh `AController::SetControlRotation` 的float 0.001°容差保留旧值，不能要求实际角度逐double位等于每次请求。四个错阶段/轴/时钟/controller负例拒绝。

source内部状态从`numeric_value`读取FVector double与float标量，不把六位exported_text当数值oracle。318组Gait/Stance/Locomotion tags全等；首普通更新pending差是D149。位置300帧、速度94、加速度62、speed91帧不同，首位置差在target query19的pre-ready竖直动态；与D31链路相关，但本数据缺cold motor系数，尚未逐值归因。

85声明完整实际表有source1978/target1879次tick、103资格差和697秒差。Idle的318个公共当前核与直接秒source核逐值通过，313差是D201；移除controller hold分支与直接秒因子分别使实际输入/原生clock最小例失败，源码字节恢复、同target重编后通过。原比较进程元报告在消融时覆盖，明确记缺失；原数值JSON与恢复后独立正进程才作最终证明。

最早未闭资格是Default Overlay的Default_Poses节点：source普通frame0 PoseState.StandingAmount=0且未tick，target已完成该节点；source frame1 StandingAmount=1且tick。source公共曲线在init/frame0已为1，不等于RefreshPose实际读取的proxy `GetAnimationCurves` map。target实际prior-curve global输入尚未直接记录。缺少这两个真实输入，候选不编号、不归D149/D201；下一沿这个直接source边补观察和同条件控制，不复开无关图。

所有脚本、原始输入身份、完整状态/clock表、负例/消融/恢复结果、源事实与manifest在Git；原始native/target报告由本目录显式引用既有Git证据。截图0、性能暂停、GPU周边验收继续排除。此PASS是完整外部输入及明确最早未闭边界的审计完成，不是完整Root等价。

复验：`pwsh docs/audit/bf144/446c/run_python.ps1 -Name compare -Case recheck`，`pwsh docs/audit/bf144/446c/run_replay.ps1 -Label recheck -Build`。使用新过程Label保留封存元报告。
