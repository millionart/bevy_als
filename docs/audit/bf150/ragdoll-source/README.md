# BF150.01 普通时钟落地布娃娃的骨盆步长

BF150 正式可见 `ragdoll` 场景在 31.77 ms 渲染帧记录到 0.374535 m 的骨盆局部平移。取实际动画步长 29.539665 ms，加载角色高落地夹具复现 0.370490 m 的骨盆局部步长及约 0.691 m 的可见世界步长。该读数来自物理回读后已传播的骨骼 `Transform`。[原普通时钟报告](../ragdoll/report.json.gz)

本地 ALS 在 `AlsCharacter.cpp` 的 Air→Grounded 回调中启动 Ragdoll。`AlsCharacter_Actions.cpp` 在开启模拟前以 KeepWorld 分离 mesh。原生高落地的相邻帧保持分离前已发布的 mesh 世界变换。源运动学 root 与 mesh 同位置，启动后速度为零，动态 pelvis 的可见骨骼位置与实际身体一致。29.539665 ms 对照的局部步长约 0.190793 m，世界步长约 0.381585 m。[原生帧与身份](native-entry.json) [运动学根骨](native-kinematic-root.json)

目标原先用已经落地的胶囊重新计算 mesh，首帧把它从约 0.317 m 移到 −0.018 m，再交给刚体。保持发布的 mesh 后，骨盆世界步长恢复约 0.380 m。同一帧的运动学 root 仍读取旧挂载空间下准备的速度，刚体与可见根骨从约 0.317 m 移到 0.702 m，造成第二处局部误差。修复沿用本帧已有的 Air→Grounded→None 消息选择发布的 mesh，并在现有身体交接处清除首次仍保持运动学的速度。动态身体继续继承启动速度。视觉护栏和物理回读算法沿用原实现。

原生复跑可在本任务起点 `6e053b31` 应用 `reader-observer.patch.gz` 或 `kinematic-reader.patch.gz` 中的对应临时观察补丁，将 `drop.request.json` 或 `kinematic-root.request.json` 交给 `tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/B_Als_Character.B_Als_Character -CharacterAnimationProbe <请求> -Output <报告>`。原生工具源码已按字节恢复。

把实际加载角色设为同样的 10.82 m 高度、两倍体型和 29.539665 ms 步长后，目标骨盆首次局部步长为 0.190117 m，世界步长为 0.380235 m，对照 UE 的 0.190793 m 和 0.381585 m。夹具检查首三个模拟帧的 mesh 与 root 保持、运动学 root 速度、已传播骨盆与物理体位置和父空间缩放。分别撤掉两处修复后，分别在“保持已发布 mesh”和“保持运动学 root”上失败。 [消融与恢复身份](ablation.json)

同 target 恢复后，72 项关联定向测试通过。正式 Playground 的无驱动启动、普通时钟布娃娃、29.539665 ms 布娃娃及普通时钟起身均通过功能验收。两个布娃娃场景各覆盖 13 项，起身覆盖 14 项，图错误为 0，最终骨盆跳变校验通过。性能超预算项随报告保留。 [正式二进制](build-identity.json) [旧新报告](runtime.json)

恢复回归使用 `CARGO_TARGET_DIR=target_game` 执行下列命令。Playground 取证参数见各场景的 `runner.json`。解压 `report.json.gz` 后，可用 `tools/validate_animation_visual_evidence.py --report <文件> --min-screenshots 0 --check --functional-only` 复核。

```sh
cargo test --locked --offline --jobs 2 --profile play \
  --config profile.play.package.bevy_als.opt-level=0 --lib -- --test-threads=1 \
  ragdoll_link::tests post_physics_animation::tests get_up::tests \
  high_landing_pre_rig_hands_keep_the_source_stage_space
```
