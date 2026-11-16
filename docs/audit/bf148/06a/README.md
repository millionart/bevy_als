# BF148.06a F64 NEON 消融

本地 UE `Quat.h` 1105–1116 的 Normalize 用 Dot4 与退化判定；`UnrealMathNeon.h` 1030–1038、1727–1740 使用 FMA 分组和两轮倒开方细化。`UnrealMath.cpp` 1226–1258 的 TQuat Slerp 以 `0.9999` 区分球面与线性分支。新读 CR_Als 编译 Rig 和 GeometryQueryProbe 同输入归一化见 [native.json](native.json)。

[compare.rs](compare.rs) 以合法单位四元数为受控输入，在真实 RigTransform::global 归一化与 rig_slerp 消费者之间比较标量倒开方。标量保留原 Dot4 算术和退化判定。对应 UE 轴为 `(-x,-z,-y,w)`，原生归一化后余弦与当前 Rust 逐值相同。当前余弦 0.99989999993191248 进入球面插值，标量 0.99990000003000001 进入线性插值。权重 0.25 时实际输出 X 轴角相差约 8.84×10⁻⁸ rad。部分权重 Rig IK 在同一消费者做插值并重建子骨位置。见 [comparison.json](comparison.json) 与 [native-branch.json](native-branch.json)。

这是消费分支改变的受控反例。消融只在夹具中做，F64 生产实现保留。六项原生数学和层级定向复验通过，公共 Gate 检查当前实际 Control Rig 与 Foot IK。

可用 `rustc --edition=2024 docs/audit/bf148/06a/compare.rs -L dependency=target_game/play/deps --extern bevy=<同一构建的 libbevy rlib> --extern bevy_als=<同一构建的 libbevy_als rlib> -o <临时程序>` 重跑比较。原生请求参数记录在 `native.json`。

同一 `target_game/play` 二进制的 startup、control_rig 和 foot_ik 功能通过，原始报告哈希见 [runtime.json](runtime.json)。
