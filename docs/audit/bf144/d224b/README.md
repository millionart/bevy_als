# D224b ACL 离线键

生成器读取 D224a 的 Character 平台压缩数据，用同一套本地 ACL/RTM SDK 取出骨骼的 default、constant、animated 型位及曲线的 constant、animated 型位。浮点键按源算法的分量次序保留。目标包记录单位投影系数，供消费者在采样后投影。Skeleton 与实际 SKM_Als 的参考姿态分别存放。

骨骼时钟取实际压缩率和有限时长。保留 62 个 wrap 标记和 127 个无 scale stream 标记。Rifle 的已剔帧 Step 使用离线 floor 重建的未归一化键，复用目标的均匀采样。合成 ACL 夹具中，将 floor 重建改为最近键读取造成 18 个差异，按相同输出路径恢复编译后读回字节完全一致。

UE 曲线的 `BuildSorted` 在名称和值上都应用 `CurveIndex`，流内名称因此仍按 `IndexedCurveNames` 的实际数组次序保存。Run Right Backward 的原生 24 个曲线值与目标包重放逐值相等，按排序索引解释流会错配 23 个值。Crouch 首帧 321 个写入分量与原生批量 codec 一致。

`als_prepared_acl.bin` 先保存 little-endian JSON 头长度和 UTF-8 JSON，再保存连续 little-endian f32 键。通道描述给出 f32 起始偏移与型位，按时钟的样本数解释 animated 型位。`summary.json` 记录整包 127 个序列的独立算术重放。提前归一化、封闭 wrap 和合并 root 插值算术均有反例。

```sh
python tools/import/compile_als_prepared_acl.py --ue-root "$UE_ROOT" --output assets/ALSRuntime/Animations/als_prepared_acl.bin --work target/d224b/final
python docs/audit/bf144/d224b/replay_prepared_acl.py
python docs/audit/bf144/d224b/replay_stripped_step.py --ue-root "$UE_ROOT" --work target/d224b/step-ablation
```

再生成时核对 `compile-report.json` 的产物 SHA-256，以及 `acl-probes.native.sha256` 的原生 SDK 探针 SHA-256。记录来自 MacEditor arm64 的 ACL/RTM 浮点合同。
