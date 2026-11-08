# BF144-D67 Jump 触发的同帧 Ragdoll 退出

fresh ALSExtras 的 Jump 与 Ragdoll 输入都同步调用 `StopRagdolling`，成功且落地时在调用内切 Walking、播放 GetUp 并阻塞输入。角色 native 输出、请求、reader 身份及源码片段保存在本目录。

目标把退出路由链排在 Jump 请求生产之后、状态提交之前，保留 Ragdoll 键与直接请求的先后约束。已写调度依赖图可达性测试，以及加载角色的直接请求/Ragdoll 键/Jump 键同帧起身与输入阻塞对照；测试、消融和动画 Gate 待统一验证。旧清单所引 `target/bf144/20/jump_order/` 已不存在，其原始报告缺失。
