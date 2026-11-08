# BF144-D100b3b3c4c3c3b：源接触进入真实窄相位

源 shape pair 先运行 bounds、复用或 GJK，再按源 phi 与 cull 激活；目标 Avian 在设置 touching、事件和 solver 槽前，只对非空目标 manifold 调用 modify hook。源和目标源码摘录见 `source-facts.json`、`target-facts.json`；Barrel、Bench 与复用跨 cull 的原生读数和哈希引用见 `native-identities.json`。

真实入口的窄相位适配器只对具备源资产、动态 ragdoll box、静态凸包、所需设置和历史的碰撞对提供一个临时点，使 Avian 调用源接触 hook。hook 在 Avian 判定 touching 前用共享源算法替换全部临时数据；源无活动组时返回 false。非源形体、sensor、缺设置及普通碰撞直接转发原几何。源逐 hull 历史通过窄相位 deferred 命令写回，后续组序阶段识别已处理的碰撞对，保留休眠历史并按实际求解槽重排，不重复生成接触。

已将 Barrel、Bench、Arch 原生夹具切到真实物理入口，编写接触开始、结束、重新接触及 30 帧休眠/5 帧唤醒控制；原普通碰撞与空 World 的 hook 控制继续保留。目标编译、测试、消融、全量转换 diff 和 Playground 尚未执行，必须在统一 Gate 中检查临时点不会泄漏到求解器并覆盖命令同步。
