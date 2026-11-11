# D165b Rig 的厘米单精度足偏移

`AlsRigUnit_FootOffsetTrace.cpp` 将坡面修正与命中高度按源 float 输出。`AlsRigUnit_ApplyFootOffsetLocation.cpp` 按两次 float 减法得到上限，保留足偏移、骨盆下限及连续弹簧状态，再以 float 计算最大腿长。`AlsRigUnit_ChainLength.cpp` 每段加入 double 厘米长度后写回 float。`CR_Als` 的这条标量链还包括求最小值、弹簧、拷贝及极向量缩放。

转换器根据已有量纲传播，将 float 长度寄存器写为 `Centimeters`。运行时用该精度传递标量和状态，在物理查询、向量分量及米制输出边界换算。旧 `F32` 值仍由米制入口读取。单精度弹簧算子接受同一类型，骨盆writer的完整原生控制由 D165c 验证。

重采 `CR_Als` 原始程序及真实源Rig前12帧。22次足位置调用的输入、状态、输出、所读骨位与封存报告逐值一致，左右脚Trace输出与Location输入相同。对636次已资格记录复算厘米高度上限和最终位置，121次限长与原生一致，全部位置的F32投影一致。把旧米制弹簧状态投回厘米仍有582次失配。Rust fixture 使用真实回调连续保存左右脚各自状态，并重放原生与受控Trace输入。

15个安装根及递归依赖全量重转，374个产物全部比较。仅 `AB_Als/AnimGraph.animgraph.ron` 的 Control Rig 定义变化，与同次生成的 `CR_Als.program.json` 相符。47个新标量按旧方式投回米制float后，独立程序逐值等于旧产物。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/CR_Als -Output docs/audit/bf144/d165b/rig.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AB_Als -RigNodeHistoryProbe docs/audit/bf144/d165b/foot-history.request.json -Output docs/audit/bf144/d165b/foot-history.native.json
python docs/audit/bf144/d165b/replay_foot_history.py
python docs/audit/bf144/d165b/check_program_projection.py bdc15955
```
