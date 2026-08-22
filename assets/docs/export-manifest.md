# assets/docs/export-manifest.md — 导出资产清单

> 每次从 ALS-Refactored 工程导出占位资产，必须在此登记：资产名、源 uasset 路径、
> 导出格式、日期。禁止提交 ALS 商店原始 uasset（见 `.gitignore` 与 agents.md 规则 5）。
> 导出方式与连接步骤见 `docs/env.md`（隐私文件）。

## 约定

- 目录：
  - `assets/models/` — UE5 Manny 系蒙皮网格 + 骨骼骨架（glTF，含 skin/joints；无纹理）
  - `assets/animations/` — 动画组 Grounded/Air/Actions/Transitions/TurnInPlace/RotateInPlace/Overlays/View/Base（glTF：骨架节点 + 单动画轨道）
  - `assets/animations/blendspaces/` — BlendSpace 轴参数与采样点定义（JSON；glTF 无此概念）
  - `assets/graphs/` — 动画图 RON（本仓库自建，非 UE 导出，不在此登记）
  - `assets/curves/` — 关键曲线数值表（JSON 中间格式；M2.5 转 RON 加载）
- 源工程：ALS-Refactored v4.18（UE 5.8.1），源路径为工程内 `/ALS/...` 虚拟路径
- 导出通道（M0.5 定案）：**headless commandlet** `UnrealEditor-Cmd -run=pythonscript`
  （脚本 `tools/export/export_all.py`，启动器 `tools/export/run_cmdlet.ps1|.sh`，
  引擎与工程路径经 gitignored `tools/env.local.json` 注入）。
  备选的 Unreal MCP toolset 经核查无网格/动画导出工具（ProgrammaticToolset 为受限沙箱），
  故导出走 commandlet，MCP 仅用于 M0.4 能力验证与资产枚举。
- 导出器：引擎自带 glTF Exporter 插件（EnabledByDefault）经 `AssetExportTask`
  + `Exporter.run_asset_export_task`；选项关闭纹理导出。
- 曲线说明：`UCurveFloat.FloatCurve` 为裸 `UPROPERTY()`，Python 属性访问不可见；
  数值表经 BlueprintCallable 的 `GetTimeRange()+GetFloatValue()` 在值域上均匀采样 129 点
  （形状保真，键点位置不保留）。Montage 为容器资产（其序列已全部导出），不单独导出。

## 登记记录

