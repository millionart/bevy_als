# BF146.04 状态请求变更检测

源角色的实际状态 setter 仅在值变化时写入并触发回调。Bevy 的 `Mut` 会在可变解引用时标记变更。状态 resolver 将请求字段和实际状态以 `Mut` 传给处理函数，只有消费请求、修改期望姿态或提交实际状态时更新对应 tick。

定向测试覆盖空请求、排队、拒绝、同值、期望姿态，以及实际移动模式、旋转、Overlay、档位变化。每轮验证消息、保留请求和组件 tick。另复验实际姿态变化后的 Avatar 偏移。

[源与 Bevy API 合同](source-facts.json) · [基线](baseline.json) · [定向测试](focused.json)

```sh
CARGO_TARGET_DIR=target_game cargo test --locked --profile play --jobs 2 --lib -- als::transition::tests:: stance_changes_preserve_the_mesh_world_origin
pwsh -NoProfile -File tools/run_animation_visual_validation.ps1 -Case crouching -CaptureCount 0 -FunctionalOnly
```
