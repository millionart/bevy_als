# D195 惯性旋转的原始差分

UE `AnimNode_Inertialization.cpp:905–924` 直接对 double 四元数乘积求轴角。`Quat.h:1174–1218` 从 W 求角度，向量长度不足时用源 X 轴。`UnrealMath.cpp:816–821` 在 double 投影和 unwind 后将上一份 twist 落为 float。目标使用同一源基的乘积与共轭，回退轴映射到 −X，输出姿态统一归一化一次。

fresh 主 AB 八个求值采样包括近单位正负符号、微小向量、单位、普通旋转、短弧 twist 和连续请求。以原生上一帧转换为目标 float 后，投影与原生四元数的最大分量距离约 1.61e−6。恢复差分归一化会使三个直接近单位控制至少偏 3.13e−4，连续请求也失配。Rust fixture 核对差分到骨骼写回。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AB_Als -InertializationProbe docs/audit/bf144/d195/inertia.request.json -Output docs/audit/bf144/d195/inertia.native.json
python docs/audit/bf144/d195/replay_rotation_difference.py
```
