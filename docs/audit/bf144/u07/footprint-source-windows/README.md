# 贴花与旋转基座的源姿态窗口

源 Standing Idle 使用 `A_Als_Stand_Pose` 的 ExplicitFrame 0，且该 pin 无输入连接。贴花等待、停止和清理窗口在实际 Standing Idle 稳定后采样。Move 窗口验证 Locomotion 运动，所有阶段继续验证实际状态、持续观察和有效根姿态。

旋转基座左右踏步由根图 `Transition` slot 提供动态姿态。原报告的 Idle 上游边界均静止，左右 slot 分别有约 0.324 rad 与 0.319 rad 的旋转变化。踏步的运动链采用该活动 slot，保留原角色、踏步生命周期、姿态阈值和方向签名。

基线实际报告、按源范围重放摘要和修复后的实际入口报告均保留。原生包及完整运行报告可无损解压，SHA-256 见各目录身份文件。
