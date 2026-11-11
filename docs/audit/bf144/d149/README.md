# D149 首个普通更新保留注册 pending

`Actor.cpp:6262,6297` 先注册组件，再设全部注册完成。`SkeletalMeshComponent.cpp:1036,1198–1216` 在 mesh 注册中执行零 delta 更新。`AnimationSettings.cpp:19` 默认启用该更新。`SkeletalMeshComponent.cpp:1354,1455` 不会在随后非强制初始化时再次初始化已有主实例。

`AlsAnimationInstance.h:48` 将 pending 初始化为 true。`AlsAnimationInstance.cpp:224–227` 按注册完成状态在更新后写回 pending，`1103–1104` 在此之前按 pending 抑制并消费 Jump 请求。原生 AB_Als 链接 AB_Als_Locomotion，T19 先判断 InAir 且 bJumped，T20 再以 InAir 进入 Fall。静态重放在首个普通更新得到 Fall，移除注册边界或把输入延后一帧均得到 Jump。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AB_Als -Output docs/audit/bf144/d149/ab-als.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/AB_Als_Locomotion -Output docs/audit/bf144/d149/locomotion.native.json
gzip -dc docs/audit/bf144/d149/locomotion.native.json.gz | shasum -a 256
python docs/audit/bf144/d149/replay_startup_pending.py
```

原生 Locomotion 报告压缩前与解压后的 SHA-256 一致。出生测试增加 pending 寿命断言，`first_ordinary_animation_update_consumes_jump_without_selecting_jump_pose` 从公开出生入口核对物理起跳与实际 Jump/Fall 权重。Rust、消融恢复和 Playground Gate 待统一执行。
