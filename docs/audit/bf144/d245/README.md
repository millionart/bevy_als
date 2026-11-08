# BF144-D245 BlendList 满权重直传

UE `BlendListBase::Evaluate` 在恰好一个权重大于 `1e-5`、该权重不小于 `1 - 1e-5` 且无 BlendProfile 时直接求值该子节点。fresh Ragdolling 读取确认两个 gameplay-tag 节点的 Profile 均为 None；fresh Standing 的 258 图中，左右 Plant Foot 的四个唯一 enum BlendList 节点也均无 Profile。UE bool 和 enum 类型同样继承该求值实现。

`blend.request.json` 包含三个受控原生样本：满权重 child 0、满权重 child 1，以及从 child 0 到 child 1 的 0.5/0.5 过渡。前两项的完整 79 骨 TQS 和曲线分别逐值等于对应输入，双输入输出不同于两端；原始结果、Standing 图、请求及无损压缩哈希均在本目录。`.458` 保存此前 319 组实际同输入资格和完整消费者证据；新样本是独立求值控制，不代表连续真实运动历史。

项目 tag、bool 和 enum 三个无 Profile BlendList 节点共用相同直传条件，返回原 Pose 和该子节点的曲线；更新相关的 sync/sample 权重仍沿用原合并。其他 TwoWay、MultiWay 和 Slot 的普通混合 helper 不变。已写非单位四元数、Pose metadata/root-motion、近满权重曲线、双输入及三个节点入口 fixture；UE 原生输出中的 root-motion metadata 尚未单独验证。child 更新 relevancy 阈值与所有运行时 Gate 保持独立，测试、消融/恢复、图审计和公共 Playground Gate 待全部静态修复及 Bevy/依赖迁移后执行。
