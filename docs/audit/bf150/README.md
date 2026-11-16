# BF150 场景查询与骨骼物理的源码归属

重新读取了 `CR_Als` 和 `PA_Als`。Rig 的 Construction 和 Forwards Solve 入口、45 处单位调用与现有程序一致。PhysicsAsset 有 21 个刚体、20 个约束、49 个碰撞排除对。原始报告、请求、watchdog 和哈希见 [native-contract.json](native-contract.json)。

源足探测通过 `UWorld::LineTraceSingleByChannel` 取命中。角色层持有 Rig 查询上下文。Avian 几何查询复用现有 D246a/b 的精度与端点路径、D162a/b 的 UE 烘焙网格路径及 BF144.43 的胶囊场景扫掠。`SceneQuery` 继续通过已有碰撞模式区分查询与物理资格。摄像机对多个穿透调整的合并归相机层。Mesh 容器及次序证据见 [D162a](../bf144/d162a/README.md) 和 [D162b](../bf144/d162b/README.md)。

`animation_graph_ext` 管理 PhysicsAsset 的骨骼刚体装配、交接、动画关节目标及按骨骼配置接入的系统。Avian 侧持有 BF144.23–.27 验收过的关节、刚体睡眠、约束轴、惯量和积分缺口。系统注册位置和顺序保持原样。测试继续通过真实骨骼体及已有几何查询入口核对。

运行 `python3 docs/audit/bf150/verify_relocation.py` 可逐项比对搬移前后的 72 个函数体及 71 个用例名。对照只允许符号、导入位置、可见性和格式差异，另核对角色物理时钟的调度注册以及 Control Rig 扩展源码字节。受控移除一条旋转积分调度边时，对照按预期拒绝。当前工作树通过，资产与依赖锁文件一致。 [搬移对照](relocation.json) [调度负例](relocation-control.json)

同一 `target_game` 的 283 项定向 Rust 测试通过。正常启动、Control Rig、Foot IK、起身和相机以普通时钟通过正式 Playground 功能验收。布娃娃普通时钟下完成倒下、碰撞、恢复和输入交接，视觉护栏在 31.8 ms 帧记录骨盆局部位移 0.374535 m。旧 U07 的同类异常为 31.6 ms 和 0.390895 m。使用旧门禁相同的固定 60 Hz 参数复验，布娃娃通过。所有场景的图错误为 0，性能超预算项随原报告保留。 [二进制身份](build-identity.json) [运行结果](runtime.json) [布娃娃时钟对照](ragdoll-clock-comparison.json)

定向回归使用下列命令。各场景的 launcher 参数见 `runner.json`。解压 `report.json.gz` 后，可用 `tools/validate_animation_visual_evidence.py --report <文件> --min-screenshots 0 --check --functional-only` 核对原报告。过程日志已移除本机路径。

```sh
CARGO_TARGET_DIR=target_game cargo test --locked --offline --jobs 2 --profile play \
  --config profile.play.package.bevy_als.opt-level=0 --lib -- --test-threads=1 \
  control_rig cooked_mesh_raycast::tests physics_asset::tests scene_query::tests \
  source_body_dynamics::tests source_capsule_sweep::tests ragdoll_link::tests \
  get_up::tests foot_ik als_camera::tests source_physics_timing::tests
```
