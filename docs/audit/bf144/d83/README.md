# BF144-D83 硬角速度约束的冲量阈值

fresh Chaos cached solver只在非soft角约束的累计lambda绝对值大于 `UE_SMALL_NUMBER` 时修正角速度。源lambda含惯量量纲，源长度cm到目标m需平方缩放：`1e-8 kg·cm²` 对应 `1e-12 kg·m²`；cached quaternion Jacobian的轴长度已包含在实际lambda中。fresh PA读取与D65a受跟踪输出逐字节相同，源码片段、请求、报告和身份存于本目录，旧临时反例缓存缺失。

执行 `python3 tools/import/import_angular_impulse_threshold.py --facts docs/audit/bf144/d83/source-facts.json --output src/avian_ext/angular_impulse_threshold.rs` 可重建目标阈值。目标仅对cached求解使用该阈值，非cached保持原路径；已写阈值正负边界及clavicle/hand在1/0.75/0.5 scale、小角度下的实际solver对照。测试、消融、完整角色与Playground待统一执行；未宣称其他角速度轴向误差已解决。
