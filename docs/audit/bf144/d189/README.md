# D189 GetUp 结束帧的旋转时点

fresh Front 和 Back GetUp 序列的 `bEnableRootMotion=False`。UE `AnimSequence.h:407`、`AnimCompositeBase.cpp:256–267`、`AnimMontage.cpp:931–941` 按槽段根运动标志确定 Montage 资格。GetUp 使用组件 TickPose。ALS `AlsCharacter.cpp:118–128,274–291,1514–1519` 先在角色阶段按 GettingUp 跳过身体旋转，再由 Mesh branching End 清除动作。

GetUp 推进时缓存通知前的 GettingUp 状态。身体旋转在本帧消费这份状态，动画图仍读取已派发的动作。下一帧 GetUp 状态刷新时退出旋转保护。移动输入仍沿已有的消费与解锁时点。

两姿势、30/60Hz、移动和瞄准的静态回放覆盖结束前、结束及下一帧。恢复旧顺序会在 8 个瞄准结束帧提前放行。真实角色夹具使用已加载 GetUp，检查结束帧身体 yaw、消费输入与已清空的图动作，再检查下一帧恢复转向和输入。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Actions/GetUp/A_Als_GetUp_Front -NotifyClockProbe -Output docs/audit/bf144/d189/front.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Actions/GetUp/A_Als_GetUp_Back -NotifyClockProbe -Output docs/audit/bf144/d189/back.native.json
python docs/audit/bf144/d189/replay_get_up_consumers.py
```
