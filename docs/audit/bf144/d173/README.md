# D173 LandToGrounded 的序列与播放参数

`AB_Als_Locomotion` 的原生 `AnimNotify_LandToGrounded` 连接 `PlayTransitionRightAnimation`，实参为混入0.1秒、混出0.2秒、速率1.4、起点0、StandingIdleOnly=false。`AlsAnimationInstance.cpp:1650–1661` 根据当前姿势选择对应右序列。通知使用已有站蹲过渡构造，实际序列时长由目标Montage配置提供。

两份原生动态Montage分别重采入场和同组旧Montage淡出。首个50 ms的采样点为0.07000000029802322秒，权重0.5，旧Montage权重0.5。旧参数分别给出约0.275秒、0.15625和0.84375。Rust fixture 由实际跳跃触发生成Locomotion通知，核对回调时站姿与蹲姿、序列、时钟及被替换实例。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/AB_Als_Locomotion -Output docs/audit/bf144/d173/locomotion.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Transitions/A_Als_Stand_Transition_Right -MontageClockProbe docs/audit/bf144/d173/landing.request.json -Output docs/audit/bf144/d173/standing.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Transitions/A_Als_Crouch_Transition_Right -MontageClockProbe docs/audit/bf144/d173/landing.request.json -Output docs/audit/bf144/d173/crouching.native.json
python docs/audit/bf144/d173/replay_landing.py
```
