# D223 按实例汇总缓存请求

UE `AnimNode_SaveCachedPose.cpp:178–204` 在 PostGraphUpdate 收齐该实例的请求，再按严格最高权重执行一次。fresh Layering 编译队列与已安装图的缓存位置逐一匹配。Legs 和左右臂路径都能请求同一个 Locomotion 输入缓存。

原生五个请求控制包含低→高、高→低、普通不同权重、单次零权重和无请求。只取首请求在两组失配，包含 0.99999994→1 的 1 ULP 边界。

目标用编译队列汇总当前实例内各路径，单个可达的 linked cache 也按胜出权重预热。分层骨骼遍历使用实际分支权重。Rust fixture 通过实际 linked graph 和两个身体缓存核对请求顺序、调用次数及权重。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/AB_Als_Layering -CachedPoseSchedule -CachedPoseLifecycleProbe -Output docs/audit/bf144/d223/cache.native.json
python docs/audit/bf144/d223/replay_cache_requests.py
```
