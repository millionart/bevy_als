# BF144-D104b1：box 接触复用

源 `FKBoxElem` 的无旋转分支将局部中心放入 AABB，shape 世界姿态使用刚体原点与刚体旋转；非单位局部旋转分支使用中心在原点的 box 与局部变换。`source-facts.json` 保存对应源代码、复用条件、睡眠唤醒和 Avian 的 midpoint/速度消费；源导出身份、原生请求及 8 组 box 双帧样本见 `identities.json` 指向的 D104a 证据。

目标按接触、两个 collider、两个 body 和形体变更 tick 保存重建帧与两侧初始表面点。活动接触复用失败时重取当帧接触，睡眠接触保留缓存；成功后恢复 midpoint、质心相对锚点、深度及旧表面点的法向速度。仅覆盖动态 ragdoll box 与静态 box 的持续目标接触；compound、trimesh 和源/目标窄相位几何差异按后续条目处理。

已写形体原点、旋转容差、法向速度、±2 mm 复用、6 mm 重建后再复用及普通 body 对照测试。源代码审阅和证据登记已完成；编译、测试、消融、动画图审计和真实 Playground 均待全部静态修复与依赖升级后的统一 Gate。
