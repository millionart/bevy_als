# D227 Raw FK 参考变换保留

源模型把 FK 骨初始化为 Skeleton 的参考值。Direct 写回比较平移、旋转和 scale，使用 `FRigComputedTransform::Equals` 的 0.0001f。比较发生在源厘米和原 Control 值域。跳过写回时，实际 SKM 的参考变换保留在 pose 中，再执行源旋转归一化。

从128个实际序列读取8660个普通骨 FK 通道。全部源模型与 MovieScene 时钟相同，整数帧读数与帧间端点读数也逐字节相同。生成包共用这套键。Raw 消费者用原 FK 通道求保留条件，沿现有骨骼身份写回实际网格参考值。原 glTF 键可用于其他采样阶段。

原生 Crouch Walk Forward 的五次完整对照检查了340个 Control 判断，29个保留后的网格参考投影与原生相等。0.5037120580673218秒的 `calf_twist_01_l` 在原 Control 值域应保留。改用已写回的 glTF 键会越过同一个门槛。各单项越界控制、回返时点与 reader 重编读回均通过。

```sh
python tools/import/compile_als_raw_controls.py --output assets/ALSRuntime/Animations/als_raw_fk_controls.bin
python docs/audit/bf144/d227/replay_reference_retention.py
```

Rust 回归以相同的实际 clip 与骨骼路径检查保留和超界，随统一 Cargo 验证执行。
