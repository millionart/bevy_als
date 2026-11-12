# D224d Prepared ACL 姿态消费者

目标包绑定实际 SKM_Als 场景的79个关节路径。普通姿态从网格参考初始化，编码器有轨道时使用其骨架默认值或压缩键。无 scale 流时保留初始化的 scale。局部和网格加性使用压缩器已烘焙的 delta，缺省 scale 为零。Sequence 和 BlendSpace 复用这一路径，23个原差分包装的样本按实际加性类型消费烘焙值。

`FAnimSequenceDecompressionContext::Seek` 先经 `FrameRate.h:232–257` 与 `FrameTime.h:265–271` 往返规范化子帧，再以 float 秒进入 ACL 骨骼时钟。Crouch 和 Run 的64个骨骼反例由此恢复。曲线仍以原时间进入各自的 codec。

重新提取127个序列确认16个 RefPose 强制根锁。完整姿态复用根锁，Roll 和 Mantle 的提取轨道读取同一压缩数据的未锁根。两个资产各4个根时点与 UE 逐值相等。保持 glTF 输入的对照在 Roll 和 Mantle 分别留下约1.38e−5 m和1.31e−5 m位置差。

44个原生姿态中3380个骨骼逐值相等。其余96个骨骼全是三根 AnimationRelative IK 骨，21个在投影后仍有差值，由 D230 验收。三类加性样本共948个骨骼逐值相等。读回、消融数量和图绑定见 `summary.json`。

```sh
python tools/import/compile_als_prepared_acl.py --ue-root "$UE_ROOT" --output assets/ALSRuntime/Animations/als_prepared_acl.bin --work target/d224d/final
python docs/audit/bf144/d224d/replay_pose_consumers.py
python docs/audit/bf144/d224c/replay_curve_consumers.py
```

原 C++ SDK 探针字节与 D224b 一致。Rust fixture 覆盖实际网格绑定、烘焙加性和平移轨与根锁的独立消费，随统一 Cargo 验证执行。
