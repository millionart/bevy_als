# BF134.02B1 主动画实例归属复验

源角色 `CharacterMesh0` 绑定 `AB_Als_C`。`AAlsCharacter::PostInitializeComponents` 从当前 Mesh 取得动画实例。`UAlsAnimationInstance::NativeUpdateAnimation` 在实例内比较动作并重置落地入口，Grounded 的 Roll 入口从父实例读取该值。两个 ECS 输入系统都从 `AlsCharacterAssembly` 定位播放器，复用装配模块的实体归属检查。

[原生请求与哈希](native-identities.json) · [源码与角色设置](source-facts.json) · [拓扑与基线资产检查](static-source-match.json) · [双 Avatar 失败](baseline-ownership.json) · [消融](ablation.json) · [恢复测试](restored-ownership.json)

定向反例通过动画场景的 `WorldInstanceReady` 为两个同时存在的 Avatar 绑定播放器，轮流切换 Assembly。分别断言保留角色落地入口、当前 Provider 发布该标签、旧根和旧 Provider 未收到输入。该夹具只运行输入阶段，真实根姿态与换 Avatar 使用公共 Playground 功能验收。

```sh
CARGO_TARGET_DIR=target_game cargo test --locked --profile play --jobs 2 --lib root_and_provider_inputs_follow_the_selected_avatar
python3 tools/validate_animgraph_authoring.py --manifest tools/animgraph_authoring/bf146_01_selected_avatar.authoring.json --check --summary
pwsh -NoProfile -File tools/run_animation_visual_validation.ps1 -Case startup -CaptureCount 0 -FunctionalOnly
pwsh -NoProfile -File tools/run_animation_visual_validation.ps1 -Case foot_ik -CaptureCount 0 -FunctionalOnly
```

`validate_runtime_graph_asset_boundary.py --check` 对基线的 `ALSRuntime` 文件及 `.DS_Store` 仍报 49 个归属项。两份受影响的图保持原文件身份，通过独立的拓扑、布局、编辑器和原生节点对照。
