# BF144-D94b 位置摩擦接入接触求解

fresh PA与PhysicsSettings确认位置求解8轮、其中末4轮有摩擦，速度求解2轮、其中末1轮有摩擦；位置刚度0.5、速度刚度1，当前使用双切向方向和零基础摩擦冲量。两份原生输出分别与D65a和D81受跟踪记录逐字节相同，source、请求及reader报告已保存。可执行 `python3 tools/import/import_contact_friction_settings.py --facts docs/audit/bf144/d94b/source-facts.json --settings docs/audit/bf144/d81/settings.native.json --native docs/audit/bf144/d94b/native-friction.json --output src/als/settings/contact_friction.rs` 重建当前世界参数；尚未实现的速度摩擦配置由工具明确拒绝。

目标在积分速度后采集Ragdoll接触的自由运动及固定力臂，暂时取出Avian切向约束；位置积分后按D94a计算位置摩擦，将位置和角度修正转换为速度，再把位置累计冲量映射回目标切向约束供速度松弛阶段消费。非Ragdoll接触保留原路径。已写加载head的30/60Hz、原生惯量、零摩擦及双切向闭式对照，涉及摩擦锥与实际物理调度。

当前闭包针对首帧与零历史偏移、单接触及已证实的硬材质；非零锚点历史归D96，法向与切向速度采样顺序和分轴质量归D137/D138，复杂多点及法向位置耦合仍待后续条目。姿态修正的组合、测试、消融与Playground均待统一Gate。
