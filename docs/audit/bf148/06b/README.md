# BF148.06b F32 NEON 消融

新读 Head 与 Layering。UE 的 ApplyMeshSpaceAdditive 在求值后归一化，ARM64 F32 倒开方在 `UnrealMathNeon.h` 1698–1724 用两轮细化。现有目标 helper 有 additive、layered、profile blend 消费者。Layered Blend 的实际导出 Pose 支持不完整骨骼通道。当自身权重为零、两份输入均无子骨通道且父旋转与源值逐值相等时，路径跳过生成该子骨。

最初夹具在空 mask 上调用 update，因此父权重也为零。发现后改用 add 注册父权重并断言父 1、子 0，再对比同一个生产 `apply_mesh_space_layered_blend` 函数。标量试验只替换 `normalize_pose_rotation` 的函数体为 `rotation.normalize()`。两版独立使用同 profile 的实际 crate 库，原实现按源码字节恢复并同 target 重编。

[sweep.rs](sweep.rs) 的 4,096 组合法单位旋转中，原实现生成 214 组子骨通道，标量试编生成 1,388 组，1,174 组的通道选择不同。第 5 组为 [compare.rs](compare.rs) 的同输入反例，原实现父旋转等于输入并省略子骨，标量父旋转改变并生成子骨旋转通道。恢复后反例重放得到原通道选择。两份矩阵及原始数据哈希见 [sweep.json](sweep.json)。这个行为差异拒绝标量替换，F32 NEON 保留。

可按 BF148.06a 的 rustc 链接方式运行两个夹具，并增加 `--extern bevy_animation_graph=<同一构建的 libbevy_animation_graph rlib>`。对照库由同 target/profile 分别编译原函数和标量函数得到。生产恢复后以全库、集成及公共图场景验收。

恢复版的 2157 项单元测试、37 项集成测试通过，定向 `unnecessary_wraps` 扫描仍为零。startup、head、locomotion 功能 Gate 通过。二进制与源码身份见 [identity.json](identity.json)，原始报告长度及哈希见 [runtime.json](runtime.json)。
