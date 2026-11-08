# BF144-D04 零倍率半转瞄准限位

fresh `CS_Als_Default` 原生读取确认瞄准限位为 70°；该次结果与 D01 保存的原生 JSON 字节相同，故本目录记录本次请求、watchdog 结果和哈希，并复用 D01 原文。本地 ALS 源在 counterclockwise remap 后，以未再次 remap 的角差做 secondary 插值，`FMath::UnwindDegrees` 保留精确的 ±180°，然后执行 primary clamp。UE actor tick 会把自身 `CustomTimeDilation` 乘到 delta 后继续调用 Tick，倍率 0 因此仍执行约束。

目标 secondary 分支改为对应的角差、阻尼和保留半转符号的 unwind；不改变其他旋转路径。已有源公式下，源 view −180°、actor 0°、limit 70°、delta 0 得到源 actor −110°，按项目坐标基映射为 runtime +110°；这是源码公式推导，不是额外声称原生函数实测。已写半转两侧、倍率 0/0.001/1 以及真实 rotation-response 系统控制，旧的非零半衰期测试保留。

尚未执行 5040 组差分、focused 测试、消融、图审计或公共 Playground Gate；这些按用户顺序在全部静态修复和 Bevy/依赖升级后统一执行。旧 `target/bf144/02/aim` 原始证据不可用。
