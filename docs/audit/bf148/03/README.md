# BF148.03 碰撞层归属

本地 `AlsCharacter_Actions.cpp` 788–794 进入 ragdoll 时关胶囊碰撞并切网格物理，1081–1089 退出时恢复胶囊碰撞。496–513 另走 Mantle 的 Custom 运动和根运动处理。目标沿现有 Avian 碰撞层归属实现，相关动作阶段通过真实入口检查。

原 `had_original` 只在首个 owner 取得时赋 `current.is_some()`，与 `original` 同时清空，并且只用于最后释放。这个不变量使 `self.original.take()` 成为等价的恢复读写。外层 `None` 表示无需修改，`Some(None)` 表示恢复原本没有的组件，`Some(Some(layers))` 表示写回原层。两个 owner 仍分别决定何时恢复。横向检查碰撞层保存、原值回退和存在标记，其他相似字段各有独立消费者或时间条件。

此前代码的两项 owner 单测见 BF148.01 的串行记录，BF148.02 保留实际 Get Up 基线。移除字段后用同一套 owner 测试、Mantle 持有时的 floor query 与 ragdoll profile 优先级复验。双 owner 使用定向夹具观察，公共场景核对实际动作交接。

四项定向通过。按 `target_game/play` 同入口启动 startup、moving_base、get_up，功能验收通过。原始报告长度和 SHA-256 见 [runtime.json](runtime.json)。
