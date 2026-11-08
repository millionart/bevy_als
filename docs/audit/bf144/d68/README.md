# BF144-D68 Ragdoll→GetUp 的姿态意图

fresh ALS 退出时先切地空模式，再清空动作并调用 ApplyDesiredStance，之后才可能进入 GettingUp；UE Crouch/UnCrouch 保存 CMC 意图，在后续移动阶段提交。fresh 角色设置与 D43 原生输出逐字节相同，复用该 Git 文件；本目录保留新请求、reader 报告和源码片段。

目标在现有蹲姿事务中保存这次动作清空时的姿态意图，即使 GettingUp 先于胶囊重塑也会消费；净空阻挡时保留重试，动作结束后重新采用最新期望姿态。已写加载角色四种实际/期望姿态对照和受阻重试对照；测试、消融和动画 Gate 待统一验证。自身 rig 在净空查询中作为障碍的问题留 D69；旧 `target/bf144/20/stance_order/` 原始报告缺失。
