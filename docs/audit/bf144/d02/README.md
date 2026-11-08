# BF144-D02 出生时初始化目标朝向

fresh 本地 `B_Als_Character` 以非零 engine frame 17 和源 yaw 0°、45°、−90°、170° 分别出生；注册快照中的输入 yaw、速度 yaw 和目标 yaw 均与出生角相同。源 `PreRegisterAllComponents` 从 actor 世界朝向调用 `SetTargetYawAngle`，并设置输入及速度方向；该 setter 同时初始化平滑目标。源和目标 yaw 符号按项目坐标基相反。原生请求、四组读数、源码和哈希已保存并核对压缩字节。

目标在首次计算旋转响应时，用当前 actor 世界 yaw 初始化尚未初始化的目标；现有首次平滑目标初始化继续执行，已初始化的目标和后续保留行为不改变。`velocity_direction_initializes_target_from_birth_world_yaw_once` 已写入四个出生角、无父节点/旋转父节点、已初始化控制和后续外部转向后目标保留的覆盖；尚未执行。

该项修复第一次正常旋转响应；`AlsMovementInputBlocked` 或资源尚未准备而提前跳过整个响应时的更早初始化边界不在当前源反例内。focused 测试、消融、图审计和公共 Playground Gate 均待全部静态修复及 Bevy/依赖升级后执行；旧 `target/bf144/02/spawn/` 报告缺失。
