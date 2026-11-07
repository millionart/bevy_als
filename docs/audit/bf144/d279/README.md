# BF144-D279 足目标缺失时的网格回退

`source-facts.json` 保存 fresh ALS 足目标读取、UE Socket/骨骼查找和目标两处消费者；`native-identities.json` 固定本轮骨架、网格、Control Rig 的原生读取和 watchdog 报告。标准骨架的10个 Socket 均不与所选足名相同，网格没有额外 Socket；在目标名既无 Socket 也无骨骼的已证边界，源返回网格世界变换。

`AlsFootBones::target` 改为显式返回可缺失的目标，足锁定和足探测在缺失时使用当前角色网格；已登记真实加载态标记移除、另一侧保持、Control Rig 输入及地面探测控制。旧 `docs/audit/bf144/490/replay.rs` 保留当时的错误行为和旧 API 作为历史源记录，需在对应旧提交复验；本项使用新的回归用例，不改写旧证据。自定义骨架中同名 Socket 的导入不由本项已保存的标准资产证明。

范围和递归源证据见 `tools/animgraph_authoring/bf144_d279.scope.json`。Focused 测试、消融、图静态审计及公共视觉 Gate 均待全部条目实现完成后执行。
