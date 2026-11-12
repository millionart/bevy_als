# D232a BlendSpace 源数值空间

Prepared 播放器可以返回原坐标和厘米中的 double 骨骼值。BlendSpace 的已绑定子样本保留这些值直到源权重乘加、最短弧累积和 D231 归一化结束，再投影给 Bevy。合成结果同时提供给实际下游姿态上下文。其他节点传来的精确姿态需与其当前投影一致。

140个原生 WalkRun 采样的20540个子骨和11060个合成骨均按当前压缩数据和权重精确重放。提前投影使3424个平移、79个旋转和420个 scale 的最终 f32 值改变。双样本 pelvis 的目标 Y 为0.8939557075500488，提前投影为0.893955647945404。

```sh
python docs/audit/bf144/d232a/replay_source_blend.py
```

原生输入来自 D231 当前平台的真实 BlendSpace 读回。Rust fixture 使用相同 clip、时刻和权重，并检查姿态变更后旧精确值失效。Cargo 统一后置。
