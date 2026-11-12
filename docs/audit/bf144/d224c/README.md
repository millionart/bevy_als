# D224c Prepared 曲线消费者

Sequence Player、Sequence Evaluator、BlendSpace 子样本及槽的序列生产端按实际 GraphClip 的 glTF 路径和动画名选择 D224b 数据。原生空曲线仍为空。外部、未绑定及普通验证夹具可继续使用其曲线表。GetUp 和 Mantle 的 Montage 曲线仍在各自序列曲线之后合并。

UE `AnimSequence.cpp:3243–3266` 以原播放时间解码曲线。BlendSpace 在 `BlendSpace.cpp:1279–1284` 转发子序列时间，Montage 段在 `AnimCompositeBase.cpp:535–554` 转发换算后的序列时间。骨骼 Step 不会把曲线时间移到离散关键帧。

复核了15套安装清单内345个图。213个 Sequence 节点与24个直接 BlendSpace 子样本找到 ACL 序列，23个经差分包装的子样本两侧曲线均为空。弓的唯一 PerTrack 序列没有曲线，姿态在 D224e 验收。

原生 Crouch Transition 在 `0.30000001192092896` 秒的 FootLeftLock 从raw的 `−9.536743306171047e−8` 变为prepared的 `−1.1920928955078125e−7`。同权重控制的槽输出从1变为0.9999999403953552。Run/Transition/Turn共53个原生曲线值与包重放相等，Turn 继续使用生成器写入的目标 yaw 符号。D224b 的原解码键字节保持。

```sh
python docs/audit/bf144/d224c/replay_curve_consumers.py
```

Rust 回归覆盖绑定、空曲线、原曲线回退、FootLock 与 yaw。随统一 Cargo 验证执行。
