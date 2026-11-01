# BF144.443 / .443a：当前物理阶段证据

父项 .443 和竖直子项 .443b 保持未完成。本目录的 `summary-a.json` 只证明 .443a 的当前水平阶段与匹配世界输入的 source 历史。

使用 .442 原 Playground/main/240驱动，新增 after-motors 和 Last 物理后 position/velocity、真实 frame/physics/substep 纳秒、N及COM。新字段扣除后，旧 .442 的240 Update记录精确保持。首次schema假定全部position在motor阶段不变被89个Y修正反驳；XZ全部保持，保留失败事实，竖直写者/floor语义转入 .443b。

实际 frame=33333335ns、physics=33333333ns、substep=5555555ns，N=6、COM/support为0。source_physics_timing按Duration而非两个接近的f32 dt选择full-frame，motion step将计划位移除帧长写motor，末尾按world位置差回算。按实际motor×frame_dt/N累计局部delta、一次world加法、末尾差分，240份XZ位置与速度全部精确；70份motor与raw计划不同，全部按delta/dt和boost减/加顺序精确，扩D264当前consumer。

新source真实240 Walking正常tick逐帧接收同一actual target世界向量，gravity=-981cm/s²。输入直接经现有AddMovementInput诊断接口，不宣称执行Enhanced Input方向事件；Source/target初始世界原点、Y与后续各自产参数不同，不能把两条整轨迹直接当同态算法差。首移动帧 source位置米double约0.022222224475、F32投影0.022222224623；同motor零原点单步同值，六次局部delta约0.022222222760，归D253；实际−15原点位置量化进入0.666675508m/s回算，归D254。世界原点的舍入会遮蔽部分子步信号，不声称每个因素在每帧都有独立最终差。

消融将正确的一次world加法改成每子步加world，实际矩阵拒绝；删除motor观察注册及Last position/velocity字段，阶段合同拒绝。`*.final` 保存本项恢复源码，SHA验证后同target重编；恢复后的240完整阶段记录精确、整个独立replay JSON byte精确。公共功能Gate分别验证baseline/restored，性能失败原样暂停。

`source.native.json.gz` 是完整原生报告的无损封存，`source-compression.json` 记录raw/gzip SHA与长度；`source-phases.json` 是逐字段精确投影，完整动画/曲线数据仍在gzip中。`archive_source.py` 可从新原始报告重建两者，原始uasset不改。所有长期证据Git跟踪，构建/日志仅是可重建临时数据。

```sh
python3 docs/audit/bf144/443/check_stages.py self-test
pwsh -NoProfile -File docs/audit/bf144/443/run_matrix.ps1 -Label replay-new-build -Build
pwsh -NoProfile -File docs/audit/bf144/443/run_matrix.ps1 -Label replay-new
pwsh -NoProfile -File docs/audit/bf144/443/run_python.ps1 -Name finalize_a
```

完整source函数/实际依赖源码摘录见 `source-facts.json`。runtime使用300秒总/90秒无参数进展上限，其余工具复用已有watchdog；四个阶段负fixture拒绝缺数据/错索引/NaN。本项不修生产、不扩周边，未验证完整竖直floor、player clock、非零view/attributes或完整Root。截图0，性能验收暂停。
