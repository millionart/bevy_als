# BF144-D251 Rig 旋转构造

fresh 本地 `CR_Als` 读取包含 10 个图、33 个 RigVM 函数和 165 条指令；`ApplyFootIk` 消费 `FAlsRigUnit_ApplyFootOffsetRotation::Execute`。该 unit 在足部角度限位后调用 `FRotator::Quaternion`。UE ARM64 的 double 构造先求各半角的 sin/cos，再按 `CR * (SP * SY)` 等分组计算四元数，不额外归一化；目标公共 helper 同时供 Rig 数学操作和足部 unit 使用，坐标映射为源 `(x,y,z,w)` 到 runtime `(-x,-z,-y,w)`。

`rotation.request.json` 和 `rotation.native.json` 保存 warm 左脚、实际双脚、零旋转、半转及绕圈控制。warm 左脚源角度为 `(9.51228141784668, 34.04317855834961, −6.547454357147217)` 度，原生构造 w 为 `0.9499608725612713`，旧目标 helper 为 `0.9499608725612712`。`witness.json` 保存原生输出及坐标映射。`rig.native.json.gz` 为完整原生 Rig 读取，压缩时逐字节验证解压结果；源资产及输出长度、SHA-256 在 `native-identities.json`。先前 `.471` 封存了 684 个完整足旋转 native 控制及构造因素消融，本项不把它们称为新的目标运行结果；更早 `.289/.290` 的 target 原始报告当前缺失。

实现保留现有角度取余，将半角和每个分量的乘法按本地 UE ARM64 路径分组。精确原生 fixture 限定于 macOS ARM64，已有近似旋转和足部 unit 用例覆盖公共调用；其他平台的三角函数实现及不同角度域仍需验证。源 `VectorMod360` 使用截断除法与融合乘加，目标已有的 `%` 不在本差异范围内，不能据此宣称所有绕圈输入逐位等价。

fresh 源资产可从项目根以 `tools/run_ue_source_reader.ps1` 和 `rig.request.json` 中的资产、时限重读；旋转控制以 `rotation.request.json` 作为 `-GeometryQueryProbe` 输入。focused 测试、`.471` 完整消费者重放、删分组消融与源码字节恢复、图静态审计和公共 Playground Gate 均待全部静态修复及 Bevy/依赖迁移后执行，目前尚未运行。
