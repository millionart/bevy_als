# D155 脚部组件空间位置使用源 QST

`TransformVectorized.h:363–373,1404–1453,1675–1693` 先构造逆旋转、逆缩放和逆平移，再执行逆 Transform 的 TransformPosition。点先按轴乘倒数，再逆旋转，最后加逆平移。全部尺度分量都在零阈值内时返回 Identity。

`AlsAnimationInstance.cpp:1307–1313,1432,1512–1523` 的当前锁定位置与最终 Rig 位置共用这个逆变换。`1538–1545` 用正向 QST 将受限的局部锁点写回世界空间。运行时沿用 `RigTransform` 的源四元数旋转及安全缩放倒数。原生 [AB_Als](../d149/ab-als.native.json) 的 Control Rig 使用 GetControlRigInput。

对相同世界点复算，单位尺度、均匀 1.5 倍和零 yaw 时均通过。尺度 (1.4, 0.8, 0.6)、yaw 0.7 时，以矩阵逆代入造成 0.170685 m 的组件空间误差。

```sh
python docs/audit/bf144/d155/replay_component_inverse.py
```

`foot_positions_reach_the_rig_in_source_qst_order` 关闭脚锁来隔离变换，读取实际 CR_Als 的左脚 RigInput。Rust、消融恢复和 Playground Gate 待统一验证。
