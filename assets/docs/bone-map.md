# ALS 骨骼映射（M4.1）

权威参考：ALS 关键骨骼常量表 + VB 虚拟骨占位策略。数据源为 `assets/models/SKM_Als.gltf`（`skin[0].joints` 79 关节，`nodes[i].name` 即骨骼名），并与 ALS-Refactored 源码常量 `Plugins/ALS-Refactored/Source/ALS/Public/Utility/AlsConstants.h` 交叉核对。生成脚本 `tools/import/gen_bone_map.py` 输出全量清单到 `tmp/bone-map.json`（gitignored）。

## 一、79 骨清单来源

SKM_Als 由 UE5 骨骼导出，`skin[0].joints` 按节点索引给出全部 79 个关节，其中：

| 区段 | 索引 | 内容 | 数量 |
|---|---|---|---|
| 根/骨盆/脊柱/颈头 | 0–4, 47–48 | root、pelvis、spine_01–03、neck_01、head | 7 |
| 左臂+手指 | 5–25 | clavicle_l 至 upperarm/lowerarm/hand + 五指×3 + twist×2 | 21 |
| 右臂+手指 | 26–46 | 同左臂镜像 | 21 |
| 左腿 | 49–54 | thigh/calf/twist/foot/ball | 6 |
| 右腿 | 55–60 | 同左腿镜像 | 6 |
| IK 链 | 61–67 | ik_foot_root/l/r、ik_hand_root/gun/l/r | 7 |
| VB 虚拟骨节点 | 68–78 | 见第三节，共 11 个 | 11 |

## 二、关键骨骼分组（`ALS_KEY_BONES`，30 个）

对应 `AlsConstants.h:14–23` 的 C++ 权威常量及 IK 链完整集合，Rust 常量表见 `crates/als-core/src/bones.rs`。

### 核心
`root`、`pelvis`

### 脊柱（spine 组）
`spine_01`、`spine_02`、`spine_03`、`neck_01`、`head`
（ALS 运行时直接引用 `spine_03` 与 `head`：视角/脊柱偏转与 LayerSpine 曲线作用域。）

### 肢体——左臂
`clavicle_l`、`upperarm_l`、`lowerarm_l`、`hand_l`

### 肢体——右臂
`clavicle_r`、`upperarm_r`、`lowerarm_r`、`hand_r`

### 肢体——左腿
`thigh_l`、`calf_l`、`foot_l`、`ball_l`

### 肢体——右腿
`thigh_r`、`calf_r`、`foot_r`、`ball_r`
（ALS 运行时直接引用 `foot_l` / `foot_r`：FeetState 足部锁定基准。）

### IK 链
`ik_foot_root`、`ik_foot_l`、`ik_foot_r`、`ik_hand_root`、`ik_hand_gun`、`ik_hand_l`、`ik_hand_r`
（ALS 运行时引用 `ik_foot_l` / `ik_foot_r` 作足部 IK 目标；`ik_hand_gun` 为持枪武器挂接空间。）

## 三、VB 虚拟骨占位策略

### 真实命名（源码 grep 结论）

**重要修正：ALS-Refactored 的虚拟骨前缀是 `VB `（VB + 空格），不是 `VB_`。**
证据：
- `Source/ALSEditor/Private/AlsSkeletonUtility.cpp:130` 校验 `StartsWith(TEXTVIEW("VB "), CaseSensitive)`，缺前缀报错 "must contain the 'VB ' prefix"；
- `Source/ALS/Public/Utility/AlsConstants.h:20–25` 四个常量均为空格命名。

C++ 直接引用的 4 个虚拟骨（`UAlsConstants`）：

| 名称 | 用途（代码证据） |
|---|---|
| `VB foot_l` | 足部 IK 目标捕获：`bUseFootIkBones=false` 时取其世界变换作为 FeetState 目标（`AlsAnimationInstance.cpp:1281`） |
| `VB foot_r` | 同上右脚（`AlsAnimationInstance.cpp:1290`） |
| `VB hand_l_to_ik_hand_gun` | 手相对 ik_hand_gun 武器空间的虚拟变换，供动画资产/Control Rig 的 HandIkRetargeting 消费 |
| `VB hand_r_to_ik_hand_gun` | 同上右手 |

UE 语义：虚拟骨 = 「target 骨在 source 骨空间下的变换」，命名即 `VB <source>_to_<target>`（无 `_to_` 者 source 为 pelvis/root 类稳定参考）。

模型内实际存在的 11 个（glTF joints 68–78，与 UE 骨架一致）：
`VB foot_root`、`VB foot_l`、`VB foot_r`、`VB hand_l_to_hand_r`、`VB hand_r`、`VB hand_l_to_ik_hand_gun`、`VB ik_hand_r`、`VB hand_r_to_hand_r`、`VB hand_l`、`VB hand_r_to_ik_hand_gun`、`VB ik_hand_l`

### bevy 侧占位方案

1. **首选路径（当前资产已满足）**：SKM_Als.gltf 已把 11 个 VB 作为普通关节节点导出，bevy_animation_graph 按 BoneId 实体名路径映射导入后，VB 自动成为骨骼实体层级中的普通实体，无需额外建表。
2. **marker entity 兜底路径**：若后续替换/新增不含 VB 节点的资产（或需在运行时合成 VB），由 M4 之后的功能层在对应 source 骨骼实体下 spawn 带 marker 组件的子实体占位（组件名按用途如 FootIkTarget / HandIkGunTarget），每帧从父骨与 target 骨全局变换合成「target 相对 source」的局部变换，等价 UE 虚拟骨求值。
3. **消费方（M7 IK）**：足部 IK 优先读 `VB foot_l/r` 实体的世界变换作锁定目标（对应 `bUseFootIkBones=false` 行为）；`bUseFootIkBones=true` 时改读 `ik_foot_l/r`。手部 retargeting 读 `*_to_ik_hand_gun` 系列。
4. 常量单一来源：`crates/als-core/src/bones.rs` 的 `VIRTUAL_BONES`（11 项）与 `ALS_KEY_BONES`（30 项）；名称必须逐字匹配 glTF 节点名（含空格），禁止在本仓库改写成 `VB_` 下划线风格。
