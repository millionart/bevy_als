# BF144-D258 视角半转的源坐标规范化

本轮 fresh 源码确认：本地 controller 的视角先经 `FRotator::GetNormalized`，其 `NormalizeAxis` 将 ±180° 规范为 +180°；随后与角色 `ActorTransform.Rotator()` 相减，`UnwindDegrees` 保留相对角的 +180° 和 −180° 两个端点。Head 将相对视角限制到 ±175° 后送入弹簧。默认动画设置资产中 Head yaw 半衰期为 0.10000000149011612。

目标先将视角转到源坐标并规范，再从源坐标角色角相减、按源规则折返。这样静止角色的正负视角半转均发布 +180°，而非零角色角产生的相对 ±180° 仍可分别保留。已写公共 look 输入→动画 provider 的反例及相对角→Head 方向用例；±170°、非零角色角和普通左右方向同时保留。

本轮 fresh native reader 保存半转/±170°/±90° 的旋转请求和结果、11 子图的 `AB_Als_Head`、其 `BS_Als_Look` 及三个序列、默认动画设置与各自资产身份；源代码、哈希与报告见 `source-facts.json` 和 `native-identities.json`。旋转 probe 读取 FQuat/FRotator，controller 的 `GetNormalized` 结论来自 fresh C++ 源码，不将其当作 probe 直接调用。旧 `.379` 的 160 帧报告及 `.381` Head 姿态报告当前缺失。

focused 测试、原角度顺序消融、源码字节恢复后同启动器 target 重编、完整图静态审计与 `startup`/`head` 公共视觉 Gate 均待全部静态修复和 Bevy/依赖迁移后执行，目前尚未运行。
