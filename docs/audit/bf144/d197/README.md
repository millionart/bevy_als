# D197 标准 Profile 双权重

UE `BlendProfile.cpp:456–485` 计算独立的正反向骨骼权重。`AnimationAsset.cpp:783–850` 只在总和超出 1e−5 容差时逐项除以总和。`AnimNode_StateMachine.cpp:1092–1105` 将两份权重分别用于平移、scale 和最短符号旋转累加。

fresh `SK_Als` 的 MoveDirectionChange 与 QuickFeetBlend 共十条轨迹覆盖容差内外、恰为 1 和缺省骨骼。用目标权重的补数替代源权重会让 72 个骨骼的非零平移或 scale 失配。目标合成保留这两个原生权重，Rust fixture 核对权重、原始及归一化旋转、平移与 scale，另有稀疏通道和 morph 权重控制。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/SK_Als -BlendProfileProbe docs/audit/bf144/d197/profiles.request.json -Output docs/audit/bf144/d197/profiles.native.json
python docs/audit/bf144/d197/replay_profile_weights.py
```
