# D199 动画序列的原生时间轴末点

UE `AnimSequenceBase.cpp:501–593` 用 GetPlayLength 推进无 marker 播放时钟，`AnimNode_StateMachine.cpp:820–854` 消费长度与累计时间的差。Bevy glTF 加载器读取 float 输入 accessor，AnimationClip 的时长来自曲线域末端，GraphClip 继承该值。

128 个已登记序列的安装资产扫描发现 20 个末点短一个 float 步长。fresh 读取分别确认 Jump、Crouch 和 Standing Walk 的原生长度。Bow Draw 的 fresh 原生及安装时长均为 1 秒，零值出现在无事件的 manifest 记录中。编译器按已确认的末点漂移生成现有 `ALSRuntime` 路径的副本并更新 GraphClip 引用，符合原导入器的单缓冲约定。

原始 glTF/bin 逐字节比对基线。每个目标 bin 只替换末点的四字节，JSON 更新对应 accessor 最大值，GraphClip 只改变 glTF 引用。完整文本差异、字节偏移、原生读数和哈希均在本目录。撤销一个末点修改时检查失败，重新编译恢复原哈希。Rust fixture 经实际加载器读取全部 20 个时长。

15 根动画蓝图及递归依赖全量转换成功，374 个生成文件与安装版本逐字节一致。65 个 GraphClip 的 177 条事件通过轨道检查。

原生读取按 `timing-sources.json` 中的资产路径重复执行，Bow Draw 使用默认属性读数。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Air/Jump/A_Als_Jump_Walk_Left -NotifyClockProbe -Output docs/audit/bf144/d199/A_Als_Jump_Walk_Left.native.json
python tools/import/compile_als_clip_timelines.py --write
python tools/import/compile_als_clip_timelines.py
python docs/audit/bf144/d199/verify_clip_timelines.py
python tools/animgraph_authoring/regenerate_installed_corpus.py --content-root "$ALS_CONTENT_ROOT" --output target/d199-native-timeline-corpus
```
