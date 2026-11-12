# D224e Bow PerTrack 播放

原生 reader 以 `FUECompressedAnimData` 读回48个平移/旋转偏移、24个 scale 偏移和1356个键字节。保留24骨及其实际目标路径。原流为3个 identity、48个 Fixed48NoW 和21个 Float96NoW 通道，8个旋转轨自带逐轨帧表。

目标 Sequence Evaluator 按原 glTF 资产和动画名绑定此包，使用 `FFrameTime` 的归一化子帧、轨道帧表与每轨键数采样。PerTrack 旋转使用 `FQuat4f::FastLerp` 后归一化，源值投影到目标坐标。原生曲线集为空。

63个原生时点包含全部30 Hz端点、半帧与额外 draw 输入。1512个骨骼投影逐值精确。去掉帧表或旋转归一化分别留下486和1171个反例。两次读取的原压缩流一致，全部24骨使用 Animation retarget mode。

```sh
python tools/import/compile_als_per_track.py --output assets/ALSRuntime/OverlayObjects/Bow/als_prepared_per_track.json
python docs/audit/bf144/d224e/replay_bow.py
```

实际 Bow 动画图及 0.505 draw 输入的 Rust 回归留到统一 Cargo 验证。
