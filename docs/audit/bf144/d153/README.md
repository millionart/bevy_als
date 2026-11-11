# D153 Linked pose 缓存的编译顺序

`AnimBlueprintExtension_CachedPose.cpp:44–123` 按根和缓存依赖生成有序队列。`AnimNode_SaveCachedPose.cpp:111–125,178–201` 收集实际请求，随后以最高权重执行一次源更新。`AnimInstanceProxy.cpp:150–158` 使用编译队列执行 PostGraphUpdate。

AB_Als_Layering 的原生队列含 11 个节点。Locomotion 在 Overlay 之前，两者的 Save 节点各直接读取一个 linked pose 输入。Default Overlay 的 Two Way Blend 读取父实例 GroundPredictionAmount。旧目标在手部层权重非零时先按需解析 Overlay，再读取含 RefreshInAir 的 Locomotion。Fall 当帧正确，Overlay 在开关切换的两帧落后。

转换器从原生队列为具有至少两路直接 linked pose 缓存的根图生成顺序。运行时沿有效分支汇总请求，沿队列展开中间缓存的依赖，再按顺序预热已请求的 linked 输入。每个缓存沿用当前更新权重选择与共享状态。首轮完整转换遇到允许缺失的根语义字段，修正并用原始 Layering 图验证可正常生成。

重转 15 个已安装根及其递归依赖，374 个产物逐文件比较。Layering 与 Grounded 各增加一处编译队列字段。静态消融移除队列会在预测值切换的两帧恢复旧结果，跳过中间缓存会漏掉通过 Pelvis 传来的 Overlay 请求。Grounded 的声明顺序按原生 Standing→Crouching 核对。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/AB_Als_Layering -CachedPoseSchedule -Output docs/audit/bf144/d153/layering.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/AB_Als_Grounded -CachedPoseSchedule -Output docs/audit/bf144/d153/grounded.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/Overlays/AB_Als_Default -CachedPoseSchedule -Output docs/audit/bf144/d153/default-overlay.native.json
gzip -dc docs/audit/bf144/d153/layering.native.json.gz | shasum -a 256
gzip -dc docs/audit/bf144/d153/grounded.native.json.gz | shasum -a 256
python docs/audit/bf144/d153/replay_cache_order.py
```

`default_overlay_reads_the_same_ground_prediction_as_fall` 观察同一角色、同帧的 Fall 与 Default Overlay 实际输入，覆盖稳定值和两次切换。Rust、消融恢复和 Playground Gate 待统一验证。
