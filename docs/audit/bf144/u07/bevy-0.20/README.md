# Bevy 0.20.1 迁移

`dependencies.json` 保存远端分支、锁定版本和验证结果。动画图与 GameplayTag 的旧版本依赖 Bevy 0.19。动画图 fork 更新反射注册、形状序列化和编辑器接口，GameplayTag fork 更新 Bevy 与检查器依赖。两者分别通过工作区全目标检查和库、测试及示例的全目标检查。

Avian 0.8 将活跃求解体移入 `SolverBodies`，保留 ECS 上的 `SolverBodyIndex`。项目依原调度阶段读写求解资源并保持角色移动和关节算法。原贴花缓冲记录仍按矩阵、颜色、材质三个字段上传，使用 Bevy 0.20 的存储缓冲接口。自定义渲染 shader 使用引擎 WESL 模块和管线常量。

Tnua 基线为 v0.33.2（`83c4785856aa11f7ac1f4e0a9b7fc6ffec0d1319`）。在该提交和 fork 修复提交分别将 `tnua_toggle_regression.rs` 放到 `avian3d/src/motor_regression.rs`，并在 `avian3d/src/lib.rs` 注册 `#[cfg(test)] mod motor_regression;`。运行 `cargo test -p bevy-tnua-avian3d --lib disabled_and_sense_only_motors_leave_later_motors_running --jobs 2`。基线输出仅第一个启用马达获得速度，修复输出三个启用马达均获得速度，禁用和仅感知马达保持零速度。两轮原始结果同目录保存。

官方稀疏索引和动画图远端历史传输分别达到无进展上限。依赖解析使用镜像索引，动画图对象由已与公开分支 SHA 核对的 Git 副本补齐。项目锁文件记录公开来源与提交。
