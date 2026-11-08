# BF144-D35b2 步行子步更换到禁止站立的 Pawn

fresh ALS/UE 源显示 Walking 更新地面并设置新基座时会触发 Pawn BaseChange；禁止站立的 Pawn 令 JumpOff 改为 Falling，剩余时间按后续模式处理。fresh 原生角色根胶囊为 ECB_No、JumpOffJumpZFactor 为 0；请求、源摘录、原生读取与哈希保存在本目录，本次未在源端驱动角色叠落。

目标步行单步、多子步及落地后接续 Walking 的路径现识别新 Pawn 基座并应用 JumpOff；单步末保留已走位移，多子步将未消费时间交给 Falling，帧末同步保留已处理的 Air 模式以免重复 JumpOff。已写加载角色、允许 Pawn、普通物体及帧末保持对照；测试、消融与真实入口 Gate 待统一验证阶段执行。

旧清单所引 `target/bf144/11/jump_and_permissions/` 已不存在，其原始报告缺失。现有反例为静止基座，运动中 Pawn 的继承速度和自定义重力尚无本项运行证据。
