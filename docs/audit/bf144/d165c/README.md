# D165c 骨盆弹簧及双权重写回

`RigUnit_SpringInterp.cpp` 的 V2 输入、结果和持续状态均为 float。fresh `CR_Als` 的 48–66 指令先取左右足偏移最小值，按 double 限制并乘权重，随后送 float 弹簧。弹簧结果重新提升为 double 乘权重，再写入 float `PelvisOffset`。骨盆平移和两脚下限消费同一个写入值。

36帧原生重采含真实前24帧、0.75权重舍入边界及25 cm台阶。原始真实帧的完整弹簧参数与`.465`逐值一致，66个下限均读取原生写入。0.75权重下三帧的原生写入为 −9.000106811523438 cm，米制double中转则为 −9.000107765197754 cm。省略任一次权重各在三帧失配。台阶首响应原生为 −1.6625938415527344 cm。

量纲转换保留22个 double 长度标量。向量分量仍以米传输，float/double精度转换在厘米内完成。把新增 double 标量投回米可逐值复原 D165b 独立程序。Rust fixture 运行已安装的完整 `CR_Als` 程序，提供原生Trace输入和写回前骨盆位置，直接比较弹簧状态、写回参数及两脚实际消费。

15个安装根及递归依赖全量重转，374个产物逐文件核对。唯一差异为主蓝图的 Control Rig 定义，生成内容与独立程序一致。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AB_Als -RigNodeHistoryProbe docs/audit/bf144/d165c/pelvis-history.request.json -Output docs/audit/bf144/d165c/pelvis-history.native.json
python docs/audit/bf144/d165c/replay_pelvis_history.py
```
