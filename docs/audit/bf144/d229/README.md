# D229 Raw FK 曲线写回历史

源 `UAnimationSequencerDataModel` 在 `EvaluationLock` 内复用 FK Rig，每次求值仅重置骨骼。新建曲线的 `bIsValueSet` 为 false，首次写入即保存。随后 `URigHierarchy::SetCurveValue` 用 float 差值比较 1e−8，保留上次成功写入的值。

目标按已加载的 raw 源动画身份保留曲线状态。同一源的多个 GraphClip 引用共享写回，prepared 读取走独立的 codec 值。

原生冷启动与回返共16个曲线值逐值重放。FeetCrossing 同一 7/30 秒在两段历史下分别返回 −4.3213368883243675e−8 和 −3.848565199859877e−8。去掉历史以及把未设置值预置为0，各留一个原生反例。

```sh
python docs/audit/bf144/d229/replay_curve_history.py
```

Rust fixture 使用两个引用和一次 prepared 读取，随统一 Cargo 验证执行。
