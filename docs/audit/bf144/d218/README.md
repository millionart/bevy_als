# D218 同步记录使用源缓存提交顺序

UE `AnimInstanceProxy.cpp:139–158` 先更新根图，再按已编译的 SavedPoseQueue 逐项 PostGraphUpdate。fresh Standing 的 Move Forward 与 Move Forward Base 在同一队列分别位于 4、5。原生 BlendList、两层 TwoWay 和共享 Base 缓存探针记录了 Sprint、加速 Sequence、Forward Base 的实际调用顺序。

四条父图轨迹共 244 帧。把同一批权重分别送入原生 FAnimSync 的实际提交序与姿态合并序，后者改变 14 次 leader 和 163 帧组比例或标记。61 帧全阻断路径保持原生输出。

生成器将编译队列位置写入保存缓存节点。同步记录携带其所属图的缓存位置，当前图的同步出口按该队列稳定排序。同步预览和实际提交共用该顺序。Rust fixture 经两级缓存和实际同步出口覆盖等权重选择及旧顺序消融。

15 个 AnimBP 根及递归依赖全量重新转换。374 个产物逐一比较，9 个图的 36 个保存缓存节点增加源队列位置。安装产物与新生成字节一致。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/Stances/AB_Als_Standing -CachedPoseSchedule -BlendListProbe docs/audit/bf144/d218/order.request.json -Output docs/audit/bf144/d218/order.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Grounded/Sprint/A_Als_Sprint -SyncGroupClockProbe docs/audit/bf144/d218/group.request.json -Output docs/audit/bf144/d218/group.native.json
python docs/audit/bf144/d218/replay_submission_order.py
python tools/animgraph_authoring/ue_animgraph_converter.py --self-test
python tools/animgraph_authoring/regenerate_installed_corpus.py --content-root "$ALS_CONTENT_ROOT" --output target/d218-sync-submission-corpus
```
