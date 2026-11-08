# BF144-D50 Roll 根运动结束后恢复普通转向

fresh UE 源显示 Montage 进入自然混出时清除根运动实例，ALS 在动作为空且无根运动后恢复普通 Grounded 旋转；fresh 原生 AB_Als 为 MontagesOnly，Roll Montage 序列长 1.5 s、混出 0.3 s。请求、源码、原生读取和哈希保存在本目录；Montage 与 D43 逐字节一致并共享其 Git 文件。

目标普通转向只在 Roll 根运动实际活跃时受阻，不再因混出尾段 `roll.active` 继续阻塞。已写根运动期、混出期、完全结束和自然加载角色瞄准对照；测试、消融和动画公共视觉 Gate 待统一验证阶段执行。

本次未在源端重跑自然混出转向场景；旧清单所引 `target/bf144/15/blend_tail/` 已不存在，其原始报告缺失。
