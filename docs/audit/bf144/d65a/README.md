# BF144-D65a 显式 Ragdoll 启动许可

fresh ALS 在任何启动副作用之前要求当前 mesh 的 pelvis 与 spine_03 BodyInstance 均存在；UE 按骨骼名查 PhysicsAsset 索引和当前实例数组。fresh PA_Als 原生读取包含这两个 body 及非必需 foot_l，原始输出、请求、reader 身份和源码闭包保存在本目录。

目标对真实 ALS 角色的显式请求检查当前动画玩家的已生成物理体与刚体组件，缺任一实例即消费并拒绝请求；非 ALS 的通用状态测试保持原接口。已写完整角色两种缺失、完整和非必需 body 缺失对照；旧无物理体 `action_priority_gate` 与输入夹具已改为验证拒绝后模拟既有 Ragdoll 状态，其归档 BF-Char5 许可边界已有限复开。测试、消融和动画 Gate 待统一验证；自动入口留 D65b。旧反例未记录可恢复的原始报告路径。
