# BF144-D52 翻滚落地的 Walking 交接

fresh UE `ProcessLanded` 在剩余时间继续运动前切换 Walking；Walking 入口先 FindFloor、AdjustFloorHeight、SetBaseFromFloor。PhysWalking 在动画根运动下跳过普通速度计算，按根速度推进。源端角色原生读数中，50 ms/0.5 cm、100 ms/0.5 cm、100 ms/2 cm 三组落地后间距均为 2.15 cm；50 ms/2 cm 未接触并保持 Falling。原始请求、输出、reader 身份与源码片段均在本目录。原生探针未播放 Roll，根运动期间的交接由源码确定。

目标的翻滚完整帧 sweep 现使用已有 landing 校验、floor 高度调整和 Walking 碰撞移动；子步内保留已落地状态，余步沿根速度移动并由现有物理后同步发布 locomotion/base。已写实际加载 Roll、非 Roll、未落地及单/多子步对照；测试、消融、动画 Gate 待统一验证。动态基座、落地当帧再次走出边缘、Pawn 不可站立和中断动作组合尚未复测，不能从本项静态结果推断通过。

旧清单所引 `target/bf144/15/landing_walking/` 已不存在，其原始报告缺失。
