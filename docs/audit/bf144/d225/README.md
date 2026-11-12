# D225 Raw 序列旋转插值

生成器读取128个实际 clip 描述符、glTF 动画名和原 accessor。109个 LINEAR 动画各自共享一套旋转键时刻，19个 STEP 继续按离散键采样。运行时复用现有 VariableCurve 读出两端键，按 UE `FTransform::Blend` 的阈值和 `VectorLerpQuat` 的最短弧归一化线性插值求值。

Sprint Acceleration 的 `ik_hand_l` 在 `0.3595596254` 秒的 raw_model 原生对照中，旧 slerp 的最大分量差为0.00105766，恢复 nlerp 后为5.81e−8。两端、半帧和反号四元数也在回放中检查。该对照使用现有 glTF 键，原 Sequencer 的 Euler 往返和帧时间由后续条目处理。

```sh
python tools/import/compile_als_raw_rotation_timelines.py --output assets/ALSRuntime/Animations/als_raw_rotation_timelines.json
python docs/audit/bf144/d225/replay_raw_rotation.py
```

Rust 回归从正常 `sample_pose` 入口检查原 Sprint 键和无绑定时的对照，随统一 Cargo 验证执行。
