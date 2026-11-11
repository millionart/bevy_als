# D175 零 delta 首帧的 Turn 实例

`AnimMontage.cpp:2785–2790` 在实例停止且Blend完成时结束生命周期。刚开始播放的动态Montage可处于valid/playing且权重为0。保留目标中尚在播放的实例，已淡出实例沿原清理路径。

八个实际站蹲左右90°/180°序列重采两个零delta帧及恢复的三个时间步，共40帧。八个首帧均有效且正在播放，恢复后权重依次达到0.15625、0.5和1。旧清理在八个采样已就绪的首帧删除实例。Rust fixture 核对实际Turn构造、同一身份、时钟和权重，另用生成图及actor时间倍率0→1验证首帧暂停恢复。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/TurnInPlace/A_Als_Turn_90_Left -MontageClockProbe docs/audit/bf144/d175/standing.request.json -Output docs/audit/bf144/d175/standing-90-left.native.json
python docs/audit/bf144/d175/replay_zero_delta.py
```
