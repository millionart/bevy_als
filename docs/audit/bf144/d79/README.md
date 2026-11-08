# BF144-D79 混合角限位退化保护

fresh Chaos源码确认scalar cached分支在connector Twist轴点积低于native阈值时跳过Limited Twist与单Limited Swing，保留Locked轴；两Swing均Limited的pyramid分支和三轴soft SIMD分支不使用该跳过条件。fresh PA原生读取与D65a受跟踪输出逐字节相同，当前阈值为−0.998。

执行 `python3 tools/import/import_joint_degenerate_limit.py --source docs/audit/bf144/d65a/physics_asset.native.json.gz --output src/avian_ext/joint_degenerate_rotation_limit.rs` 可重建独立目标参数。目标只在源指定的cached scalar条件下跳过相关限位；已写neck阈值两侧、硬锁保留、双Swing/SIMD控制和真实PhysicsSchedule对照。旧缓存反例不可用；测试、消融、完整角色及Playground均待统一执行。
