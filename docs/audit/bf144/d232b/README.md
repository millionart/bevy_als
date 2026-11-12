# D232b 缓存、链接与状态转场姿态

`AnimNode_SaveCachedPose.cpp:129–163` 按求值计数及缓存作用域存取整份骨骼姿态。Linked Input 和 Linked Anim Graph 通过各自的 PoseLink 转发结果。目标精确姿态随原 pose context 经过同一节点、pin、scope 和 state key，图包装只取实际输出 pin 的 context。反射快照沿用 `Clone`，保留同步预采样所需的姿态引用。实际执行惯性变换时释放上游精确姿态引用。

`AnimNode_MultiWayBlend.cpp:126–180` 保留输入顺序，公共加权核在多输入时归一化，节点末尾再归一化。无输入且为非加性时使用 mesh reference。 `AnimNode_BlendListBase.cpp:301–385` 直通唯一满权重输入，双输入快路径从首权重回算补数，普通加权路径仅在多输入时归一化。 `AnimNode_TwoWayBlend.cpp:122–152` 使用公共双输入求值，权重补数与曲线 Lerp 的生产调整分别沿 D234 与 D233 执行。五个安装播放器显式 RefPose root lock 均与当前压缩数据的源模式一致。

`AnimNode_StateMachine.cpp:993–1024,1044–1121` 用实际前态或前一转场的未归一化输出作为首输入，并用 Profile 双权重逐骨累积。中断链携带对应精确姿态，只有 FSM 最终输出归一化。现有同步、根运动属性与 morph 通道保持各自的权重来源。

受控父节点以 D231 三份真实 WalkRun 姿态作为输入。原生对照包含 2054 个 BlendList 骨骼、711 个 MultiWay/TwoWay 算子读数、13509 对 FSM 原始与最终骨骼结果。去掉 FSM 末尾归一化会改变 12861 条 double 变换。提前投影改变 615 条 BlendList、114 条 MultiWay、114 条 TwoWay 和 8907 条最终 FSM f32 变换。32 个缓存调度帧与 D221 原样一致，其中 16 帧有多个消费者共享一次最大权重更新。Rust fixture 把同一 f32 投影的两份不同精确姿态送入独立缓存，经链接图交替直通或混合，并检验反射克隆。Rust fixture 随统一验证执行。

首次请求把捕获输出对象直接用作输入，现有 probe 要求数组格式并返回 7。按其读取合同转换字段后四份原生读取通过。

```sh
python docs/audit/bf144/d232b/replay_pose_chain.py --requests
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/Stances/AB_Als_Standing -BlendListProbe docs/audit/bf144/d232b/two.request.json -Output docs/audit/bf144/d232b/two.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/Stances/AB_Als_Standing -BlendListProbe docs/audit/bf144/d232b/multi.request.json -Output docs/audit/bf144/d232b/multi.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/SK_Als -BlendProfileProbe docs/audit/bf144/d232b/operators.request.json -Output docs/audit/bf144/d232b/operators.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/Stances/AB_Als_Standing -BlendListProbe docs/audit/bf144/d221/weights.request.json -Output docs/audit/bf144/d232b/cache.native.json
python docs/audit/bf144/d232b/replay_pose_chain.py
```
