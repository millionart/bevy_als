# BF144-D130c1：胶囊 body 局部形体与 bounds

fresh 源实现把 `FKSphylElem` 的中心、旋转、轴长和半径烘焙到 body 局部 `FCapsule`。其世界 bounds 由变换后的两个 float 端点加半径计算，而非先变换局部 AABB；shape pair 在首次访问时还做双向局部 bounds 检查，上帧活跃时跳过这两项。胶囊不允许 manifold 几何复用。源码摘录与哈希见 `source-facts.json`。

现从目标 capsule 和 collider/body 局部变换重建源方向、端点与 float 厘米长度/半径，并实现胶囊专用世界及双向局部 bounds；复用已有凸包 bounds。已写 head 原生端点、七组姿态/scale 的候选条件、上帧活跃与关闭 bounds 控制。当前转换由目标形体重建，尚需用新 reader 的原生轴、长度与缩放输出直接确认逐值等价，不能视为该 Gate 已通过。

reader 新增可选 `body_scale` 请求；`body-scale.request.json` 覆盖单位、均匀、非均匀及反射 scale，合同为 `tools/tests/test_physics_capsule_body_scale_probe.py`。旧 reader 不支持该字段，须在统一阶段重建后采集；本项引用的既有原生证据和请求哈希见 `native-identities.json`。真实碰撞接入、全量转换 diff、编译、测试、消融和 Playground 均待后续。
