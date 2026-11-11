# D167 动态过渡的原生时长和混出窗口

四个 Sequence 与原生动态 Montage 的 `GetPlayLength()` 均为 `1.5333333015441895` 秒。`AnimMontage.cpp:2626–2637` 在当前 `BlendOutTriggerTime=0` 时按剩余播放时间与 `1e-4` 秒比较。D121b的`d43dfff1`已用原生时长生成四个目标配置，当前槽采样和两个站姿的实际播放构造均得到同一 float 值。

四个原生Montage各重采边界、窗口前和窗口内三个场景，共32帧逐值对照。先推进1秒，再推进0.0221220143秒，原生位置为1.533182978630066秒且仍在播放。将配置恢复为六位小数会在该帧提前混出，随后两帧的最大权重差为0.059750015。修正旧Rust断言并用原生读数检查四个实际播放构造、槽配置、时钟和权重。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Transitions/A_Als_Stand_DynamicTransition_Left -MontageClockProbe docs/audit/bf144/d167/boundary.request.json -Output docs/audit/bf144/d167/stand-left.native.json
python docs/audit/bf144/d167/replay_boundary.py
```
