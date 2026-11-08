# BF144-D92 普通接触回弹速度门槛

fresh PA读取的`p.CollisionRestitutionThreshold=-1`，源世界因此使用1000cm/s²默认值；容器以实际物理delta乘该值，且只在入射速度严格大于阈值时回弹。源项目`BounceThresholdVelocity`没有进入此世界调用。原生输出与D65a记录逐字节相同，调用片段、身份和reader报告已保存。

执行 `python3 tools/import/import_restitution_threshold.py --facts docs/audit/bf144/d92/source-facts.json --native docs/audit/bf144/d92/native-threshold.json --output src/als/settings/restitution_acceleration.rs` 可生成目标加速度。目标在每物理帧按clamped physics delta更新Avian世界阈值，考虑Avian长度单位，并用下一个可表示值适配目标的包含等号比较；游戏帧时钟不变。已写30/60/120Hz、长帧、等号两侧和加载head实际接触对照。

测试、消融、复杂接触与Playground待统一Gate；非默认长度单位的舍入及后续碰撞求解差异须在统一验证时复核。
