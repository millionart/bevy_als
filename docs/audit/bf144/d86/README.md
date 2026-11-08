# BF144-D86 源世界物理单步

fresh PhysicsSettings的 `bSubstepping=False`、`bTickPhysicsAsync=False`；虽然MaxSubsteps字段为6，fresh `FChaosScene::SetUpForFrame` 在此分支明确将求解器上限设为1，AsyncDt为−1，实际每物理帧最多推进一次并限制物理delta。native settings和PA与D81/D65a受跟踪输出逐字节相同，source调用链、请求、报告和身份已保留；旧 `target/bf144/26/initial/` 缓存缺失。

执行 `python3 tools/import/import_physics_substeps.py --settings docs/audit/bf144/d81/settings.native.json --output src/als/settings/physics_substeps.rs` 可重建独立目标配置；工具对尚未移植的源异步/substepping模式明确失败。源物理时钟插件设置物理子步为1，现有delta clamp仅作用于物理时钟，不固定游戏帧率。已写加载rig在60/30Hz和100ms帧、有无重力的步数、速度和半隐式位移对照；D84单独补回六子步扩展控制。测试、消融、完整关节轨迹和Playground待统一执行。
