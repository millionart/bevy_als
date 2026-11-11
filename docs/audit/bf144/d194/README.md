# D194 正的极短惯性请求

UE `AnimNode_Inertialization.cpp:478–588` 推进非负请求并以 elapsed 和 duration 结束。标量函数的 `1e-7` 时间容差决定极短请求的实际输出。目标骨骼与曲线只要求激活时长为正。

fresh 主 AB 的 18 条轨迹覆盖零、1e−7、两个极短值、float epsilon 和其上方，分别以零、10ns、普通步长推进。旧激活门槛造成 13 个输出帧失配，1.1e−7 在零步长保留一米与曲线 1。Rust fixture 复用实际惯性节点并分别运行有骨骼和仅曲线输入。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AB_Als -InertializationProbe docs/audit/bf144/d194/inertia.request.json -Output docs/audit/bf144/d194/inertia.native.json
python docs/audit/bf144/d194/replay_short_inertial_requests.py
```
