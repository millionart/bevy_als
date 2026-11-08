# BF144-D18 Falling 胶囊的世界旋转

fresh UE `PhysFalling` 从 `UpdatedComponent.GetComponentQuat()` 取得世界旋转并传给移动扫掠。原生几何探针将 60° 胶囊/平墙关系共同变换到胶囊局部坐标后，测得命中距离 113.03848 cm；直立对照为 165 cm。坐标系变换保持相对几何与距离，探针请求、原始读数和源代码见本目录。

目标在 Air、单步和多步 Walk→Air 三个 Falling 入口传入角色世界旋转，并在位置写回时保留原有局部旋转与尺度。已写加载角色直接旋转、父级旋转、yaw 和 pitch 的实际碰撞对照；三个入口的统一运行验证尚未执行。历史目标报告 `target/bf144/06/world_rotation/` 已缺失；测试、消融、图审计和真实 Playground Gate 待全部静态修复及 Bevy/依赖升级后执行。
