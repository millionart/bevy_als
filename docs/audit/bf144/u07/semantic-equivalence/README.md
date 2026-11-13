# BF144-U07 清理与源语义复验

撤除 View 双份朝向、GroundPrediction 默认胶囊尺寸和命中比例适配、默认网格挂载原点与旋转补偿。撤回尚未提交的步幅、速度上限及精确值缓存。Feet 和 Control Rig 使用实际动画网格的当前变换，仍使用起身恢复后实际挂回的网格。源依据为 `UAlsAnimationInstance::NativeUpdateAnimation`、`RefreshFeetOnGameThread`、`RefreshFeet` 和 `AAlsCharacter::StopRagdollingImplementation`。保留同一挂载的独立变换断言及已验证的起身地面支撑回归。

## 实际入口

[原88步加初始化的比较](final/replay/comparison.json) 逐帧核对 30 Hz 角色步长、输入方向、Grounded/Air、图更新与三处姿态读点。第1步进入 Air，第11步落地。前16步核对实际 Locomotion 与 Grounded 状态、转场前后态和时钟，别名成员来自本地原生图。三处各有 7,031 行骨骼数据，源自定义动画属性均为空。全部曲线键一致，21个标量差未改变已核对的源停止选脚、IK 与锁脚相关性、转场许可、Sprint 混合子分支及默认脚步音量相关性。最大世界位置差 3.05 µm，最大 Root 局部平移差 2.40 µm，最大局部旋转差 0.00438°。这是本次输入历史的实测结果。

- **GroundPrediction**。`RefreshGroundPrediction` 的落速资格、夹取、扫掠、可走支撑与曲线消费保持。第7至10步同输入控制分出厘米／米的 F32 舍入与落体积分输入差。最大 Root 曲线差为 2.39×10⁻⁷。第7至10步双脚 IK 夹取后均为1。 [原生查询及同输入比较](../prediction-input/comparison.json)
- **Standing 步幅**。原生 `CalcVelocity` 在厘米 F32 把 150 × 0.8 的上限取为120。目标以米 F32 计算，同一输入投影回厘米为 120.0000076294。源步幅曲线在两值上分别给出 0.9720751643 和 0.9720752239。实际滤波输出、原生两条压缩序列及站姿混合权重可重放 Root 的 `FootPlanted` 余差。已读取的前向主样本时钟一致。 [步幅对照](standing-stride-control.json)
- **落地趾骨**。第11步两边都在 0.0333333351 秒采 `A_Als_Land_Light`，两个趾骨的叶旋转逐值相同。把源两帧历史与该叶交给实际 Locomotion 原生惯性节点，可逐值复现源趾骨。仅把输入量化到 F32 就得到 0.00374° 差。喂入实际目标 Locomotion 历史后，原生输出投影为 F32 与目标惯性节点逐值相同。该差在进入 Rig 前已经出现。 [惯性化对照](landing-rotation.json)
- **View**。同一个目标 F32 弧度表示可以对应两个不同的源微角。原路线中，世界方向分量的差不超过 3.70×10⁻⁸，60个活动输入仍选择 Forward。 [原生同角及反例](../view-yaw-origin/README.md)

真实 `ragdoll`（含两倍体型）、`get_up`（正反面）、`locomotion` 及 `control_rig` 使用移除临时观察后的正常入口，公共功能校验通过。`head` 的俯仰输入及 `standing_turn` 的左右 180° 仍由已封存的专项报告覆盖。 [最终功能报告](final) [Head 与转向证据](../head-prepared-observation) [两倍体型原生对照](../scaled-get-up)

锁定 Bevy 0.20.1。清理后 2,149 项单元测试和 37 项集成测试全部通过，`cargo check --locked --profile play --all-targets --jobs 2` 通过。截图为0。功能报告记录了超出性能预算的指标，性能验收保持暂停。 [测试日志](final/tests) [编译日志](final/check)

## 复验

从仓库根运行 `python docs/audit/bf144/u07/semantic-equivalence/compare.py docs/audit/bf144/u07/semantic-equivalence/final/replay/root-history.json.gz`。实时采样沿用 [原88步启动脚本](../crouch-warm-120-latest/replay.ps1) 和 [原入口观察补丁](../crouch-warm-120/observer-hook.patch.gz)。先把本目录的 `standing-stride-observer.rs` 放到该补丁引用的位置，采样后恢复入口字节。公共场景使用 `tools/run_animation_visual_validation.ps1 -Case <ragdoll|get_up|locomotion> -CaptureCount 0 -FunctionalOnly`。

原生 `.request.json` 可配合 `tools/run_ue_source_reader.ps1` 使用。以同名原生报告的 `asset_path` 作为 `-Asset`。落地和步行序列使用 `-SequencePoseProbe`，落地惯性化使用 `-InertializationProbe`，其余依次为 `-GeometryQueryProbe`（步幅曲线）、`-BlendSpaceProbe`（步幅混合）、`-CharacterVelocityProbe`（模拟输入限速）。`identity.json` 记录归档字节和 SHA-256。
