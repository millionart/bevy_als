# D164 TwoBoneIK 近根距离

`TwoBoneIK.cpp:74–120` 先在厘米空间求两个端点的差和长度，仅在长度严格小于 `1e-4` 时改用源 X 轴。`Vector.h:1720,2058` 的长度和安全归一化均消费这一厘米向量。项目按同一个 `0.01` 投影比较距离和已有阈值。当旧米制计算落在阈值内而源计算保留实际方向时，使用同一厘米向量归一化。普通解继续使用原有米制几何。

用 `.478` 的两个实际根位置构造阈值两侧及重合输入，再加入精确等号和整个链平移后的归一化控制。10 个原生膝与末端读数通过。旧比较四组失配，先减再换算仍遗漏两个实际根位置的反例。平移控制中仅修比较仍会把实际方向归零。Rust fixture 通过 `FRigUnit_TwoBoneIKSimplePerItem::Execute` 读取并写回关节位置。

```sh
python docs/audit/bf144/d164/replay_goal_boundary.py --request > docs/audit/bf144/d164/goal.request.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALSExtras/Environment/Meshes/SM_Als_Stairs_01 -GeometryQueryProbe docs/audit/bf144/d164/goal.request.json -Output docs/audit/bf144/d164/goal.native.json
python docs/audit/bf144/d164/replay_goal_boundary.py
```
