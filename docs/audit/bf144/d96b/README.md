# BF144-D96b 真实接触的摩擦锚点历史

fresh PA及8组跨帧PhysicsContactProbe与D96a受跟踪原始结果逐字节相同。源将锚点存于两个shape的局部空间，下一帧按quadratic侧和strict距离匹配，再用预测后的shape变换形成切向误差；位置与末轮速度摩擦得到的ratio决定下一次保存。请求、reader报告、源/目标坐标与生命周期片段均已保存。

目标按ContactId、碰撞体与body身份及shape变更tick保存局部锚点，处理碰撞体顺序交换、4/6/8/14点保存规则及接触消失清理；匹配后使用自由运动预测变换恢复世界切向差，再交给D94位置摩擦。单接触速度锥裁剪可更新保存ratio，其平方容差可由 `python3 tools/import/import_angular_impulse_threshold.py --facts docs/audit/bf144/d96b/source-facts.json --output src/avian_ext/velocity_friction_squared_tolerance.rs` 重建（两个物理量均按长度平方换算）。已写局部/世界坐标、身份/shape变化、原生保存匹配、速度ratio以及加载head的跨帧±1mm/5mm/超距和失效清理对照。

目标按动态body一阶积分及Playground运动学目标速度分别预测双方shape；新接触几何时机归D98，复杂多点的顺序和速度采样归D100/D137/D138。运动学接触及完整生命周期的实际行为、测试、消融与Playground均待统一Gate。
