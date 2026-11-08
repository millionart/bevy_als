# BF144-D44 翻滚输入按输入时运动模式判定

fresh ALS 源显示 Roll 输入回调立即调用 StartRollingGrounded，该函数当场检查 Grounded；角色设置 bInterruptRollingWhenInAir=False。当前本地源码的输入回调位于 `AlsCharacterExample.cpp:231`，与旧清单行号不同。新原生读取与 D43 设置数据逐字节一致，故复用 `docs/audit/bf144/d43/settings.native.json`，本次请求、reader 报告及哈希仍保存在本目录。本次未在源端重跑同帧地空切换场景。

D43 已将输入触发的实际启动前移，且记录已处理输入，避免 PostUpdate 根据运动后的模式重新启动；本项新增输入时 Air→Grounded 与 Grounded→Air 的系统级及加载角色对照。测试、消融和动画公共视觉 Gate 待统一验证阶段执行，同帧根运动位移不在本项内。

旧清单所引 `target/bf144/14/lifecycle_core/` 已不存在，其原始报告缺失。
