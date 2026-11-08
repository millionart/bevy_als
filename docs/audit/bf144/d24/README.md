# BF144-D24 蹲下时缩短胶囊后的下移扫掠

fresh UE `Crouch` 先设置新的胶囊尺寸，再以组件世界 quat 沿重力方向执行带扫掠的 base-location 移动。角色 CDO 的蹲姿半高为 56 cm，`bCrouchMaintainsBaseLocation` 初始为 false；ALS 在 movement mode 变化后将其设为 true。将角色/地面共同变换到胶囊局部坐标的原生几何探针显示，34 cm 下移请求在直立时无命中，60° 和 90° 时分别在约 19.15 cm、2.15 cm 命中地面。

目标蹲下前根据 D11 的源物理胶囊尺度构造缩短后的世界形体，按世界旋转扫掠实际下移，再写回中心和事务；D11 尺寸计算提取为共享函数。已写加载角色直立、倾斜、父级旋转和无地面对照。历史目标报告 `target/bf144/08/tilted_initial/` 已缺失；测试、消融、图审计和真实 Playground Gate 待全部静态修复及 Bevy/依赖升级后执行。
