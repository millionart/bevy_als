# BF144-D07a Walking 输入加速度阶段

本地 UE `ControlledCharacterMove` 在执行移动前用当时的 `GetMaxAcceleration` 缩放输入并计算 analog modifier；ALS `PhysWalking` 随后刷新当帧 gait 曲线，`CalcVelocity` 的普通玩家输入仍使用先前缓存的 Acceleration，而本帧制动和摩擦采用刷新值。fresh 原生 CMC 核在同一 400.0003 cm/s 初速、16.972263 m/s² 当前最大加速度下，缓存输入 20 得到 433.333635 cm/s，直接重算为当前 16.972263 得到 428.287406 cm/s；零输入保持初速。请求为受控 kernel 输入，不冒称完整角色运动历史。

目标现保留已存在的 ControlledCharacterMove 输入最大加速度，并只在普通地面 motor 和同模式 Walking 多子步积分时替换输入加速度；刷新后的动力组件、制动、摩擦与其他消费者保持。已补真实加载角色的首帧/下一帧缓存及 60 ms 多子步控制，并调整 D05 与既有 analog 边界 fixture 使其显式提供前帧缓存；这些测试尚未运行。

本子项不关闭 Falling→Walking 同帧余步及刷新值寿命，留给 D07b。native 请求/结果、源码和哈希在本目录；测试、消融、图审计和公共 Gate 按用户顺序待全部静态修复和 Bevy/依赖升级后执行。
