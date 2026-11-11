# D163b Rig 的源 double 四元数乘法

`UnrealMathNeon.h:2328–2335` 与 `UnrealMathSSE.h:2728–2735` 在源基按 W→X→Y→Z 累加。`DQuat` 在同一基中的标量实现符合该顺序。`AlsRigUnit_ApplyFootOffsetRotation.cpp:35–97`、`ControlRigMathLibrary.cpp:35–84` 和 Rig 层级的 `TTransform` 组合及相对变换使用这个合同。足旋转保留当前、目标和写回三条表达式各自的括号。

用实际 Rig 记录选取六组初始相对、当前小腿空间和普通正向姿态的精确输入，另用参考旋转构造半转。19 组源 double 乘积逐值复现原生报告。去掉源基转换后有 16 组失配，受控 Roll 从 −180° 变为 +180°，令同一个 −177.4759° 目标被裁为 0°。Rust fixture 覆盖原生乘积、Rig 层级组合/相对变换及真实足旋转回调。源重放结果和 Rust 运行状态随清单记录。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALSExtras/Environment/Meshes/SM_Als_Stairs_01 -GeometryQueryProbe docs/audit/bf144/d163b/products.request.json -Output docs/audit/bf144/d163b/products.native.json
python docs/audit/bf144/d163b/replay_products.py
```
