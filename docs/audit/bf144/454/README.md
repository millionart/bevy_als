# BF144.454：实际 Head 的 F32 运算方法

复用318实际Head输入、82路径骨架、公开核输出以及同current输入的native CC结果。固定所有输入、F32类型、有效alpha1、delta/product/local三个归一化阶段，只分别改变乘法累加顺序或倒平方根方法。baseline完整80骨/25,440旋转行精确实际公开核，非旋转字段保持。

UE源QuaternionMultiply按W→X→Y→Z累计；glam NEON将WX和YZ两对结果相加。控制按已证source/runtime坐标基转换，在F32保留源累加顺序。reciprocal控制使用estimate加两轮refinement，但保留当前glam Dot4；这个控制不是完整源F64 normalizer，也不把当前额外归一化阶段一起删除。

| 方法控制 | 改变的旋转骨行 | 对共同输入native恢复的分量 | 失去原native匹配的分量 |
|---|---:|---:|---:|
| 仅源乘法顺序 | 22209 | 9659 | 10528 |
| 仅reciprocal方法 | 3172 | 2643 | 1523 |
| 两者 | 见完整四域表 | 10731 | 10529 |

原/仅method/仅order/两者对native仍有23032/23048/23116/23138个旋转骨行不等。这些是分开的必要因素与交互，不是可直接施加的生产修复。部分分量恢复、另一些失配，不能把结果相加或称完整姿态改善。

最小current合同分别对native失败，单独对应method控制精确通过。首frame0 `VB hand_l_to_ik_hand_gun` q.y从−0.669664740562439变为native −0.6696646809577942；reciprocal首`VB ik_hand_l` q.y从0.6696646809577942变为native0.6696646213531494。oracle是共同current输入的原生运算，不是原source实际数据；四元数／Root整体仍未恢复。

两个方法各自物理删除后丢因素信号，分析拒绝；源码以字节恢复并同target六库重编，完整输出、分析和逐帧表精确恢复。四个坏矩阵fixture拒绝；两个current合同失败、两个controlled合同通过。首次运行的临时日志目录缺失与误替换骨行计数均已纠正，原数值逐骨校准始终通过；计数失败报告与修正记录保留，未放宽数值边界。

复验：`run_replay.ps1 -Label <new-label> -Build`，`run_python.ps1 -Name analyze -Case <new-label>`。源事实与identity需要环境变量`ALS_UE_SOURCE_ROOT`；glam根从Cargo注册目录解析，不写本地绝对路径。完整消融保存基线已有Git原件，可直接运行`mutate.py <control>`、runner、`mutate.py restore`；`controls.ps1`拒绝覆盖已保存基线。六个大输出无损gzip，解压SHA、所有依赖Git跟踪由manifest绑定。

只扩既有D163/D248，未新增编号；272项未修。生产/reader/引擎/原资产不改，无新真实场景启动；剩余数据、Rig与最终Root继续，截图0、性能暂停，GPU周边排除。
