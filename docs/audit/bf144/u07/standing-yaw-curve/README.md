# D321 RotationYawOffset 键顺序

UE `FRichCurve::Eval` 以原生时间相邻的两键做 F32 差值插值。已安装设置以前同时取反时间和值，导致相同输入从另一端做浮点求值。保留源方向的目标设置，在运行时先转换输入角，采样完成再转换输出符号。两条步幅曲线和半衰期经重生成保持数值相等。

三条偏航曲线以 D320 的同一原生角度请求重读。15 组输入涵盖五个活动帧的两端速度、正负微小角、零和两半转，45 个源曲线值均经实际 Rust 入口比较。原生输入和哈希在本目录及同级 `first-motion-yaw`。`key-direction-witness.py` 在相同输入下仅交换先后端点，原生前向结果 0.0043792724609375°，旧结果 0.004378151148557663°。还原旧设置文件使实际Rust左右读数断言失败。恢复同 target 重编通过。

从项目根运行 `python tools/import/gen_standing_curves.py --uasset <AIS_Als_Default.uasset> --content-root <ALS Content> --output <settings.json> --report <report.json>` 可重生成。合并生成器调用原生读取提前退出的报告仍保存，最终由相同转换器直接消费本轮六份原生报告，结果见 `generation.json`。

15 个 AnimBP 根及递归依赖重新转换后，374 个图产物逐字相同。`corpus-diff.json` 和空 `full_diff.patch` 保存全量比较。设置文件的三条偏航曲线单独重新生成。

正式 Standing Direction 和 319 帧原路线均通过功能验证。首五个实际偏航输入等于同输入原生值，第 21 个活动帧最终偏航精确恢复。79 个冷帧仍逐值匹配，活动段还有 FeetCrossing 和姿态余差。
