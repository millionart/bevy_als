# D183 Montage branching marker 回调

UE `AnimMontage.cpp:665–708,741–777,2281–2289` 以有效通知端点建立标记并在跨越时拆步。`2868–3011` 每个位置先按 `start < position <= end` 校正活动状态，再执行该标记的 Begin 或 End。`2587–2592,2783–2816` 零步长仍校正区间，Tick 发生在拆步完成后且仅派发给仍活动的状态。

九个 fresh Montage 的原生 `BranchingPointMarkers` 与原生有效时间逐项核对，30 个标记与安装 manifest 一致。回放覆盖精确开始、精确结束、零步长、负起点后的首次跨越，以及单帧多个标记。旧路径有 60 帧不符；移除显式标记回调、子步区间校正或首帧起点分别留下 52、16 和 9 个不符。原生读取提供标记与精确时间，回调序列按 UE 源码重放。

实际 Mantle 使用已有上一剪辑位置，GetUp 让图上下文与通知读取同一上一位置。Rust fixture 经 GetUp 推进、播放刷新和派发核对两种姿势的首次 Begin/End，生命周期 fixture 核对精确端点及随后零步长的回调。

各源资产路径记录在 `markers.replay.json`。对对应路径调用读取器并沿用原报告文件名。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Actions/Roll/AM_Als_Roll -NotifyClockProbe -Output docs/audit/bf144/d183/roll.native.json
python docs/audit/bf144/d183/replay_branching_markers.py
```
