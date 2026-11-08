# BF144-D35a 落到禁止站立的 Pawn 时 JumpOff

fresh UE 源显示 SetBase 后通过 Pawn 根组件权限决定是否 JumpOff，默认算法先按最大速度 0.85 加随机方向、必要时限速，再设置 JumpOffJumpZFactor 对应的竖直速度并切 Falling。fresh 原生角色的 CollisionCylinder 为 ECB_No、JumpOffJumpZFactor 为 0、JumpZVelocity 为 420 cm/s；请求、源摘录、原生读取与哈希保存在本目录。此次原生读取没有运行角色叠落场景。

目标下落落地事务现检查 Pawn 根碰撞体所属实体的权限；禁止时立即应用源速度结构并进入 Falling，允许的 Pawn 和普通物体保留落地。共享函数使用既有角色随机种子及默认重力方向；随机方向精确逐样本对齐和自定义重力不属于当前反例。已写权限、速度结构和落地事务对照，测试、消融及真实入口 Gate 待统一验证阶段运行。

旧清单所引 `target/bf144/11/jump_and_permissions/` 已不存在，其原始报告缺失。出生、步行更换基座和帧末同步的入口由 D35b 处理。
