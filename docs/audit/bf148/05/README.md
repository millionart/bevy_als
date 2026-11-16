# BF148.05 恒成功的包装

本地 `AnimNode_StateMachine.cpp` 866–895 只有找到规则才填写有效过渡及源规则索引。运行时的既有低层转移描述随后总持有转移 ID。`AlsAnimationInstance.cpp` 1636–1661 与 1780–1797 按姿势和左右选择序列，在实际 Transition slot 发布 Montage。Standing 原生编译读数覆盖 4 个 FSM、22 状态和 40 个资产播放器。见 [native.json](native.json)。

转移 ID、已给定动作的选片、原样的父时间和场景目标 yaw 都有值。移除这些包装，把 `Option` 留在可以没有 playback 或配置的调用，把 `Result` 留在图查询和 GraphIoEnv 方法。选片仍按同一 `index <= 0.5` 判断左右。三处直接转移配置查找复用同一条件，`source_transition_event` 复用已选转移引用，仍先评估原有规则再检查目标状态。父时间回退使用相同快照。

[定向 Clippy 记录](targeted-clippy.json) 对内部恒成功包装从四条到零。用现有 FSM、动作选片和通知测试，加上场景 yaw 的两侧与中间值断言验收。[作者性清单](../../../../tools/animgraph_authoring/bf148_fsm_results.authoring.json) 固定实际 Standing FSM 的 5 个状态和 12 条转移。公共入口验证 startup、state_machine 和 standing_stop。

初次 state_machine 的 Quick Stop 通知覆盖、根与最终姿态通过，最终旋转跨幅 0.3727 rad。它还沿固定 Standing Idle 的姿态链检查运动，造成与 BF148.02 相同的误判。这个动作与 Standing Stop 场景中的 Quick Stop 共用实际 Transition Slot 观察边界。原报告在 [failed-runtime.json](failed-runtime.json)，其余场景与图求值断言保持原有合同。

定向 103 项、观察器复验四项通过。最终同入口 startup、state_machine、standing_stop 功能 Gate 通过。原始报告长度及 SHA-256 见 [runtime.json](runtime.json)。
