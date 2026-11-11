# D172 Turn Slot 当帧惯性化请求

`AnimNode_Slot.cpp:60–71` 在当前槽更新时向最近的惯性化请求者传递组请求。`AnimInstance.cpp:4671–4675` 将当帧请求交给代理并清除。重新提取两站姿蓝图，`TurnInPlaceStanding`和`TurnInPlaceCrouching`各在 `Idle`，惯性化节点位于状态机输出之后。各自 `Move` 子图保持自己的求值路径。

生成绑定落在实际槽节点。Turn停止或自然结束当帧发布请求，下一次更新清除发布值。槽更新将请求附在现有动画上下文，父惯性化通过原有图请求入口消费。已有求解器、RotationYawSpeed过滤和状态机本身的惯性化请求沿原路径执行。Rust fixture 覆盖Idle下有/无覆盖姿态、移动分支和请求过期，站蹲专用验证入口与其目标图同步。

全量重转15个安装根及递归依赖并逐文件比较374个产物。站蹲各两份生成图仅变更惯性化请求的绑定位置，已校验和安装。原生节点身份及实际目标绑定见 `installed-topology.json`。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/Stances/AB_Als_Standing -Output docs/audit/bf144/d172/standing.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/Stances/AB_Als_Crouching -Output docs/audit/bf144/d172/crouching.native.json
python docs/audit/bf144/d172/check_slot_requests.py
```
