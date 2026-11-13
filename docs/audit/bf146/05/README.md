# BF146.05 动作触发的模式提交

本地 UE `DoJump` 提交 Falling。ALS 攀爬开始写 Custom 并锁定模式，结束解锁并写 Walking。布娃娃开始请求 None，退出根据地面选择 Walking 或 Falling。`SetLocomotionMode` 仅在值变化时先写状态，再通知。

目标在已有阶段用 `commit_state` 提交模式并发消息，复用请求处理中的变更检测。动作许可、锁和动作标签由原系统决定。物理局部模式和同帧多次切换的事件按原顺序发布。

定向验收从原 Jump、Mantle 目标销毁及 Ragdoll 退出的实际系统读取模式事件，分别确认单条 Grounded→Air、有序 Custom→Grounded→None 和单条 None→Grounded。没有状态变化的控制不产生多余消息。另运行空中末态仍发生过落地的 Roll 控制。

[源证据](source-facts.json) · [定向测试](focused.json)

```sh
CARGO_TARGET_DIR=target_game cargo test --locked --profile play --jobs 2 --lib -- accepted_jump_does_not_follow_the_old_base_transform destroyed_mantle_target_requires_ragdoll_bodies ragdoll_exit_inputs_start_get_up_in_the_same_frame
pwsh -NoProfile -File tools/run_animation_visual_validation.ps1 -Case moving_base -CaptureCount 0 -FunctionalOnly
pwsh -NoProfile -File tools/run_animation_visual_validation.ps1 -Case get_up -CaptureCount 0 -FunctionalOnly
```
