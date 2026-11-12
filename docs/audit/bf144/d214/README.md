# D214 BlendList 的原生 FAlphaBlend 时钟

UE `AnimNode_BlendListBase.cpp:128–244` 用目标子项当前权重缩短转场，为每个子项设置新值域，并分别推进 FAlphaBlend。`AlphaBlend.cpp:134–230,290–323` 按剩余时间递推 alpha，按各子项实际终值判断完成。

fresh Standing 的 gait 节点为 Cubic，暴露引脚为 0.3/0.2 秒。原生 582 帧 gait 和 273 帧三子 HermiteCubic 逐值匹配权重与反转时长。gait 的 elapsed 消融改变 117 帧，Hermite 替代 Cubic 改变 275 帧，提前完成正时长改变 4 帧。约 18 纳秒的反转保留两项原始权重。

运行时逐子保留原始 blended value、alpha 与剩余时间，状态机和 BlendList 共用推进式。转换器保留 Cubic 与 HermiteCubic 的源模式。Rust fixture 比较公开权重推进路径。

15 个 AnimBP 根及递归依赖完整重新转换，374 个文件全部比较。差异限于 Standing 和 Camera 的 5 个图内 11 个节点的 Cubic 模式，另同步 Standing 验收图。压缩的完整 diff 可按 SHA-256 校验。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/Stances/AB_Als_Standing -BlendListProbe docs/audit/bf144/d214/gait.request.json -Output docs/audit/bf144/d214/gait.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/Stances/AB_Als_Standing -BlendListProbe docs/audit/bf144/d214/hermite.request.json -Output docs/audit/bf144/d214/hermite.native.json
python docs/audit/bf144/d214/replay_blend_list_clock.py
python tools/animgraph_authoring/ue_animgraph_pose_renderer.py --self-test
python tools/animgraph_authoring/regenerate_installed_corpus.py --content-root "$ALS_CONTENT_ROOT" --output target/d214-blend-mode-corpus
```
