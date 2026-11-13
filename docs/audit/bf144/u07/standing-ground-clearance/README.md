# BF144-D25 站起贴地重试

本地 UE `CharacterMovementComponent.cpp` 的 `UnCrouch` 按重力朝向对充气站姿胶囊做两次阻挡重叠检查。第一次在保持胶囊底部的中心检查，受阻时允许在有效地面上降到 `MinFloorDist`。站起后保留角色实际旋转。

同一 181cm 顶板几何的原生短扫显示，初始站姿顶部穿入约 1.152cm，贴地重试在顶板和地板两侧都无命中，故意穿地约 0.006cm 的对照仍命中。`geometry.request.json` 可用 `tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/SKM_Als.SKM_Als -GeometryQueryProbe` 复跑。目标原始读数显示重试时 Avian 仍返回地板候选，胶囊先做 contact 返回负穿入深度。两次站起查询均在箱体候选上确认正穿入，保留现有的碰撞通道与重力朝向。

正式 `crouching` 功能验证通过。`crouching_actions` 保留了通过的蹲起和左右 180° 路径，另有三组 Quick Stop 与右动态的外层姿态静态报错，左动态未完成。后一份报告仍用于 U07 追查。
