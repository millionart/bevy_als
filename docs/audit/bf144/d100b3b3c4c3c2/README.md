# BF144-D100b3b3c4c3c2：窄相位入口

源端在每个 shape pair 的 bounds 检查后执行接触恢复或生成，再按源 phi 与 cull 决定是否激活约束。目标 Avian 在调用 modify hook 之前先以自身窄相位结果裁掉空 manifold，因此无法用该 hook 修复目标没有点而源有点的情况。两侧当前源码摘录和哈希分别见 `source-facts.json`、`target-facts.json`；原生 Barrel 样本引用 D100b3b3c4c3c1。

真实物理入口改用 Avian 公共 `AnyCollider`/`NarrowPhasePlugin` 扩展点。原 `Collider` 仍负责碰撞树、查询和质量；窄相位组件在每次计算前同步原 collider，当前完整转发其几何与质量计算。原碰撞过滤 hook 及 Avian 的事件、约束图和休眠岛调度保持原入口。源接触判定仍待 D100b3b3c4c3c3，当前不能声称 Barrel 已修复。

已写普通形体几何及质量逐值对照，以及 collider 替换、移除的物理生命周期测试。所有测试、编译、消融、全量转换 diff 和 Playground 待统一 Gate；消融须核对去掉窄相位适配后的 Barrel 最小反例。
