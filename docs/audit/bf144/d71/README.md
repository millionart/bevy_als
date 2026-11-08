# BF144-D71 缩放刚体的惯量分解

fresh UE `BodyUtils::ComputeMassProperties` 与 Chaos `TransformToLocalSpace` 确认源使用 kg·cm²，先判断均值是否不大于 `UE_SMALL_NUMBER`，再以非对角平方和除均值平方比较同一门槛。源资产与reader身份、1.0/0.2 scale的全部原生body主惯量和frame保留在本目录；原审计缓存没有作为唯一证据。

执行 `python3 tools/import/export_body_inertia_probe.py --source docs/audit/bf144/d71/body_probe.native.json.gz --output docs/audit/bf144/d71/body_probe.runtime.json` 以既有坐标基转换轴、旋转与惯量单位。执行 `python3 docs/audit/bf144/d71/compare_inputs.py --native docs/audit/bf144/d71/body_probe.runtime.json --profile assets/ALS/Character/PA_Als.physics.json --output docs/audit/bf144/d71/source_target_inputs.json` 可重建十组源/目标分解前静态矩阵对照；这些对照不构成目标运行验证。输入最大相对差约 2.2e-7，0.2 head的源XZ为约 4.089e-6 kg·m²。

目标保留现有本征分解器，对源近对角/极小条件使用源阈值，其他矩阵按均值归一化后分解再恢复单位。已写阈值跨尺度、原生frame对应的实际rig body和角冲量响应测试；矩阵相对容差1e-4覆盖旧记录的默认head约1.43e-6绝对分解差，待以执行结果核对。编译、测试、消融及真实Playground均未执行；相等或近重本征值时的目标算法与Chaos数值差异仍待统一验证。
