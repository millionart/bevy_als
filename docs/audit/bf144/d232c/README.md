# D232c LayeredBoneBlend 的双精度姿态

`AnimationRuntime.cpp:1821–1842,2421–2464` 先累加 branch filter 的逐骨权重与来源索引，再乘该来源的动态 float 权重。权重失活时索引归零。 `AnimNode_LayeredBoneBlend.cpp:250–322` 跳过未激活层的求值并取 reference。全部层失活时原样返回 Base。目标两种精度使用同一 BoneMask、来源索引和权重端点。

`TransformVectorized.h:463–492` 与 `UnrealMathVectorCommon.h.inl:538–565,680–699` 的部分权重在源厘米 double 域执行向量加权 FMA 与独立四元数 nlerp。 `AnimationRuntime.cpp:1950–1985,2217–2258` 用每骨的实际来源组合父组件旋转，按源空间的乘法、FastLerp 和局部还原求值。部分权重组件只归一化一次。全权重组件的额外归一化由 D238 继续。

原生七个 Layering 节点与 Crouching Stop 节点使用 D231 的真实 WalkRun 姿态。通过可选 `blend_weights` 在原模板上覆盖零、部分、近满与满权重。48 个样本的 3792 条 TQS 和实际 mask 精确，其中 436 条部分权重。提前投影改变 416 条 mesh-space 与 30 条 local-space f32 结果。固定读取第一层会改变 196 条 double 结果。全权重额外归一化单因素改变 521 条 double 与 39 条 f32 结果。原资产属性、CDO 与可编辑 AnimGraph 与无 probe 的读取逐值一致。生成的瞬态函数图在独立进程间分配新 GUID。

```sh
python docs/audit/bf144/d232c/replay_layered_pose.py --requests
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/AB_Als_Layering -Output docs/audit/bf144/d232c/layering-graph.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/AB_Als_Layering -LayeredBoneBlendProbe docs/audit/bf144/d232c/spine.request.json -Output docs/audit/bf144/d232c/spine.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/Stances/AB_Als_Crouching -LayeredBoneBlendProbe docs/audit/bf144/d232c/stop.request.json -Output docs/audit/bf144/d232c/stop.native.json
python docs/audit/bf144/d232c/replay_layered_pose.py
```

其余六个 Layering 模板使用同一资产及对应请求名。Rust 原生 partial 旋转 fixture 随统一验证运行。
