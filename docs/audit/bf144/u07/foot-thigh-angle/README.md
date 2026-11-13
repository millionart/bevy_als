# BF144-D319 大腿足锁位置限位

ALS `RefreshFootLock` 把世界位置变换到组件空间后保存为 `FVector3f` 厘米。`ConstrainFootLock` 以 `UAlsVector::AngleBetweenSignedXY` 分别归一化大腿方向和足位置，水平分量的平方和须严格大于 `1e-8 cm²`。叉积为零时符号为零。超过角度限制才旋转组件位置并回写世界坐标与基座缓存。

`centimeter-boundary.request.json` 是九组大腿控制与三组独立足扭转原生请求。大腿控制涵盖正反共线、近共线、两侧越界和阈值两侧。厘米阈值输入原生为 `9.99999993922529e-9 cm²`，等于比较阈值。相同 double 世界位置提前投成 F32 米时平方和为 `1.0000001044244144e-12 m²`，恢复厘米后仍越过阈值。可用 `tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/SKM_Als.SKM_Als -GeometryQueryProbe <request> -Output <output>` 复跑原生对照。请求和报告按名称配对，压缩报告解压字节和源码身份见 `native.identity.json`。

目标使用 `cargo test --locked --profile play --lib --jobs 2 thigh` 运行三层定向测试。同一 target 的消融仅把生产组件位置提前投为 F32 米，再交回厘米核。该单因令边界用例改写锁点并稳定失败，原始断言见 `early-meter-red.log.gz`。恢复后以同一 target 重编并复验，结果见 `centimeter-restored.log.gz`。Control Rig 和旋转基座均经现有正式入口执行功能校验，采图数为 0。
