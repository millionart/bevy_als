# D259–D262 足锁扭转回归

本地 ALS 以实际网格插槽的世界四元数作目标，先在世界双精度相乘，再把相对四元数投成单精度给 `GetTwistAngle`。三个原生半转输入在固定轴和双精度投影条件下均返回 −180°。原始请求、报告及 SHA256 同目录保存，可通过项目 `run_ue_source_reader.ps1 -GeometryQueryProbe` 重读。

旧已加载角色测试使用仿射分解四元数来注入半转。实际插槽读取使用正缩放父链，测试输入与实际目标在半转分支处因此不一致。夹具改用插槽读取结果并以双精度弦长检查既有的 `1e-4 rad` 旋转误差界，保留 0°、90°、两个四元数符号的半转和接近零扭转的水平轴控制。

从项目根运行 `cargo test --locked --profile play --lib --jobs 2 source_foot_twist_reaches_the_actual_rig_rotation`。旧优化测试输出在同一 U07 目录的 `bevy-0.20/tests-round-13-lib/optimized.stdout.log.gz`，恢复输出见 `loaded-rig.log.gz`。
