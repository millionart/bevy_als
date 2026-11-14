# BF116.01c2a 相机首次求值

Quick Stop 的并行失败发生在加载夹具中的相机。相机场景和配置由不同的句柄加载。原实现从 `AnimatedSceneInstance` 找到播放器后，等配置可用才插入 `ManualAnimationPlayer`。播放器先到时，`PrePhysics` 通用求值器接收了空输入集合，生成图的动作混合节点报告 `GlobalInputDataMissing("locomotion_action")`。

受控启动中，配置延后 3/3 次触发原错误。配置提前 3/3 次由相机系统写入已存在的空动作标签，并产出姿态。新增夹具保留真实 `ALSCamera/AB_Als_Camera.static.animscn.ron` 及 PlayableCharacter 调度。它先把所有者关联到已保留但未填充的配置句柄，等场景、图依赖及真实配置加载完成，再恢复真实句柄。等待阶段播放器属于相机，且没有通用求值、图错误或输出姿态。配置就绪首帧检查动作输入和输出姿态。

[历史并行失败](../../bf146/06/parallel-quick-stop.json) · [两种加载顺序](controlled-reproduction.json) · [原实现的回归失败](red.json)

原生组件 CDO 的 `AnimationMode` 为 `AnimationBlueprint`，`AnimClass` 指向 `AB_Als_Camera`，`Settings` 指向 `CS_Als_Default`。源动画实例初始化取得自己的相机和角色，更新时从角色读取动作。原生图的 `LocomotionAction` 初始值为空 Gameplay Tag。目标相机在发现播放器时即登记求值归属，配置就绪后仍由相机系统发布当帧输入并求值。读取了 8 个源子图、80 个源节点和 73 条姿态边。编译比较覆盖 14 个已安装的生成资产，包装图、布局和编辑器也通过独立结构检查。全局资产归属审计结果与 BF146.01 的 49 条记录逐项一致。

[原生读数与哈希](source.json) · [静态验收](static.json)

在项目根目录运行。Cargo 目标目录使用 `target_game`。

```sh
CARGO_TARGET_DIR=target_game cargo test --locked --profile play --jobs 2 --lib -- --exact camera_component::tests::camera_player_waits_for_its_definition_before_first_evaluation
CARGO_TARGET_DIR=target_game cargo test --locked --profile play --jobs 2 --lib --tests --no-fail-fast -- --test-threads=2
python3 tools/validate_animgraph_authoring.py --check --manifest tools/animgraph_authoring/source_camera_first_evaluation.authoring.json --summary
```

用新输出目录运行公开入口的 `startup` 和 `camera`。保存的报告可用 `gzip -dc` 解压后复验。

```sh
pwsh -NoProfile -File tools/run_animation_visual_validation.ps1 -Case startup -OutputDirectory docs/audit/camera-first-evaluation-replay/startup -CaptureCount 0 -FunctionalOnly
pwsh -NoProfile -File tools/run_animation_visual_validation.ps1 -Case camera -OutputDirectory docs/audit/camera-first-evaluation-replay/camera -CaptureCount 0 -FunctionalOnly
```

去掉手动播放器的查询过滤后，等待帧重新写入标记并触发回归断言。源码字节恢复并在同一 target 重编后，相机模块 11 项通过。全量双线程回归通过 2,156 项单元测试及 37 项集成测试，包含此前偶发失败的 Quick Stop 和新增的延迟配置用例。公开 Playground 的 startup 和 camera 功能通过，camera 覆盖 11/11，图错误为 0。性能预算超出记录在两份原始报告中。

[消融与恢复](ablation.json) · [全量并行](full-parallel.json) · [真实入口](runtime.json)
