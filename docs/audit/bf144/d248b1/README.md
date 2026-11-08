# BF144-D248b1 Head additive 的单精度归一化

UE ARM64 的浮点 quaternion 归一化使用倒平方根估算与两轮 NEON refinement；目标 glam 原方法先开方再取倒数。`.454` 固定同一 current F32 输入、原 Dot4、乘法顺序及 delta/product/local 三个阶段，只改变这一方法，保存了 318 帧的受控输出与消融；该控制不等于完整源 F64 算法。

fresh `head.request.json` 重采第一帧 current/current 和 source/source 两种输入，完整原生 pose 与 `.451b` 对应记录一致。相同 current 输入下 `VB ik_hand_l` 的 native F32 y 为 `0.6696646213531494`，旧目标为 `0.6696646809577942`，`.454` 的仅方法控制与 native 该分量一致；其他分量仍有差异。`head-fixture.json` 从 Git 记录提取该骨的 12 节祖先链和同一方法控制结果，供公开 mesh-space additive fixture 使用，不称该整骨或完整 pose 已与源相同。

目标三个现有归一化点复用 ARM64 F32 estimate/refinement，其他架构保持原 `Quat::normalize`；同一函数也供 local additive、组合及其 root-motion 路径使用，这些相邻路径的独立 native 效果尚未验证。F32 Dot4、乘法与阶段次数没有在此改变，Layered Pose 留 D248b2；零/非有限输入、完整 Root 和跨平台仍待验证。原生请求/结果、源事实和无损压缩哈希均在本目录，focused 测试、完整因素回归、消融恢复及公共 Playground Gate 待全部静态修复与 Bevy/依赖迁移后执行。
