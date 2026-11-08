# BF144-D252 出生运动模式

本地 UE 的 `PostInitializeComponents` 在允许无控制器物理时设置默认陆地模式；进入 Walking 会查询地面并设置 base，无有效 base 时恢复原竖速并切换 Falling。`birth.request.json` 与原生读取结果记录 engine frame 17 的两种出生：92.15 cm 时首帧前为 Walking，floor 距离 2.1500000953674316 cm；150 cm 时首帧前为 Falling，第一步位置为 149.45499994315207 cm、竖速为 −32.7000017054379 cm/s。`witness.json` 摘录这两个原生控制，`native-identities.json` 保存原文与压缩文件字节数及 SHA-256；压缩时已逐字节核对解压结果。

角色首次具备运动参数、碰撞体和物理状态时，在控制器与首个物理步之前复用地面查询并初始化支撑：有支撑时绑定 base 并清除竖速，无支撑时保留竖速并切换 Air。初始化标记只消费一次，尚未加载参数的角色等待；已有非 Grounded 状态不被覆盖。Playground 玩家准入排在该初始化之前。已有测试代码覆盖支撑、无支撑、传感器、显式 Air、延迟参数、重复执行、同帧新建地面和首个物理步。

Avian 0.7 源码显示刚体添加钩子从 Transform 初始化 Position，碰撞体插入观察器登记空间树，后续移动由 BroadPhase 更新。真实 Playground 准入等待场景物理就绪；同帧新建地面的测试尚未运行，同帧移动已有碰撞体而树尚未更新的边界未验证。旧 `.296/.298/.299` 目标原始报告缺失，不能以文字补作运行证据。所有 focused 测试、消融与恢复、动画图静态审计和公共 Playground Gate 均待全部静态修复及 Bevy/依赖迁移后执行。
