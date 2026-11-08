# BF144-D01 动作期间的冲刺旋转优先级

本地 `AAlsCharacter::RefreshRotationMode` 只根据移动组件保留的 `MaxAllowedGait` 判断是否冲刺；空中走独立分支，其他运动模式和站姿均使用同一冲刺优先级。`RefreshGait` 在非 Grounded 时不刷新最大步态。fresh 读取 `CS_Als_Default` 确认空中允许瞄准、冲刺优先于瞄准、冲刺使用速度方向三项均为 True；`B_Als_Character` 对应设置引用见 D243 原生记录。设置资产、读取报告、源码片段和哈希均在本目录。

目标保留原有角色查询范围，只移除冲刺判定中多余的 Grounded/Standing 条件；原本独立的 Air、第一人称和无冲刺分支保持。已补 Custom/None、两种 stance、两种视角、瞄准切换及 Air/非冲刺控制；加载角色的受控 High Mantle 状态测试还检查旋转模式进入实际动画输入。该测试直接创建有效 mantle 状态，不冒充真实碰撞探测启动。

所有测试、消融、图审计及真实 Playground 的 sprint/locomotion 场景均待全部静态修复与 Bevy/依赖升级后执行。旧 `target/bf144/02` 运行日志缺失，未声称真实入口已通过。
