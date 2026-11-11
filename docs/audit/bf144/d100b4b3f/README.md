# D100b4b3f 网格接触原序与局部输出

源两项排序开关关闭，法线修正后稳定压缩有效点。第二支持点和法线经网格到凸体的逆刚体变换回网格局部，保留已应用到几何的 scale。约束先以 double `phi < cull` 过滤，再用 `FContactPointf` 写入。源 `p.Chaos.Collision.MaxManifoldPoints` 默认 `-1`，本地配置未发现覆盖，已把该 CVar 加入 reader，统一重采时确认实际值。接触深度、原 cooked 面号及类型保持原序。

`prepare_cases.py` 从 D100b、D104b3b、a2、a3 的原始读数整理 9 个生产姿态和对照，共 37 个源点。focused 测试要求 `ALS_MESH_BVH_TARGET_DATA` 指向 b4a2 的生成数据，逐点比较数量、原序、面号、类型、位置、法线及深度。另有带旋转和负缩放的最小控制，覆盖无 scale 逆变换、有效点稳定压缩和转 float 前的严格 cull。实际容量和 BVH 数据待 reader 重建后生成，编译、测试、消融、Playground 仍待统一 Gate。
