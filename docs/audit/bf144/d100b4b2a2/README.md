# D100b4b2a2 胶囊三角面筛选与端帽

源 `ConstructCapsuleTriangleOneShotManifold2` 先用 float 厘米胶囊几何，再用 double 厘米计算逐面接触。三角面法线平方长度须大于 `1e-10`。两端都超过 `radius + cull` 或中点在背面超过 `1e-5f * height` 时剔除。边按 2→0、0→1、1→2 检查。最近点位于三角面下侧时，外侧边的分离范围只取半径。通过筛选后依次尝试 P0 和 P1，要求端点位于全部边平面内、在半径加 cull 以内且距离不超过另一端加源容差。保留原始点和逐点法线，供后续平行边、圆柱及最终边接触阶段消费。

`triangles.request.json` 由 `prepare_request.py` 生成，使用 Rock、Stairs01 的生产接触姿态和 Cart、Crate_Broken 的前后侧样本。现有 UE 读取二进制直接调用源函数，原始读数以 `triangles.retry.native.json.gz` 保存。首次启动停在 Zen 服务，见 `triangles.native.json.reader.report.json`。系统盘空间恢复后使用默认系统临时目录重采，六组样本在 35.488 秒内完成，见 `triangles.retry.native.json.reader.report.json`。`extract_triangle_cases.py` 从压缩原始读数生成 `triangle-cases.json`，固定单位缩放和恒等旋转并把三角顶点移到胶囊局部坐标。Stairs01 面 252 的两个端帽依次对应 P0 和 P1。Cart 背面无点，Crate_Broken 的 175 和 176 随朝向交换有效面。旧二进制只记录端点与半径，当前对照夹具从端点重建轴和长度。

重采使用仓库 `tools/run_ue_source_reader.ps1`，参数为 `-Asset /ALS/ALS/Character/PA_Als.PA_Als`、`-PhysicsContactProbe` 指向该请求、`-Plugin target/ue_graph_source_reader_package/AlsGraphSourceReader.uplugin`、`-TimeoutSeconds 420` 和 `-NoProgressSeconds 90`。编译、focused 对照、消融和真实入口 Gate 待静态批次与依赖升级完成后统一执行。
