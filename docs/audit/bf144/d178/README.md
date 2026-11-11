# D178 Turn 的近全权重输出

`AnimInstanceProxy.cpp:2058–2085` 根据剩余基础权重决定参与姿态。`AnimationRuntime.cpp:469–480` 的单层路径按实际权重输出整个姿态，输入多于一层时最终归一化。Slot保留基础姿态更新的既有门槛，Turn传递实际提交的层数资格，输出按该分支保留权重与旋转模长。

站蹲两个实际槽在0.25及0.4秒重采权重0.99998、0.99999、0.999995及1，并各重采近全权重双层。18组结果中632个单层骨TQS逐值符合缩放，158个双层骨最终归一化通过。旧升为全权重的四组曲线有差，当前代表最大约0.000556946。Rust fixture 验证真实槽的单层、多层和AlwaysUpdateSourcePose，并核对Turn桥接层资格。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/Stances/AB_Als_Standing -SlotProbe docs/audit/bf144/d178/standing-turn.request.json -Output docs/audit/bf144/d178/standing-turn.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/Stances/AB_Als_Crouching -SlotProbe docs/audit/bf144/d178/crouching-turn.request.json -Output docs/audit/bf144/d178/crouching-turn.native.json
python docs/audit/bf144/d178/replay_near_full.py
```
