# D226 Raw Sequencer 的 Euler 往返

当前128个安装序列的活动模型都是 `AnimationSequencerDataModel`。原生模型在相邻键经 `FTransform::Blend` 后执行 `FEulerTransform` 转换，将角度写入float通道，再重建四元数。精确键直接使用其已导出的姿态，非零子帧的 Blend 端点也执行源 Euler 写回。

目标复用 Control Rig 的源坐标旋转工具。Crouch Walk Left Forward 的 `ik_foot_r` 在0.7756271362304688秒跨越负向奇异阈值，重放的分量差从0.000934238变为约1.00e−8。已比较五个同输入 raw/prepared 原生时点，包含精确键、近端点和普通分支。Prepared 数据继续走原压缩采样路径。

```sh
python docs/audit/bf144/d226/replay_raw_euler.py
```

原始平台流比对、模型清单及原生原件随本项保存。Rust 回归留到统一 Cargo 验证。
