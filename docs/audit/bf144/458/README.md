# BF144.458：Ragdolling 单满权重传递阶段

fresh 原生 Ragdolling 编译图确认外层a88的Base来自277、BlendProfile None、Linear；ALS仅实现selector，Evaluate继承UE BlendListBase。源在一个活动pose/满权重/无Profile时直接Evaluate child。目标同节点进入共享blend_source_poses，即使只有一个输入也Normalize。

原可见Playground的319组实际状态全部initialized、child0、weights[1,0]，内层从未求值；读取BAG真实upcoming Default或last state，而非用动作请求构造权重。非默认局部state fixture用[.25,.75]/child1验证两个存储路径，写死[1,0]会在fixture失败。此局部fixture明确不充当真实运行状态。

实际Base完整等于Rig输出，public helper完整等于节点缓存、Ragdolling graph root和最终Root。全319组多3728个旋转骨行，其中318正步长多3714，T/S/weights/Pose metadata均保持；全部变化精确由单输入Normalize复现。复制值控制只保留Base，没有应用到runtime或修改图。首query0 ball_l整个quat当前合同失败，源copy值控制通过；延伸已确认D245，没有新编号。

原生BlendList API对319个实际79骨输入单独初始化child0，delta0只用来建立满权重Evaluate fixture，不冒作真实连续更新。全部25,201骨TQS输入精确，完整输出/曲线与child0逐值相同；未活动第二输入显式放同值，不冒实际Flail或Snapshot。首批因Rig与Skeleton虚拟骨顺序不同被拒，原错误请求/报告保留；修正为原生实际Skeleton骨名顺序后重新读取、校准通过。

删除新观察字段使合同缺字段失败；把state reader权重常量化使非默认fixture失败。源码按字节恢复，同target六库重编重跑后，完整参数、检查、definition、skeleton和state-reader fixture字节精确，原旧五数组不变；两次原场景functional-only过，原预算仍deferred。源reader保持457版本，没有修改生产/图/原资产/引擎。

复验：compile.ps1 -Label <new>，run.ps1 -Label <new>，run_python.ps1 -Name check -Case <new>，functional.ps1 -Label <new>。prepare从实际输入与source-bone-order生成共同输入请求，run_native.ps1与native_check验证原生；source事实/identity要求ALS_UE_SOURCE_ROOT。ablate.py支持no-observation、constant-state、restore；每次必须等编译终态再运行。所有长期证据在Git目录，七个大报告无损gzip并绑定原SHA；build/log可重建且不是唯一证明。

当前Ragdolling这批额外Normalize已有完整归属。它不代表实际Ragdoll动作、双输入过渡或整个Rig/Root已等价；接下来用已采实际VM状态继续Rig输出的输入/写者归因。272项差异仍未修，截图0、性能暂停，GPU周边排除。
