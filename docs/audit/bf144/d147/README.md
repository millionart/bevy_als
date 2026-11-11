# D147 隐藏期间保留上一动画更新的全局倍率

`SkeletalMeshComponent.cpp:1883–1915` 的空闲隐藏路径跳过 pose tick。`AnimInstance.cpp:709–746,789–793` 的隐藏 montage-only 路径也在 NativeUpdateAnimation 前返回。`AlsAnimationInstance.cpp:111–116` 先用上一次 NativeUpdate 的全局倍率恢复未缩放步长，再保存当前倍率。`637–647` 只在第一人称 Head 平滑中用这个步长。

`settings.native.json` 记录 AIS_Als_Default 的 Head 半衰期 0.01 秒（第一人称）与 0.1 秒（第三人称）。`AlsRotation.h:144–156`、`AlsMath.h:84–88` 和 `UnrealMathUtility.h:1553–1559` 给出插值公式。静态重放保留六组，移除写入条件只改变隐藏期间降低倍率的第一人称组，恢复帧由 11.285648° 偏到 30.767437°。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Data/AnimationInstance/AIS_Als_Default -Output docs/audit/bf144/d147/settings.native.json
python docs/audit/bf144/d147/replay_time_history.py
```

反例 `hidden_idle_retains_dilation_until_head_updates_again` 经过实际 Head 图，检查隐藏期间帧号、恢复第一帧的历史倍率与头部角度，以及第二帧使用新倍率。Rust 与真实 Playground 验证待统一执行。
