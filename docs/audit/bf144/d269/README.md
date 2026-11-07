# BF144-D269 移动方向的源 float SinCos

`source-facts.json` 保存 fresh UE `SinCos(float)` 范围折叠和多项式、ALS 方向调用、目标既有 `rig_sin_cos` 与移动输入入口，并引用 `.442b` 的317原生样本和 D270 同轮45°原生读取及身份。两份实现的常量、分支与算子顺序一致。

世界移动输入直接复用既有源式 float 近似，不改变相对二维旋转或其他三角调用。已写45°精确原生 cos/sin 对照；旧方向测试只以宽容差比较单位向量。全部静态修复及 Bevy 迁移后运行 focused 测试、消融和原 Playground Gate；改回系统 `sin_cos` 时该反例应失败，恢复源码字节并同 target 重编后复验。以上测试和消融尚未运行。
