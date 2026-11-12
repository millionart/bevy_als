# D228 Sequencer 整数帧与子帧

Raw 骨骼用 D227 的原 FK 键在源帧域求值。普通写回投影当前 Control，近参考写回保留实际网格参考。两条路径均保留原生归一化顺序。源 ARM 的四元数算子核验了544个普通骨投影。

从128个序列的活动 MovieScene 读取367条曲线。生成数据保留整数键时刻、曲线自身 tick rate 和 double 段系数。运行时按源 `FFrameTime` 规范化子帧，线性按帧差计算，无权重 Cubic 复用源 Bezier 系数与 float 插值比率。预备压缩曲线继续使用 codec 秒域。

43个原生曲线值全部精确。FeetCrossing 在0.5302536487579346秒得到0.9076095819473267，原 glTF 秒域结果为0.9076088070869446。去掉高 tick 换算或对标量应用骨骼 Step，各留下15个反例。改用线性求值替代实际 Cubic 留5个反例。

```sh
python tools/import/compile_als_raw_curves.py --output assets/ALSRuntime/Animations/als_raw_sequencer_curves.json
python docs/audit/bf144/d228/replay_source_frame_time.py
```

ARM 探针通过同一个 watchdog 重建。Rust 回归留到统一 Cargo 验证，raw 写回历史由 D229 验收。
