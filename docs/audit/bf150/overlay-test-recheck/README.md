# BF150.04a 站姿 Idle 观察器复验

本轮用本地 UE 原生 reader 重新提取 Standing 和 Layering。Standing 的 `Idle` 中节点 `a189ffa4-46d2-a4ea-01e7-728ccfb6d729` 是 `A_Als_Stand_Pose` 的 Sequence Evaluator。`ExplicitFrame` 默认 0、没有连线，`bTeleportToExplicitTime` 为真。它的 Pose 连到 `Modify Curve`。Layering 的 `Locomotion Input` 节点 `3bcbb23c-4bf1-415c-bdba-f6b61029c999` 接到缓存姿态节点 `6fc6ad34-44fb-cee4-d29d-fa9cc5439c67`。目标 RON 保留两处身份、采样值和姿态链。原生 JSON 无损压缩，[身份清单](native-identity.json)列出读取对象、原始字节长度与 SHA-256。

`default_overlay_window(0, …)` 表示站姿 Idle。真实观察器仍检查该窗口的 Locomotion 输入存在且含有效骨骼，运动链则要求 Secondary Motion、加性结果和分层输出。旧单测误用 Idle 窗口断言全部动态输入边。把该测试改用步行窗口后，原有边身份和缓存姿态断言仍执行。已有加载角色的用例检查 Idle 的 Locomotion 输入存在、Idle 运动链排除该边、步行运动链包含该边。角色系统、图、素材和验收阈值沿用原版本。

基线 `dae3d3b5` 的精确测试[失败](before.txt)，同构建目录重编后[分层定向 8 项](overlay-layering.txt)与[加载角色对照 1 项](overlay-loaded.txt)通过。[公共验证器](visual-self-test.txt)和[运行资产边界](asset-boundary.txt)自测通过。统一真实入口复验归 BF154.01。

```powershell
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/Stances/AB_Als_Standing.AB_Als_Standing -Output standing.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/AB_Als_Layering.AB_Als_Layering -Output layering.native.json
cargo test --workspace --locked --jobs 2 --lib layering_
cargo test --workspace --locked --jobs 2 --lib default_overlay_observers_compare_the_loaded_prepared_sequence_poses
```
