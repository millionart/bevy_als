# D180 Roll 排队状态的有效时间

`AnimTypes.cpp:81–98` 以 float 叠加触发偏移、时长与结束偏移。fresh Roll的 `AlsRootMotionScale` 原生有效区间为 `9.999999747378752e-05` 至 `1.500100016593933`，倍率 `1.2999999523162842`。现行manifest已有这些值。旧GraphClip区间为0至1.5，旧编译器也会在源float偏移端点拒绝映射。

轨道编译按源float偏移边界校验，保留已验证的时间。全部65个GraphClip、98轨、177条事件重编，差异仅涉及起止时间。Roll有效开始前一float尚未采样，clip终点仍在状态内；超出合法偏移的下一float被拒绝。Rust fixture 直接读取已安装GraphClip并核对采样边界。

15个安装AnimBP根及递归依赖完成全量转换，374个生成文件与安装版本字节一致。轨道与蓝图的完整diff及watchdog结果分别记录于本目录。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Actions/Roll/A_Als_Roll -NotifyClockProbe -Output docs/audit/bf144/d180/roll.native.json
python tools/import/compile_als_event_tracks.py --check
python docs/audit/bf144/d180/replay_event_bounds.py
python docs/audit/bf144/d180/compare_event_tracks.py target/d180-replayed-tracks 04c792dc
python tools/animgraph_authoring/regenerate_installed_corpus.py --content-root "$ALS_CONTENT_ROOT" --output target/d180-event-tracks-corpus
```
