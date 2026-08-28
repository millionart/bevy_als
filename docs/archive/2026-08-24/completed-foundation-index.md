# 已完成基座索引

> 完整 M0–M11 条目、实现细节与 Gate 证据见 [CHECKLIST-M0-M11.md](CHECKLIST-M0-M11.md)。

- [x] M0 环境与资产管线（Gate：skeleton_boot 骨架+动画加载播放 OK）
- [x] M1 ECS 核心状态模型（Gate：state_switch 九步序列合法转移全符合预期）
- [x] M2 曲线与数据驱动基础设施（Gate：curve_gate 曲线驱动数值逐点吻合）
- [x] M3 tnua 运动基座（Gate：motion_gate 三档速度精确达标无回归）
- [x] M4 动画图基座（Gate：速度自动混合+热重载+timescale 帧率无关）
- [x] M5 Locomotion 状态机动画（Gate：locomotion_gate 七阶段全程无脚滑跳变）
- [x] M6 Overlay 与 Aim/View 系统（Gate：overlay_gate 叠加解耦+aim 四象限平滑）
- [x] M7 Foot IK 与 ControlRig 等价物（Gate：foot_ik_gate 三 VERDICT 全 PASS）
- [x] M8 特殊动作（Gate：actions_gate 滚/跳/停步综合 PASS + root motion 链路验收）
- [x] M9 Ragdoll（Gate：ragdoll_gate 自然倒地恢复 PASS）
- [x] M10 相机（Gate：camera_gate 不穿模/aim 切换平滑/数据联动 PASS）
- [x] M11 调试工具与总验收（Gate：workspace 测试全绿 + parity-report 落地）
