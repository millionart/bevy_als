# BF144-D60 Mantle 物理后的基座携带

fresh UE 仅在模拟基座的 post-physics tick 调用 `UpdateBasedMovement`；其以胶囊脚底为锚点变换位置，忽略基座 scale。native 角色 `bIgnoreBaseRotation=True`，保存绝对朝向。角色原生请求、输出、reader 身份和相关源码保存在本目录。旧 BF116.03c2b Gate 把物理后的基座变换重新代入整段 warp，已有限复开归档 BF115.09e2b2 对应边界。

目标删除物理后二次 warp，模拟基座复用脚底携带，非模拟基座保留下一次物理前的快照；旧移动目标测试改为源时序。已写位置、旋转、scale、父空间、owner 0/1 和亚微秒根运动对照。测试、消融、图转换对比及动画 Gate 待统一验证；旧清单所引 `target/bf144/19/initial/` 与 `clock/` 已不存在，其原始报告缺失。基座物理禁用状态未在本项复测。
