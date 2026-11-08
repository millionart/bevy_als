# BF144-D130b2：胶囊/凸包源接触点

fresh 源算法按凸包面边平面裁剪胶囊轴线，并保留 X1→X2 顺序；圆柱分支沿垂直轴的法向投影两个表面点，端帽平面分支先生成符合距离条件的圆柱辅助点，再追加 GJK 点。源以 double 厘米计算，输出约束点时按源 float 精度保存；反射缩放改变裁线绕序。源码摘录见 `source-facts.json`。

fresh reader 读取 7 个 head/Arch shape pair，覆盖圆柱、端帽、抬高、分离、倾斜、非均匀 scale 和镜像；各自点数与 phi 见 `native-summary.json`。请求、原生报告、精简样本和哈希见 `native-identities.json`，精简样本可通过 `tools/import/export_convex_classification_probe.py` 从压缩报告重建。

已实现源同空间 GJK、面分类、严格裁线、两种投影及 GJK 回退组合，编写七组原生局部点/法向/phi/点序和裁线边界测试。当前函数尚未接入真实碰撞；reader 的直接 GJK 与轴字段、CVar 重采、编译、测试、消融、全量转换 diff 和 Playground 均待后续统一 Gate。