| 日期 | 源 uasset 路径 | 格式 | 目标路径 |
|---|---|---|---|
| 2026-08-22 | `/ALS/ALS/Animations/Actions/GetUp/A_Als_GetUp_Back.A_Als_GetUp_Back` | glTF | `assets/animations/Actions/GetUp/A_Als_GetUp_Back.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Actions/GetUp/A_Als_GetUp_Front.A_Als_GetUp_Front` | glTF | `assets/animations/Actions/GetUp/A_Als_GetUp_Front.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Actions/Mantle/A_Als_Mantle_High.A_Als_Mantle_High` | glTF | `assets/animations/Actions/Mantle/A_Als_Mantle_High.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Actions/Mantle/A_Als_Mantle_Low_Left.A_Als_Mantle_Low_Left` | glTF | `assets/animations/Actions/Mantle/A_Als_Mantle_Low_Left.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Actions/Mantle/A_Als_Mantle_Low_Right.A_Als_Mantle_Low_Right` | glTF | `assets/animations/Actions/Mantle/A_Als_Mantle_Low_Right.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Actions/Roll/A_Als_Roll.A_Als_Roll` | glTF | `assets/animations/Actions/Roll/A_Als_Roll.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Air/Fall/A_Als_Fall.A_Als_Fall` | glTF | `assets/animations/Air/Fall/A_Als_Fall.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Air/Fall/A_Als_Fall_Fast.A_Als_Fall_Fast` | glTF | `assets/animations/Air/Fall/A_Als_Fall_Fast.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Air/Fall/A_Als_Flail.A_Als_Flail` | glTF | `assets/animations/Air/Fall/A_Als_Flail.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Air/Jump/A_Als_Jump_Loop.A_Als_Jump_Loop` | glTF | `assets/animations/Air/Jump/A_Als_Jump_Loop.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Air/Jump/A_Als_Jump_Run_Left.A_Als_Jump_Run_Left` | glTF | `assets/animations/Air/Jump/A_Als_Jump_Run_Left.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Air/Jump/A_Als_Jump_Run_Right.A_Als_Jump_Run_Right` | glTF | `assets/animations/Air/Jump/A_Als_Jump_Run_Right.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Air/Jump/A_Als_Jump_Walk_Left.A_Als_Jump_Walk_Left` | glTF | `assets/animations/Air/Jump/A_Als_Jump_Walk_Left.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Air/Jump/A_Als_Jump_Walk_Right.A_Als_Jump_Walk_Right` | glTF | `assets/animations/Air/Jump/A_Als_Jump_Walk_Right.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Air/Land/A_Als_Land_Heavy.A_Als_Land_Heavy` | glTF | `assets/animations/Air/Land/A_Als_Land_Heavy.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Air/Land/A_Als_Land_Heavy_Additive.A_Als_Land_Heavy_Additive` | glTF | `assets/animations/Air/Land/A_Als_Land_Heavy_Additive.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Air/Land/A_Als_Land_Light.A_Als_Land_Light` | glTF | `assets/animations/Air/Land/A_Als_Land_Light.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Air/Land/A_Als_Land_Light_Additive.A_Als_Land_Light_Additive` | glTF | `assets/animations/Air/Land/A_Als_Land_Light_Additive.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Air/Lean/A_Als_Air_Lean.A_Als_Air_Lean` | glTF | `assets/animations/Air/Lean/A_Als_Air_Lean.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Air/Lean/A_Als_Air_Lean_Backward.A_Als_Air_Lean_Backward` | glTF | `assets/animations/Air/Lean/A_Als_Air_Lean_Backward.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Air/Lean/A_Als_Air_Lean_Forward.A_Als_Air_Lean_Forward` | glTF | `assets/animations/Air/Lean/A_Als_Air_Lean_Forward.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Air/Lean/A_Als_Air_Lean_Left.A_Als_Air_Lean_Left` | glTF | `assets/animations/Air/Lean/A_Als_Air_Lean_Left.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Air/Lean/A_Als_Air_Lean_Right.A_Als_Air_Lean_Right` | glTF | `assets/animations/Air/Lean/A_Als_Air_Lean_Right.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Air/Lean/BS_Als_Air_Lean` | JSON(BlendSpace 定义) | `assets/animations/blendspaces/ALS/Animations/Air/Lean/BS_Als_Air_Lean.json` |
| 2026-08-22 | `/ALS/ALS/Animations/Base/A_Als_Crouch_Pose.A_Als_Crouch_Pose` | glTF | `assets/animations/Base/A_Als_Crouch_Pose.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Base/A_Als_Idle.A_Als_Idle` | glTF | `assets/animations/Base/A_Als_Idle.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Base/A_Als_Stand_Pose.A_Als_Stand_Pose` | glTF | `assets/animations/Base/A_Als_Stand_Pose.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/Acceleration/A_Als_Acceleration_Backward.A_Als_Acceleration_Backward` | glTF | `assets/animations/Grounded/Acceleration/A_Als_Acceleration_Backward.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/Acceleration/A_Als_Acceleration_Forward.A_Als_Acceleration_Forward` | glTF | `assets/animations/Grounded/Acceleration/A_Als_Acceleration_Forward.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/Acceleration/A_Als_Acceleration_Left.A_Als_Acceleration_Left` | glTF | `assets/animations/Grounded/Acceleration/A_Als_Acceleration_Left.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/Acceleration/A_Als_Acceleration_Right.A_Als_Acceleration_Right` | glTF | `assets/animations/Grounded/Acceleration/A_Als_Acceleration_Right.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Backward.A_Als_Crouch_Walk_Backward` | glTF | `assets/animations/Grounded/Crouch/A_Als_Crouch_Walk_Backward.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Forward.A_Als_Crouch_Walk_Forward` | glTF | `assets/animations/Grounded/Crouch/A_Als_Crouch_Walk_Forward.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Left_Backward.A_Als_Crouch_Walk_Left_Backward` | glTF | `assets/animations/Grounded/Crouch/A_Als_Crouch_Walk_Left_Backward.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Left_Forward.A_Als_Crouch_Walk_Left_Forward` | glTF | `assets/animations/Grounded/Crouch/A_Als_Crouch_Walk_Left_Forward.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Pose.A_Als_Crouch_Walk_Pose` | glTF | `assets/animations/Grounded/Crouch/A_Als_Crouch_Walk_Pose.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Right_Backward.A_Als_Crouch_Walk_Right_Backward` | glTF | `assets/animations/Grounded/Crouch/A_Als_Crouch_Walk_Right_Backward.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Right_Forward.A_Als_Crouch_Walk_Right_Forward` | glTF | `assets/animations/Grounded/Crouch/A_Als_Crouch_Walk_Right_Forward.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/Lean/A_Als_Lean.A_Als_Lean` | glTF | `assets/animations/Grounded/Lean/A_Als_Lean.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/Lean/A_Als_Lean_Backward.A_Als_Lean_Backward` | glTF | `assets/animations/Grounded/Lean/A_Als_Lean_Backward.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/Lean/A_Als_Lean_Forward.A_Als_Lean_Forward` | glTF | `assets/animations/Grounded/Lean/A_Als_Lean_Forward.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/Lean/A_Als_Lean_Left.A_Als_Lean_Left` | glTF | `assets/animations/Grounded/Lean/A_Als_Lean_Left.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/Lean/A_Als_Lean_Pose.A_Als_Lean_Pose` | glTF | `assets/animations/Grounded/Lean/A_Als_Lean_Pose.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/Lean/A_Als_Lean_Right.A_Als_Lean_Right` | glTF | `assets/animations/Grounded/Lean/A_Als_Lean_Right.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/Lean/BS_Als_Lean` | JSON(BlendSpace 定义) | `assets/animations/blendspaces/ALS/Animations/Grounded/Lean/BS_Als_Lean.json` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/Sprint/A_Als_Sprint.A_Als_Sprint` | glTF | `assets/animations/Grounded/Sprint/A_Als_Sprint.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/Sprint/A_Als_Sprint_Acceleration.A_Als_Sprint_Acceleration` | glTF | `assets/animations/Grounded/Sprint/A_Als_Sprint_Acceleration.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Backward.A_Als_Run_Backward` | glTF | `assets/animations/Grounded/WalkRun/A_Als_Run_Backward.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Forward.A_Als_Run_Forward` | glTF | `assets/animations/Grounded/WalkRun/A_Als_Run_Forward.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Forward_Pose.A_Als_Run_Forward_Pose` | glTF | `assets/animations/Grounded/WalkRun/A_Als_Run_Forward_Pose.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Left_Backward.A_Als_Run_Left_Backward` | glTF | `assets/animations/Grounded/WalkRun/A_Als_Run_Left_Backward.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Left_Forward.A_Als_Run_Left_Forward` | glTF | `assets/animations/Grounded/WalkRun/A_Als_Run_Left_Forward.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Left_Pose.A_Als_Run_Left_Pose` | glTF | `assets/animations/Grounded/WalkRun/A_Als_Run_Left_Pose.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Right_Backward.A_Als_Run_Right_Backward` | glTF | `assets/animations/Grounded/WalkRun/A_Als_Run_Right_Backward.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Right_Forward.A_Als_Run_Right_Forward` | glTF | `assets/animations/Grounded/WalkRun/A_Als_Run_Right_Forward.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Right_Pose.A_Als_Run_Right_Pose` | glTF | `assets/animations/Grounded/WalkRun/A_Als_Run_Right_Pose.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Backward.A_Als_Walk_Backward` | glTF | `assets/animations/Grounded/WalkRun/A_Als_Walk_Backward.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Forward.A_Als_Walk_Forward` | glTF | `assets/animations/Grounded/WalkRun/A_Als_Walk_Forward.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Forward_Pose.A_Als_Walk_Forward_Pose` | glTF | `assets/animations/Grounded/WalkRun/A_Als_Walk_Forward_Pose.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Left_Backward.A_Als_Walk_Left_Backward` | glTF | `assets/animations/Grounded/WalkRun/A_Als_Walk_Left_Backward.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Left_Forward.A_Als_Walk_Left_Forward` | glTF | `assets/animations/Grounded/WalkRun/A_Als_Walk_Left_Forward.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Left_Pose.A_Als_Walk_Left_Pose` | glTF | `assets/animations/Grounded/WalkRun/A_Als_Walk_Left_Pose.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Right_Backward.A_Als_Walk_Right_Backward` | glTF | `assets/animations/Grounded/WalkRun/A_Als_Walk_Right_Backward.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Right_Forward.A_Als_Walk_Right_Forward` | glTF | `assets/animations/Grounded/WalkRun/A_Als_Walk_Right_Forward.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Right_Pose.A_Als_Walk_Right_Pose` | glTF | `assets/animations/Grounded/WalkRun/A_Als_Walk_Right_Pose.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/WalkRun/BS_Als_WalkRun_Backward` | JSON(BlendSpace 定义) | `assets/animations/blendspaces/ALS/Animations/Grounded/WalkRun/BS_Als_WalkRun_Backward.json` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/WalkRun/BS_Als_WalkRun_Forward` | JSON(BlendSpace 定义) | `assets/animations/blendspaces/ALS/Animations/Grounded/WalkRun/BS_Als_WalkRun_Forward.json` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/WalkRun/BS_Als_WalkRun_Left_Backward` | JSON(BlendSpace 定义) | `assets/animations/blendspaces/ALS/Animations/Grounded/WalkRun/BS_Als_WalkRun_Left_Backward.json` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/WalkRun/BS_Als_WalkRun_Left_Forward` | JSON(BlendSpace 定义) | `assets/animations/blendspaces/ALS/Animations/Grounded/WalkRun/BS_Als_WalkRun_Left_Forward.json` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/WalkRun/BS_Als_WalkRun_Right_Backward` | JSON(BlendSpace 定义) | `assets/animations/blendspaces/ALS/Animations/Grounded/WalkRun/BS_Als_WalkRun_Right_Backward.json` |
| 2026-08-22 | `/ALS/ALS/Animations/Grounded/WalkRun/BS_Als_WalkRun_Right_Forward` | JSON(BlendSpace 定义) | `assets/animations/blendspaces/ALS/Animations/Grounded/WalkRun/BS_Als_WalkRun_Right_Forward.json` |
| 2026-08-22 | `/ALS/ALS/Animations/Overlays/Binoculars/A_Als_Binoculars_Aim.A_Als_Binoculars_Aim` | glTF | `assets/animations/Overlays/Binoculars/A_Als_Binoculars_Aim.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Overlays/Binoculars/A_Als_Binoculars_Aim_Crouch.A_Als_Binoculars_Aim_Crouch` | glTF | `assets/animations/Overlays/Binoculars/A_Als_Binoculars_Aim_Crouch.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Overlays/Binoculars/A_Als_Binoculars_Poses.A_Als_Binoculars_Poses` | glTF | `assets/animations/Overlays/Binoculars/A_Als_Binoculars_Poses.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Overlays/Binoculars/A_Als_Binoculars_Poses_NoCurves.A_Als_Binoculars_Poses_NoCurves` | glTF | `assets/animations/Overlays/Binoculars/A_Als_Binoculars_Poses_NoCurves.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Overlays/Bow/A_Als_Bow_Aim.A_Als_Bow_Aim` | glTF | `assets/animations/Overlays/Bow/A_Als_Bow_Aim.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Overlays/Bow/A_Als_Bow_Aim_Crouch.A_Als_Bow_Aim_Crouch` | glTF | `assets/animations/Overlays/Bow/A_Als_Bow_Aim_Crouch.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Overlays/Bow/A_Als_Bow_Poses.A_Als_Bow_Poses` | glTF | `assets/animations/Overlays/Bow/A_Als_Bow_Poses.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Overlays/Bow/A_Als_Bow_Poses_NoCurves.A_Als_Bow_Poses_NoCurves` | glTF | `assets/animations/Overlays/Bow/A_Als_Bow_Poses_NoCurves.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Overlays/Other/A_Als_Barrel_Poses.A_Als_Barrel_Poses` | glTF | `assets/animations/Overlays/Other/A_Als_Barrel_Poses.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Overlays/Other/A_Als_Box_Poses.A_Als_Box_Poses` | glTF | `assets/animations/Overlays/Other/A_Als_Box_Poses.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Overlays/Other/A_Als_Default_Poses.A_Als_Default_Poses` | glTF | `assets/animations/Overlays/Other/A_Als_Default_Poses.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Overlays/Other/A_Als_Feminine_Poses.A_Als_Feminine_Poses` | glTF | `assets/animations/Overlays/Other/A_Als_Feminine_Poses.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Overlays/Other/A_Als_HandsTied_Poses.A_Als_HandsTied_Poses` | glTF | `assets/animations/Overlays/Other/A_Als_HandsTied_Poses.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Overlays/Other/A_Als_Injured_Poses.A_Als_Injured_Poses` | glTF | `assets/animations/Overlays/Other/A_Als_Injured_Poses.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Overlays/Other/A_Als_Masculine_Poses.A_Als_Masculine_Poses` | glTF | `assets/animations/Overlays/Other/A_Als_Masculine_Poses.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Overlays/PistolOneHanded/A_Als_PistolOneHanded_Aim.A_Als_PistolOneHanded_Aim` | glTF | `assets/animations/Overlays/PistolOneHanded/A_Als_PistolOneHanded_Aim.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Overlays/PistolOneHanded/A_Als_PistolOneHanded_Aim_Crouch.A_Als_PistolOneHanded_Aim_Crouch` | glTF | `assets/animations/Overlays/PistolOneHanded/A_Als_PistolOneHanded_Aim_Crouch.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Overlays/PistolOneHanded/A_Als_PistolOneHanded_Poses.A_Als_PistolOneHanded_Poses` | glTF | `assets/animations/Overlays/PistolOneHanded/A_Als_PistolOneHanded_Poses.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Overlays/PistolOneHanded/A_Als_PistolOneHanded_Poses_NoCurves.A_Als_PistolOneHanded_Poses_NoCurves` | glTF | `assets/animations/Overlays/PistolOneHanded/A_Als_PistolOneHanded_Poses_NoCurves.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Overlays/PistolTwoHanded/A_Als_PistolTwoHanded_Aim.A_Als_PistolTwoHanded_Aim` | glTF | `assets/animations/Overlays/PistolTwoHanded/A_Als_PistolTwoHanded_Aim.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Overlays/PistolTwoHanded/A_Als_PistolTwoHanded_Aim_Crouch.A_Als_PistolTwoHanded_Aim_Crouch` | glTF | `assets/animations/Overlays/PistolTwoHanded/A_Als_PistolTwoHanded_Aim_Crouch.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Overlays/PistolTwoHanded/A_Als_PistolTwoHanded_Poses.A_Als_PistolTwoHanded_Poses` | glTF | `assets/animations/Overlays/PistolTwoHanded/A_Als_PistolTwoHanded_Poses.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Overlays/PistolTwoHanded/A_Als_PistolTwoHanded_Poses_NoCurves.A_Als_PistolTwoHanded_Poses_NoCurves` | glTF | `assets/animations/Overlays/PistolTwoHanded/A_Als_PistolTwoHanded_Poses_NoCurves.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Overlays/Rifle/A_Als_Rifle_Aim.A_Als_Rifle_Aim` | glTF | `assets/animations/Overlays/Rifle/A_Als_Rifle_Aim.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Overlays/Rifle/A_Als_Rifle_Aim_Crouch.A_Als_Rifle_Aim_Crouch` | glTF | `assets/animations/Overlays/Rifle/A_Als_Rifle_Aim_Crouch.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Overlays/Rifle/A_Als_Rifle_Poses.A_Als_Rifle_Poses` | glTF | `assets/animations/Overlays/Rifle/A_Als_Rifle_Poses.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Overlays/Rifle/A_Als_Rifle_Poses_NoCurves.A_Als_Rifle_Poses_NoCurves` | glTF | `assets/animations/Overlays/Rifle/A_Als_Rifle_Poses_NoCurves.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Overlays/Rifle/A_Als_Rifle_Run.A_Als_Rifle_Run` | glTF | `assets/animations/Overlays/Rifle/A_Als_Rifle_Run.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Overlays/Rifle/A_Als_Rifle_Run_Arms.A_Als_Rifle_Run_Arms` | glTF | `assets/animations/Overlays/Rifle/A_Als_Rifle_Run_Arms.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Overlays/Rifle/A_Als_Rifle_Sprint.A_Als_Rifle_Sprint` | glTF | `assets/animations/Overlays/Rifle/A_Als_Rifle_Sprint.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Overlays/Rifle/A_Als_Rifle_Sprint_Arms.A_Als_Rifle_Sprint_Arms` | glTF | `assets/animations/Overlays/Rifle/A_Als_Rifle_Sprint_Arms.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Overlays/Rifle/A_Als_Rifle_Sprint_Impulse.A_Als_Rifle_Sprint_Impulse` | glTF | `assets/animations/Overlays/Rifle/A_Als_Rifle_Sprint_Impulse.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Overlays/Rifle/A_Als_Rifle_Sprint_Impulse_Arms.A_Als_Rifle_Sprint_Impulse_Arms` | glTF | `assets/animations/Overlays/Rifle/A_Als_Rifle_Sprint_Impulse_Arms.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Overlays/Torch/A_Als_Torch_Aim.A_Als_Torch_Aim` | glTF | `assets/animations/Overlays/Torch/A_Als_Torch_Aim.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Overlays/Torch/A_Als_Torch_Aim_Crouch.A_Als_Torch_Aim_Crouch` | glTF | `assets/animations/Overlays/Torch/A_Als_Torch_Aim_Crouch.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Overlays/Torch/A_Als_Torch_Poses.A_Als_Torch_Poses` | glTF | `assets/animations/Overlays/Torch/A_Als_Torch_Poses.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Overlays/Torch/A_Als_Torch_Poses_NoCurves.A_Als_Torch_Poses_NoCurves` | glTF | `assets/animations/Overlays/Torch/A_Als_Torch_Poses_NoCurves.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/RotateInPlace/A_Als_Crouch_Rotate_90_Left.A_Als_Crouch_Rotate_90_Left` | glTF | `assets/animations/RotateInPlace/A_Als_Crouch_Rotate_90_Left.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/RotateInPlace/A_Als_Crouch_Rotate_90_Right.A_Als_Crouch_Rotate_90_Right` | glTF | `assets/animations/RotateInPlace/A_Als_Crouch_Rotate_90_Right.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/RotateInPlace/A_Als_Rotate_90_Left.A_Als_Rotate_90_Left` | glTF | `assets/animations/RotateInPlace/A_Als_Rotate_90_Left.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/RotateInPlace/A_Als_Rotate_90_Right.A_Als_Rotate_90_Right` | glTF | `assets/animations/RotateInPlace/A_Als_Rotate_90_Right.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Transitions/A_Als_CrouchToStand.A_Als_CrouchToStand` | glTF | `assets/animations/Transitions/A_Als_CrouchToStand.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Transitions/A_Als_Crouch_DynamicTransition_Left.A_Als_Crouch_DynamicTransition_Left` | glTF | `assets/animations/Transitions/A_Als_Crouch_DynamicTransition_Left.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Transitions/A_Als_Crouch_DynamicTransition_Right.A_Als_Crouch_DynamicTransition_Right` | glTF | `assets/animations/Transitions/A_Als_Crouch_DynamicTransition_Right.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Transitions/A_Als_Crouch_Transition_Left.A_Als_Crouch_Transition_Left` | glTF | `assets/animations/Transitions/A_Als_Crouch_Transition_Left.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Transitions/A_Als_Crouch_Transition_Right.A_Als_Crouch_Transition_Right` | glTF | `assets/animations/Transitions/A_Als_Crouch_Transition_Right.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Transitions/A_Als_StandToCrouch.A_Als_StandToCrouch` | glTF | `assets/animations/Transitions/A_Als_StandToCrouch.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Transitions/A_Als_Stand_DynamicTransition_Left.A_Als_Stand_DynamicTransition_Left` | glTF | `assets/animations/Transitions/A_Als_Stand_DynamicTransition_Left.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Transitions/A_Als_Stand_DynamicTransition_Right.A_Als_Stand_DynamicTransition_Right` | glTF | `assets/animations/Transitions/A_Als_Stand_DynamicTransition_Right.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Transitions/A_Als_Stand_Transition_Left.A_Als_Stand_Transition_Left` | glTF | `assets/animations/Transitions/A_Als_Stand_Transition_Left.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Transitions/A_Als_Stand_Transition_Right.A_Als_Stand_Transition_Right` | glTF | `assets/animations/Transitions/A_Als_Stand_Transition_Right.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Transitions/A_Als_Stop_Left.A_Als_Stop_Left` | glTF | `assets/animations/Transitions/A_Als_Stop_Left.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/Transitions/A_Als_Stop_Right.A_Als_Stop_Right` | glTF | `assets/animations/Transitions/A_Als_Stop_Right.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/TurnInPlace/A_Als_Crouch_Turn_180_Left.A_Als_Crouch_Turn_180_Left` | glTF | `assets/animations/TurnInPlace/A_Als_Crouch_Turn_180_Left.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/TurnInPlace/A_Als_Crouch_Turn_180_Right.A_Als_Crouch_Turn_180_Right` | glTF | `assets/animations/TurnInPlace/A_Als_Crouch_Turn_180_Right.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/TurnInPlace/A_Als_Crouch_Turn_90_Left.A_Als_Crouch_Turn_90_Left` | glTF | `assets/animations/TurnInPlace/A_Als_Crouch_Turn_90_Left.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/TurnInPlace/A_Als_Crouch_Turn_90_Right.A_Als_Crouch_Turn_90_Right` | glTF | `assets/animations/TurnInPlace/A_Als_Crouch_Turn_90_Right.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/TurnInPlace/A_Als_Turn_180_Left.A_Als_Turn_180_Left` | glTF | `assets/animations/TurnInPlace/A_Als_Turn_180_Left.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/TurnInPlace/A_Als_Turn_180_Right.A_Als_Turn_180_Right` | glTF | `assets/animations/TurnInPlace/A_Als_Turn_180_Right.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/TurnInPlace/A_Als_Turn_90_Left.A_Als_Turn_90_Left` | glTF | `assets/animations/TurnInPlace/A_Als_Turn_90_Left.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/TurnInPlace/A_Als_Turn_90_Right.A_Als_Turn_90_Right` | glTF | `assets/animations/TurnInPlace/A_Als_Turn_90_Right.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/View/A_Als_Look_Down.A_Als_Look_Down` | glTF | `assets/animations/View/A_Als_Look_Down.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/View/A_Als_Look_Forward.A_Als_Look_Forward` | glTF | `assets/animations/View/A_Als_Look_Forward.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/View/A_Als_Look_Up.A_Als_Look_Up` | glTF | `assets/animations/View/A_Als_Look_Up.gltf` |
| 2026-08-22 | `/ALS/ALS/Animations/View/BS_Als_Look` | JSON(BlendSpace 定义) | `assets/animations/blendspaces/ALS/Animations/View/BS_Als_Look.json` |
| 2026-08-22 | `/ALS/ALS/Character/SKM_Als.SKM_Als` | glTF | `assets/models/Character/SKM_Als.gltf` |
| 2026-08-22 | `/ALS/ALS/Data/AnimationInstance/Air/CF_Als_GroundPredictionAmount` | JSON(均匀采样) | `assets/curves/ALS/Data/AnimationInstance/Air/CF_Als_GroundPredictionAmount.json` |
| 2026-08-22 | `/ALS/ALS/Data/AnimationInstance/Air/CF_Als_LeanAmount_Air` | JSON(均匀采样) | `assets/curves/ALS/Data/AnimationInstance/Air/CF_Als_LeanAmount_Air.json` |
| 2026-08-22 | `/ALS/ALS/Data/AnimationInstance/Blending/CF_Als_Aim_In` | JSON(均匀采样) | `assets/curves/ALS/Data/AnimationInstance/Blending/CF_Als_Aim_In.json` |
| 2026-08-22 | `/ALS/ALS/Data/AnimationInstance/Blending/CF_Als_Aim_Out` | JSON(均匀采样) | `assets/curves/ALS/Data/AnimationInstance/Blending/CF_Als_Aim_Out.json` |
| 2026-08-22 | `/ALS/ALS/Data/AnimationInstance/Blending/CF_Als_StanceChange` | JSON(均匀采样) | `assets/curves/ALS/Data/AnimationInstance/Blending/CF_Als_StanceChange.json` |
| 2026-08-22 | `/ALS/ALS/Data/AnimationInstance/Grounded/CF_Als_RotationYawOffset_ForwardBackward` | JSON(均匀采样) | `assets/curves/ALS/Data/AnimationInstance/Grounded/CF_Als_RotationYawOffset_ForwardBackward.json` |
| 2026-08-22 | `/ALS/ALS/Data/AnimationInstance/Grounded/CF_Als_RotationYawOffset_Left` | JSON(均匀采样) | `assets/curves/ALS/Data/AnimationInstance/Grounded/CF_Als_RotationYawOffset_Left.json` |
| 2026-08-22 | `/ALS/ALS/Data/AnimationInstance/Grounded/CF_Als_RotationYawOffset_Right` | JSON(均匀采样) | `assets/curves/ALS/Data/AnimationInstance/Grounded/CF_Als_RotationYawOffset_Right.json` |
| 2026-08-22 | `/ALS/ALS/Data/AnimationInstance/Grounded/CF_Als_StrideBlendAmount_Run` | JSON(均匀采样) | `assets/curves/ALS/Data/AnimationInstance/Grounded/CF_Als_StrideBlendAmount_Run.json` |
| 2026-08-22 | `/ALS/ALS/Data/AnimationInstance/Grounded/CF_Als_StrideBlendAmount_Walk` | JSON(均匀采样) | `assets/curves/ALS/Data/AnimationInstance/Grounded/CF_Als_StrideBlendAmount_Walk.json` |
| 2026-08-22 | `/ALS/ALS/Data/Character/Movement/CF_Als_RotationInterpolationSpeed_Normal` | JSON(均匀采样) | `assets/curves/ALS/Data/Character/Movement/CF_Als_RotationInterpolationSpeed_Normal.json` |
| 2026-08-22 | `/ALS/ALS/Data/Character/Movement/CF_Als_RotationInterpolationSpeed_Responsive` | JSON(均匀采样) | `assets/curves/ALS/Data/Character/Movement/CF_Als_RotationInterpolationSpeed_Responsive.json` |
| 2026-08-22 | `/ALS/ALS/Data/Character/Movement/CF_Als_RotationInterpolationSpeed_Sluggish` | JSON(均匀采样) | `assets/curves/ALS/Data/Character/Movement/CF_Als_RotationInterpolationSpeed_Sluggish.json` |
| 2026-08-22 | `/ALS/ALS/OverlayObjects/Bow/A_Als_Bow_Draw.A_Als_Bow_Draw` | glTF | `assets/animations/Overlays/Bow/A_Als_Bow_Draw.gltf` |
| 2026-08-22 | `/ALS/ALS/OverlayObjects/Bow/SKM_Als_Bow.SKM_Als_Bow` | glTF | `assets/models/Character/SKM_Als_Bow.gltf` |
| 2026-08-22 | `/ALS/ALS/OverlayObjects/Pistol/SKM_Als_Pistol.SKM_Als_Pistol` | glTF | `assets/models/Character/SKM_Als_Pistol.gltf` |
| 2026-08-22 | `/ALS/ALS/OverlayObjects/Rifle/SKM_Als_Rifle.SKM_Als_Rifle` | glTF | `assets/models/Character/SKM_Als_Rifle.gltf` |
| 2026-08-22 | `/ALS/ALSCamera/Data/CF_Als_CameraBlend_Quick` | JSON(均匀采样) | `assets/curves/ALSCamera/Data/CF_Als_CameraBlend_Quick.json` |
| 2026-08-22 | `/ALS/ALSCamera/Data/CF_Als_CameraBlend_Smooth` | JSON(均匀采样) | `assets/curves/ALSCamera/Data/CF_Als_CameraBlend_Smooth.json` |
| 2026-08-22 | `/ALS/ALSCamera/SKM_Als_Camera.SKM_Als_Camera` | glTF | `assets/models/ALSCamera/SKM_Als_Camera.gltf` |
| 2026-08-22 | `/ALS/ALSExtras/Environment/CF_Als_MovingObject` | JSON(均匀采样) | `assets/curves/ALSExtras/Environment/CF_Als_MovingObject.json` |

## M3.1 增补导出（2026-08-22）— Playground 场景

- 依据：CHECKLIST M3.1（场景复刻增补条目，用户指示）；通道同上 commandlet。
- 内容：ALSExtras/Levels/L_Als_Playground 内 20 个唯一 StaticMesh → ssets/models/playground/*.gltf+.bin
  （SM_Als_Arch_01/02、Barrel(_Broken)、Bench、Block_2x2/3x3、Cart、Crate(_Broken)、
  Floor_20x20、Fountain_01、Pillar、Rock、Stairs_01/02/03、TitleBoard、Wall_2x20/3x5）；
  场景描述 	mp/export/playground/playground_actors.json（219 actor 全量 transform/bounds/mesh 映射，
  运行时清单经 als-ext PlaygroundPlugin 以 RON 直读，JSON 原件为产物留档）。
- 跳过：Landscape（bevy 无等价方案，按条目约定省略）、Sky 系、TextRender/AI/NavMesh 等
  非碰撞装饰与系统 actor；Brush(BSP) 以 actor bounds 盒体近似。