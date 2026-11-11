# D193 即时停止清除惯性欠时

UE `AnimNode_Inertialization.cpp:478–492,568–587` 在请求推进达到基础时长时清除 deficit，并按骨骼最大时长停用。目标在 inactive 输出路径同样清零，活动路径在基础时长到期时清零。

fresh 主 AB 的 30/60Hz 六条轨迹包含零时长、极短时长和同帧包含零的请求。旧路径在即时停止后保留欠时，把第二次 0.2 秒重入缩到单帧以内。修复后的时序回放恢复 0.2 秒，原生重入帧均有正平移。Rust fixture 通过惯性节点派发同一批请求，核对时长、欠时和原生平移。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AB_Als -InertializationProbe docs/audit/bf144/d193/inertia.request.json -Output docs/audit/bf144/d193/inertia.native.json
python docs/audit/bf144/d193/replay_inertial_deficit.py
```
