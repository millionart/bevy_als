本目录为 BF144.442a 的 Git 长期证据。`summary.json` 限定结论为当前已就绪相机的选择与消费阶段；完整角度数学、初次/无相机fallback、相机lag和全物理反馈没有宣称等价，父项442保持未完成。

`source-facts.json` 保存重新读取的 UE Input_OnMove、PlayerController/CameraManager缓存读取与target方向/view发布链摘录、哈希；六份实际库复用已提交441c身份并核对不变。`runtime-baseline/parameters.json` 保存原场景240帧实际effective input、控制目标、已发布相机yaw、Update相机和Last相机值。

`prepare.py` 保留原 manual Playground 并添加只读字段，`compile.ps1`和`run.ps1`用于新证据标签；截图0、性能暂停。`check.py` 核验240实际向量、240 Update yaw与239前帧Last对应，五种缺字段/错帧/错消费值反例必须拒绝。

`replay.rs` 调用未改公开方向helper复算实际输入；原视角选择被替成控制目标时恰有80移动帧不符。恢复源码bytes并同target重编后，240完整输出byte相同。正式source/报告/请求/源码/身份与矩阵均随Git保存，可执行文件和普通进程日志临时存tools/tmp。
