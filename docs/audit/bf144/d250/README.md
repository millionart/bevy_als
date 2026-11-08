# BF144-D250 Rig 加权旋转插值

UE `Slerp_NotNormalized` 按源 XYZW 顺序累加点积，依据符号选择短弧，并在三角分支先计算 `1 / sin(Omega)` 后复用；`Slerp` 随后归一化。源四元数映射为 runtime `(-x,-z,-y,w)`，因此目标点积顺序是 XZYW。部分权重 TwoBoneIK 对三骨做插值，再用局部 B/C 位置重建全局位置；RigVM 加权 Transform 和 ALS 旋转分布也消费同一插值语义。

fresh 原生记录复用 `.476/.480` 中的 0.25 权重、七帧历史，分别采集 pole unit 后的输入、TwoBoneIK 后的输出，以及两个独立原生 IK 核。第 6 帧左右腿各三个输入和输出变换、两份独立核结果，均与 Git 保存的 `.480/negative-inputs-slerp.json` 逐值相同；`witness.json` 保存这些数据与源公式构造的未归一化控制。原生输出来自 UE，测试中未归一化控制是根据 UE 源算法和原生输入重算，不能称作单独的原生 primitive 输出。完整 native 请求、输出及无损压缩哈希均在本目录；`prepare_requests.py` 从受 Git 跟踪的原请求生成最小子集。

目标共享 `rig_slerp` 改为源顺序点积及复用倒数。新增 macOS ARM64 精确 fixture 比较插值结果和同一个 `rig_normalize` 对源公式中间量的处理，隔离尚未修复的 D248 归一化差异；另测短弧负号。`.480` 的完整消费者、source/core 隔离及删除插值因素失败记录继续作为历史证据。其他平台库函数及完整 Rig/Root 行为尚未验证。

复采使用 `tools/run_ue_source_reader.ps1`：两个 `weights-*.request.json` 分别作 `-RigNodeHistoryProbe`，`core.request.json` 作 `-GeometryQueryProbe`，资产和输出身份见 `native-identities.json`。本项 focused 测试、`.480` 全量消费者回归、消融与源码字节恢复、图静态审计及公共 Playground Gate 均待全部静态修复和 Bevy/依赖迁移后执行，目前尚未运行。
