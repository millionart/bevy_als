# BF144-D55 Mantle 期间保留胶囊查询

fresh 源角色 CollisionCylinder 为 Pawn、QueryAndPhysics、非模拟物理且平移/旋转锁定；ALS Custom 运动用 `MoveUpdatedComponent(..., false)`，Mantle 开始与结束不关闭胶囊。角色原生结果、回调图、请求、reader 身份和源码片段保存在本目录。Avian 的锁定轴作用于求解器逆质量，根运动已有自定义位置与速度积分。

目标不再由 Mantle 关闭 CollisionLayers，物理帧临时锁定求解器轴并恢复原值；已有动作帧标记继续阻止同帧旋转响应。已写完整角色在物理阶段实际射线查询、原层与轴恢复，并保留已有无 sweep 轨迹及速度测试；测试、消融和动画 Gate 待统一验证。动态物体受到的碰撞冲量未在本项复测。旧清单所引 `target/bf144/17/initial/` 已不存在，其原始报告缺失。
