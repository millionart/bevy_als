# BF144-D05 站姿地面动力曲线

fresh `MS_Als_Normal` 原生读取列出三种 rotation × 两种 stance 的引用：所有 Standing 使用 Normal 三轴加减速/摩擦曲线，所有 Crouching 使用 Responsive；Standing/VelocityDirection 的 Responsive 引用只用于旋转插值。fresh 两条 CurveVector 的首键分别为 Normal `(2000, 1500, 5)` 和 Responsive `(2500, 2000, 10)`，前两项源单位为 cm/s²。源移动组件按 rotation、stance 取得 gait settings，并将曲线三轴分别用作最大加速度、制动减速度和地面摩擦。原生文件、请求、源码及哈希在本目录。

目标公共 profile 选择现只按 stance 选择这两条加减速曲线；旋转插值设置保持既有独立配置。停止预测、落地剩余运动和实际 motor 均通过同一个选择入口消费。已更正两个将旧映射当作 oracle 的测试，并写六组合及加载角色真实 motor 的首步速度控制；测试尚未运行。

Sprint 入场/截止序号、实际物理、消融、完整图审计与公共 Playground Gate 待全部静态修复和 Bevy/依赖升级后执行。旧 `target/bf144/03/profile/` 原始报告缺失，未声称运行时结果已恢复。
