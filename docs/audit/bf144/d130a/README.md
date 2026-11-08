# BF144-D130a：凸包与胶囊轴线的同空间 GJK

fresh 源代码表明，胶囊/凸包使用凸包和轴线的无 margin 同空间 GJK；线段支持点在方向与轴点积为零时选择 X2。实现保留源 float 凸包支持、double 厘米 simplex、EPA 及末次顶点，不使用目标接触法向代替源查询。源片段和哈希见 `source-facts.json`。

两次 fresh 原生请求分别记录圆柱、端帽、上移及横向分离：首次上移 40 cm 仍与高凸包接触，第二次横移 1000 cm 才无接触。原始请求、完整报告、watchdog 和哈希见 `native-identities.json`。端帽末点可按源 radius 反算 GJK 见证点，已写与该原生点及线段平局对照的 Rust 测试。

现有已构建 reader 不包含新字段，故这些报告没有直接 GJK、轴或高度读数。源码已增加可选 `capture_capsule_gjk`、轴和高度；直接探针目前只接受单位凸包缩放，已写输出合同 `tools/tests/test_physics_capsule_gjk_probe.py`。统一 Gate 须重建 reader、重采 `gjk-v2.request.json`，运行该合同，并以 `ALS_CAPSULE_GJK_NATIVE_REPORT` 将新报告接入 Rust 测试。新实现尚未接入接触生成；所有编译、测试、消融、全量转换 diff 和 Playground 均未执行。
