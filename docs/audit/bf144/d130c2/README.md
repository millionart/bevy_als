# BF144-D130c2：胶囊/凸包真实碰撞入口

fresh head/Barrel 原生聚合在 2.4 cm 间距激活两个点，在 3.5 cm 时没有活动组；head/Arch 的原始与 1 mm 位移样本均有 hull 0、2。源胶囊 shape pair 不复用上一帧 manifold，上帧是否活跃只用于 bounds 的局部检查。请求、原始报告、精简样本和哈希见 `native-identities.json`，源及目标消费者代码见 `source-facts.json`、`target-facts.json`。

真实窄相位现按胶囊 body 局部形体、源 bounds、cull 和源 GJK/裁线逐 hull 生成点组；每帧重建几何，历史只保存活动身份及原始源点。空目标 manifold 仍能进入源判定，Avian 继续负责 touching、事件和 solver 槽。已写原生正间距、组序、小位移重建、事件及有限帧睡眠/唤醒测试；非源碰撞仍由原路径处理。

源每点可携带不同法向，而当前 Avian manifold 使用一个共享法向。Arch 样本的组内最大法向差约 `5.97e-8`；原始逐点法向已保存在源接触历史中，目前没有证实该差值造成消费者失败。统一 Gate 须检查既有容差及后续 D137 消费，若失败则按对应条目复开，不能声称逐点 solver 法向已完全对齐。目标 body scale 的源轴/长度直接核验、编译、测试、消融、全量转换 diff 和 Playground 均未执行。
