# BF144.448a 新域原生Root五边界与属性事实

同 `.446f` 的engine帧、初态和318+2请求，仅开启现有 `capture_pose_boundaries`。每个初始化及普通帧读取Rig Source、main Root Result、Head Base、Head Additive与BaseProvider Input的原生Pose、79骨、曲线、update/evaluation、engine帧、delta和custom attribute计数。两次完整character probe逐值精确；当前全部五边界的attribute_count均0，仅限本cohort，不泛化非空属性或其它模式。

此开关还按现有实现增加642份LayeredBlend诊断包。扣除 `pose_boundaries` 与 `layered_blends` 两类新增观察字段后，整个旧character probe逐值不变；关闭观察的完整probe也精确等于旧报告。关闭后边界校验缺字段失败，恢复请求原始字节、同未修改native package重读后完整character probe精确恢复。没有reader、生产或引擎代码变化，因此没有重编未变源码。

首次校验错误地假定raw Root Result等于最终mesh local，已按fresh `USkeletalMeshComponent::FinalizePoseEvaluationResult` 的NormalizeRotations阶段修正读点合同：T/S全相等，17465个原生旋转骨行在两个source阶段不同。保留二者原值，不把source内部阶段差记成移植缺陷，不再用最终mesh当raw Root oracle。后续target public Root应对本目录raw Root Result。另一次误传新输入label而缺文件只作工具前置失败，不作消融证明。

fresh主AB原生图与target RON核Root←Ragdolling←ControlRig←Head三条直接边。四个缺边界/错engine帧/非空属性/缺骨负例拒绝；396现行身份、六库、原main与final package source保持。原始native四报告采用mtime=0无损gzip，逐份解压bytes和SHA验证，`compression-map.json`保留原身份，长期证据全部Git跟踪。

复验 `pwsh docs/audit/bf144/448a/run_python.ps1 -Name check -Case restored`。新的native运行用新label，避免覆盖封存；请求 `request.json`。完整target Root和余差归属依 `.448b/.448c`，本项只闭原生读点与本域属性事实。无新增编号，271未修；截图0、性能暂停。

主图重读：`pwsh tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AB_Als.AB_Als -Output docs/audit/bf144/448a/new-root.native.json -TimeoutSeconds 180 -NoProgressSeconds 90`。
