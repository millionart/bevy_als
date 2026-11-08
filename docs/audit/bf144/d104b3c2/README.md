# BF144-D104b3c2：三角网格缓存与求解槽

真实 Playground 的动态 ragdoll box 对静态源三角网格，在窄相位完成后按 collider、body、形体和源资产身份保存跨组接触；睡眠保持历史，唤醒后按源整体阈值尝试复用。重建时按目标求解器实际 overflow、颜色及槽顺序保存组；复用时按同一次序写回，使用当前接触材质，并通过 Avian 公开的 `ConstraintGraph::push_manifold` 与 `pop_manifold` 同步组数。

`source-facts.json` 保存目标窄相位、约束图、睡眠及求解遍历源码，源原生 Cart/Stairs 数据和源 bounds 由 `identities.json` 引用。已写实际两三角形接触的槽恢复及缩减、普通 body 对照、资产身份变化、有限帧睡眠/唤醒与槽顺序测试；现有源多点和写回测试保留。

此阶段要求目标碰撞对仍有活动 manifold；目标完全缺组时的形体生成和源三角面排序留在 D100b4 等后续条目，不能据此声称完整 mesh 几何对标。测试、消融、全量转换 diff 与真实 Playground 均待统一 Gate。
