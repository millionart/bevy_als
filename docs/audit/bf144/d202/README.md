# D202 同步组的原生同分排序

UE `FAnimGroupInstance::Prepare` 在 `AnimationAsset.cpp:121–124` 用 `FAnimTickRecord::operator<` 的严格分数比较执行 `TArray::Sort`。`Algo/IntroSort.h:41–146` 在最多八项时从末端交换，在更大组分区，耗尽深度后按 `Algo/Impl/BinaryHeap.h:66–95,156–203` 排序。当前组和上一组的重排共用相同的输入顺序及比较。

四个实际 Land Sequence 按请求的普通 1 倍与 Additive 1.75/1.5 倍速率加入真实 `FAnimSync`。16 条请求共 21 帧的原生完整候选序、leader 及各时间逐值匹配。旧稳定排序改变 18 帧的顺序和 13 帧的 leader。仅选择排序在 7 个大组失配，移除堆分支在 2 个组失配。Rust fixture 通过实际排序和同步组 resolver 消费原生轨迹。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Air/Land/A_Als_Land_Light -SyncGroupClockProbe docs/audit/bf144/d202/land.request.json -Output docs/audit/bf144/d202/land.native.json
python docs/audit/bf144/d202/replay_sync_order.py
```
