# 冷启动足位置首差

同一正式30Hz轨迹新增八帧实际Foot Lock输入观察。319帧其余原字段逐值相同，公共功能通过。第3个采样是首次有效Feet，两端均为满锁，目标、锁点和最终世界位置在本端相等。

`factors.py` 以新采原生的世界位置及组件逆变换重放两侧最终F32厘米位置。只在世界坐标处投F32米改变两侧最终位置，独立只在组件处投F32米再换回厘米也改变两侧实际寄存器输入。`first-foot-location-factors.json` 记录两因素、原生值与实际目标值。完整源报告可按 `../source-root-history/reconstruct.py` 恢复。

D318 A段使Rig消费源F32厘米组件位置。旧Vec3调用保留米制合同。单因素反例、6项定向、319帧和正式Control Rig功能通过，冷首六有效帧的36个真实寄存器分量均记录源F32厘米投影。世界足位置差继续在B段处理。

```sh
python docs/audit/bf144/u07/foot-target-precision/factors.py <native.json> docs/audit/bf144/u07/foot-target-precision/target-before/root-history.json.gz
```
