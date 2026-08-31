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
| 2026-08-28 | `/ALS/ALS/Character/SKM_Als` camera sockets `FirstPersonCamera`/`ThirdPersonTraceShoulderLeft`/`ThirdPersonTraceShoulderRight` | RON runtime table | `assets/params/als_camera_sockets.ron` |
| 2026-08-28 | `/ALS/ALSCamera/AB_Als_Camera` ModifyCurve source profiles | RON runtime table | `assets/params/als_camera_profiles.ron` |
| 2026-08-22 | `/ALS/ALSExtras/Environment/CF_Als_MovingObject` | JSON(均匀采样) | `assets/curves/ALSExtras/Environment/CF_Als_MovingObject.json` |

## M3.1 增补导出（2026-08-22）— Playground 场景

- 依据：CHECKLIST M3.1（场景复刻增补条目，用户指示）；通道同上 commandlet。
- 内容：ALSExtras/Levels/L_Als_Playground 内 20 个唯一 StaticMesh → assets/models/playground/*.gltf+.bin
  （SM_Als_Arch_01/02、Barrel(_Broken)、Bench、Block_2x2/3x3、Cart、Crate(_Broken)、
  Floor_20x20、Fountain_01、Pillar、Rock、Stairs_01/02/03、TitleBoard、Wall_2x20/3x5）；
  场景描述 tmp/export/playground/playground_actors.json（219 actor 全量 transform/bounds/mesh 映射，
  运行时清单经 als-ext PlaygroundPlugin 以 RON 直读，JSON 原件为产物留档）。
- 跳过：Landscape（bevy 无等价方案，按条目约定省略）、Sky 系、TextRender/AI/NavMesh 等
  非碰撞装饰与系统 actor；Brush(BSP) 以 actor bounds 盒体近似。

## M0.5b 增补提取（2026-08-23）— 动画内嵌浮点曲线 → glTF `ALS_float_curves` 扩展

- 依据：CHECKLIST M0.5b（用户指示增补）。引擎 glTF 导出器不写 AnimSequence 内嵌浮点曲线
  （M2.4 已核实），本条目以独立工具从源 .uasset 直接提取并回写进既有动画 glTF。
- 工具：`tools/uasset_curve_export/`（Python stdlib，无引擎依赖）
  - `uasset.py` — UE5.8 包格式解析：FPackageFileSummary（legacy -9 / UE4 522 / UE5 1018，
    SavedHash+FIoHash、CustomVersions Optimized 格式）、名字表（FString+双 u16 hash）、
    Import/Export 表（含 SCRIPT_SERIALIZATION_OFFSET 尾部字段）、版本化标签属性
    （PROPERTY_TAG_COMPLETE_TYPE_NAME：Name FName + TypeName 节点链{Name FName,InnerCount i32} +
    Size **i32** + Flags u8 + 可选 ArrayIndex/Guid/Extensions；BoolProperty 值在 tag flag 0x10）。
  - `extract_curves.py` — 定向走 `UAnimSequenceBase.RawCurveData(FRawCurveTracks).FloatCurves`
    →`FFloatCurve{CurveName,CurveTypeFlags,FloatCurve(FRichCurve)}`；FRichCurveKey 为原生序列化
    （WithSerializer）：u8×3(Interp/Tangent/WeightMode)+f32×6(Time,Value,Arrive,ArriveW,Leave,LeaveW)=27B；
    枚举属性（EnumProperty 及带枚举参数的 ByteProperty）payload 为 **FName**（8B）非 u8。
  - `export_curve_gltf.py` — CLI：扫描工程 Content 根全部 uasset→按对象名 stem 匹配
    `assets/animations/**.gltf`（与 M0.5 文件一一对应）→写回。
  - `validate_gltf.py` — 回读校验：JSON 合法性、单一 animation、times/values/interps 等长且时间单调。
- 提取范围与结果（2026-08-23 实测）：扫描 828 个 uasset，识别 127 个 AnimSequence 并全部匹配到
  既有 glTF；**82 个资产含曲线，共 367 条**。曲线族：
  LayerArmLeft/Right(+Additive/LocalSpace)、LayerHandLeft/Right、LayerHead(+Additive)、LayerLegs、
  LayerPelvis、LayerSpine(+Additive)、HandLeftIk/HandRightIk、HipsDirectionLock、PoseAiming、
  PoseGait、PoseCrouching/PoseStanding、FootPlanted、FootLeftLock/FootRightLock、FeetCrossing、
  RotationYawSpeed、AllowTransitions、FootstepSoundBlock。抽查数值符合 ALS 语义
  （Rotate_90_Left.RotationYawSpeed 31 键积分≈-90°/s·s；Walk_Forward.FootPlanted ∈[-0.25,1] 相位；
  Rifle_Poses.LayerHandRight 恒 1）。
- glTF 扩展 schema（写入 animation 对象，root `extensionsUsed` 追加 `"ALS_float_curves"`，
  不加入 extensionsRequired；bevy_gltf/gltf-json 对未知扩展静默忽略，不影响现有加载）：

```json
"extensions": { "ALS_float_curves": { "curves": [ {
  "name": "RotationYawSpeed",
  "flags": 4,
  "default": null,
  "pre_infinity": "RCCE_Constant",
  "post_infinity": "RCCE_Constant",
  "interps": ["LINEAR", "..."],
  "times":  [/* f32 秒，域=clip 时间 */],
  "values": [/* f32 */]
} ] } }
```

  字段语义：`flags`=EAnimAssetCurveFlags 位掩码；`default=null` 表示 UE 的 FLT_MAX「未设置」，
  消费端按 0 处理；`pre_infinity/post_infinity`=ERichCurveExtrapolation 名
  （RCCE_Cycle/CycleWithOffset/Oscillate/Linear/Constant/None）；`interps[i]`∈
  LINEAR/CONSTANT/CUBIC/NONE 对应 FRichCurveKey.InterpMode 0..3；当任一键为 CUBIC 或带权重时
  额外输出 `tangent_modes/weight_modes/arrive_tangents/leave_tangents/arrive_weights/leave_weights`。
- 后续摄取路径（不改 fork）：bevy_gltf 0.19 `GltfExtensionHandler::on_animation` +
  `gltf::Animation::extensions()` 在 als-ext 注册 handler 即可把曲线注入运行时通道，
  替代 M6.2/M6.3 的 CF_Als_Aim_In/Out 与静态骨骼 mask 替代方案（迁移条目归入相应模块）。
## M9.1 Ragdoll 资产生成（2026-08-23）

- 工具：`tools/import/gen_ragdoll.py`（python stdlib）——解析 `animations/Base/A_Als_Idle.gltf` 的 `skins[0].inverseBindMatrices`（float4x4 列主序，rigid 逆）得骨骼绑定姿态世界矩阵；按 16-body 配置表生成两份资产：
- `assets/ragdolls/als_manny.rag.ron`：16 刚体（head/chest/stomach/pelvis/双臂×3/双腿×3）各挂一个 Y 轴 Capsule collider（ByVolume 分配 total_mass=75）、15 个 SphericalJoint（position=解剖锚骨世界坐标，零 compliance）；全部 default_mode=Dynamic。
- `assets/ragdoll_bone_maps/als_manny.bm.ron`：22 根骨骼→body 权重映射（weight=1.0，offset=body_rest⁻¹∘bone_rest 完整 isometry，override_offset=false），skeleton 指向 `skeletons/SKM_Als.skn.ron`。
- 已知偏离：capsule 未沿骨骼长轴旋转（Y 轴对齐世界），四肢斜段覆盖以半径补偿；未列骨骼（手指等）不参与 readback。

## M8.6 Root Motion 回填（2026-08-24）— 编辑器 uasset 根骨骼原始轨道 → glTF root 通道

- 依据：CHECKLIST M8.6（root motion 数据回填）。引擎 glTF 导出器把 root 节点平移轨道写零，
  本条目从源编辑器 .uasset（非 Cooked，`<als-content>` 下 Plugins Content）直接解码根骨骼
  RawTrack 并回写 4 个 clip 的 glTF/bin，使 mantle/roll 的位移驱动来自真实数据。
- 工具：`tools/uasset_root_export/`（Python stdlib；复用 `tools/uasset_curve_export/uasset.py`
  的包格式解析，未复制）
  - `probe_animdatamodel.py` — AnimDataModel 导出枚举、标签属性布局打印、payload 十六进制转储、
    序列化布局判定（保留的诊断工件，可独立运行）。
  - `extract_root_track.py` — 库+CLI：定位 AnimDataModel 导出→解码 BoneAnimationTracks→
    取 `Name=="root"` 轨道→按 FrameRate 构建均匀时间栅格→坐标变换→追加 bufferView/accessor
    并重指 root 平移/旋转 sampler。
  - `validate_root_tracks.py` — 仅读回写后 glTF：积分 root 平移得净位移/水平幅值/垂直增量/
    单调性注记，对照 PASS 判据输出逐项结论与总判定（exit code 非 0 表示有越界项）。
- 源对象路径与序列化事实（实测）：`UAnimDataModel.BoneAnimationTracks[root].InternalTrackData`
  （导出类经 Import 解析：class_name=`Class`、object_name=`AnimDataModel`）。本引擎构建的
  FPropertyTag 为 Name FName + TypeName 节点链{Name FName,InnerCount i32}（结构名节点以包名叶收尾，
  如 `.RawAnimSequenceTrack./Script/Engine`）+ Size i32 + Flags u8。`InternalTrackData` tag
  Flags=0x08（HasBinaryOrNativeSerialize），其 payload 为 FRawAnimSequenceTrack 原生序列化：
  **BulkSerialize 布局** `[i12 ElementSize][i32 ArrayNum][ArrayNum*ElementSize 原始小端浮点]`×3
  （PosKeys 12B、RotKeys 16B xyzw、ScaleKeys 12B）；证据偏移 A_Als_Mantle_High.uasset 0x33D9：
  `0C 00 00 00 | 51 00 00 00 | <81*3f…>`。元素内其余字段（BoneTreeIndex IntProperty、
  Name NameProperty）仍为标签属性；数组 payload 首端为 i32 轨道数（68），逐元素流以 None 收尾，
  精确耗尽属性区间。元数据实测：FrameRate.Numerator=30（Denominator=1 因等于默认被省略）、
  NumberOfFrames=80/45、NumberOfKeys=81/46，全部轨道键数一致。
- 坐标/单位变换（经验拟合，非理论假定）：以 pelvis 轨道 UE 键值 vs 既有 glTF pelvis 通道数值
  全量比对，在 48 个带符号轴置换中唯一胜出：**gltf = (ue.x, ue.z, ue.y) × 0.01**
  （Y/Z 轴交换 + cm→m，无符号翻转），位置最大误差 7.6e-08 m；旋转经 R_g = M·R_ue·Mᵀ
  共轭后再转四元数，误差 1.8e-07（pelvis rot[0] 逐分量吻合）。朝向核验：rest pose 下
  foot_l→ball_l（跟→趾）Δ=(+0.008,-0.107,+0.164)，角色面向 glTF **+Z**；Roll 净位移恰为
  glTF +Z 方向 2.75 m，即相对自身朝向向前滚动，符号正确。
- 写入 schema：向既有 .bin 追加两段（4 字节对齐）：root translation VEC3×N f32 与
  rotation VEC4×N f32；glTF 追加 2 个 bufferView + 2 个 accessor（含 min/max），并把
  animation[0] 中 target=node(root) 的 translation/rotation 两通道 sampler.output 重指到新
  accessor（共享 input 时间栅格不变，t_i=i/fps，与原 times 逐点一致 ≤1e-4 s）；其余内容
  （节点/蒙皮/mesh/材质/ALS_float_curves 扩展）保持不变，JSON 以原有制表符缩进+CRLF 重排
  （空跑下与原文件字节级 round-trip 一致）。root RotKeys 全为单位四元数（各 clip 同），照常写入。
- 回填范围与实测统计（validate_root_tracks 输出，2026-08-24）：

| clip | keys | 时长(s) | 净位移 glTF(x,y,z) m | 水平(m) | 垂直(m) |
|---|---|---|---|---|---|
| A_Als_Mantle_High | 81 | 2.6667 | (+0.0000, +2.0000, +0.6500) | 0.6500 | 2.0000 |
| A_Als_Mantle_Low_Left | 46 | 1.5000 | (+0.0000, +1.0000, +0.6500) | 0.6500 | 1.0000 |
| A_Als_Mantle_Low_Right | 46 | 1.5000 | (+0.0000, +1.0000, +0.6500) | 0.6500 | 1.0000 |
| A_Als_Roll | 46 | 1.5000 | (+0.0000, +0.0000, +2.7500) | 2.7500 | 0.0000 |

  判据核对：Roll 水平 ∈[2.0,4.0] PASS；Low 两支垂直 ∈[0.3,1.1] PASS；时长 2.667/1.5 不变 PASS；
  Mantle High 垂直实测 **2.0000 m**——源轨道终点恰为 UE Z=+200 cm（作者设定的 2 m 台面高度，
  Low 支同理精确 +100 cm），超出任务书暂定带宽 [1.0,1.8]，属判据标定偏差而非提取误差；
  建议后续将 High 带修订为 [1.8,2.2]。垂直全程单调无回落（y_descents=0），时间栅格严格均匀
  （dt=1/30 s）。
- CLI 用法（路径一律参数注入，禁入仓库）：

```
python tools/uasset_root_export/probe_animdatamodel.py --uasset <als-content>/ALS/Animations/<...>.uasset
python tools/uasset_root_export/extract_root_track.py --als-content <als-content> --repo-root <repo-root> [--apply]
python tools/uasset_root_export/validate_root_tracks.py --repo-root <repo-root>
```

## BF-Scene 增补导出（2026-08-26）— Playground 材质与地形

- 依据：用户指令（L_Als_Playground 精准复刻：材质贴图/法线 PBR、动态体行为、TextRender、光照、
  Landscape 高度场；agents.md 规则 12 实时提取）。编辑器 MCP + commandlet 双通道。
- 工具：`tools/export/export_materials_textures.py`（commandlet，MaterialEditingLibrary
  get_material_expressions 节点图遍历 + AssetExportTask 贴图 PNG 导出）；
  场景行为事实经在线 MCP `read_graph_dsl` 反汇编 B_Als_MovingObject/B_Als_RotatingObject 事件图。
- 内容：
  - `assets/textures/playground/T_Als_Tiles_M.png` ← `/ALS/ALSExtras/Environment/Materials/T_Als_Tiles_M`
    （2048² 线性 Mask；G 通道为 M_Als_Tiles Ambient Occlusion，运行时映射到 Bevy occlusion R）
  - `assets/textures/playground/T_Als_Tiles_N.png` ← 同目录 `T_Als_Tiles_N`（2048² 线性 DirectX 法线贴图，运行时启用 Y 翻转）
  - `assets/textures/playground/T_Default_Material_Grid_M.png` ← `/Engine/EngineMaterials/T_Default_Material_Grid_M`
    （512² 线性网格；M_Als_Tiles 以 UV×Tiling×2 采样其 G 作为两组颜色插值、A 乘到底色）
  - `tmp/export/playground/materials/materials.json` — M_Als_Tiles/M_Als_Main/M_Als_Transluent 全节点图
    （M_Als_Tiles: Tiling 标量、SideColorA/B+TopColorA/B、Grid G/A、T_Als_Tiles_N、T_Als_Tiles_M.G；
    M_Als_Main: 单 Color 参数；M_Als_Transluent: VertexNormalWS.B×0.5 opacity）
  - `assets/models/playground/playground_scene.ron` — 场景清单 v2：219 actor + 22 TextRender
    (text/color/world_size 实值) + 150 StaticObject 实例配色（TopColorA 实测：蓝126/绿16/橙4/
    红2/蓝紫2，Side 恒白）+ Moving(1.25s/1.75s ping-pong)/Rotating(yaw=100°/s) 参数 +
    sun rotator(pitch=-45,yaw=45) + Landscape 高度场（64×14 网格 trace_world 实采，
    足迹 x∈[-1000,1016]cm y∈[1000,3016]cm 步距 187.5cm，高度 0~250cm）
- 行为事实（蓝图反汇编）：Moving=InterpToMovement 沿样条 SetDuration(MovementDuration)，
  Tick 按 ServerWorldTime/spline.duration 取模 ping-pong，缓动 CF_Als_MovingObject 曲线；
  Rotating=每 Tick RelativeRotation=RotationSpeed×世界时间。
- 偏差登记：样条路径点不可反射读取（spline_curves 受保护），移动幅度以 2m 竖直往替代
  （原样条为局部短距往返）；贴图仅 _M/_N 两张（引擎无 ORM 合并图，roughness 用常量近似）。

## BF-Scene 勘误增补（2026-08-26）— Landscape glTF

- 依据：BF-Scene 清单条目批准，用户从 `L_Als_Playground` 的 `Landscape_1` 导出。
- 内容：
  - `assets/models/playground/Landscape1.gltf`
  - `assets/models/playground/Landscape1.bin`
  - `assets/models/playground/MI_Als_Landscape_BaseColor.png`
  - `assets/models/playground/MI_Als_Landscape_Normal.png`
  - `assets/models/playground/MI_Als_Landscape_Occlusion.png`
- 变换事实：glTF 根节点已编码 Bevy 坐标下的 translation=(-10,0.5,10)、scale=(16,16,16)，
  对应 UE actor location=(-1000,1000,50)cm、scale=(16,16,16)；运行时 wrapper 保持 identity，禁止重复应用 actor 变换。
- 边界：未导入或提交 `.uasset`，未导入字体资产。

## BF103 Notify 事件侧车数据（2026-08-30）

- 依据：BF103.01 本地 ALS 动画资产审计；源序列与 Montage 的 Notify、Notify State、AuthoredSyncMarker 均从 uasset 标签属性实时读取，不提交原始 uasset。
- 工具：`tools/import/audit_als_animation_events.py` 生成可复查的 `docs/bf103-notify-source-audit.{json,md}`；`tools/import/generate_als_notify_manifest.py` 从该审计生成并校验项目侧事件数据。
- 输出：`assets/animation_events/als_notify_manifest.json`（v1）。包含 137 个 source clip、177 条 Notify／Notify State、60 个同步 Marker 和 9 个 Montage→序列 segment 映射；时间、引用、GraphClip 路径和 source event ID 均由生成器验证。
- 编译：`tools/import/compile_als_event_tracks.py` 将 177 条 source event 确定性写入 65 个 `.anim.ron` GraphClip 的 98 个 EventTrack；point event 保留零长度精确时间，state event 保留原区间，Montage event 映射到其实际 sequence GraphClip。`--check` 验证生成物无漂移。
- 边界：AuthoredSyncMarker 的同步相位数据继续以 `assets/sync/als_sync_metadata.ron` 为运行时规范，不作为角色全局定时器；Marker crossed event 的运行时生命周期留待 BF103.04。

## BF109 CR_Als 原生审计与运行时材化数据（2026-09-01）

- 源对象：`/ALS/ALS/Character/CR_Als.CR_Als`、`/ALS/ALS/Character/SKM_Als.SKM_Als`。
- 原生读取：`tools/ue_graph_source_reader` 直接反射 `ControlRigBlueprint`、`UEdGraph`、`UEdGraphNode`、`UEdGraphPin`、RigVM link、Blueprint CDO 与 SkeletalMesh final bone hierarchy；原始输出只保留在 gitignored `target/bf109/source_native/`，不提交 uasset。
- 审计合同：`assets/control_rigs/cr_als.profile.json`。保留 10 个源 graph、133 个 node 的完整 position/property/pin/connection、1,407 个 pin、160 条 RigVM link、CDO 属性和 source graph layout；该文件只供离线审计，不在运行时解析。
- 运行时材化数据：`assets/control_rigs/cr_als.runtime.json`。由 `tools/import/coordinate_basis.py` 与 `tools/import/gen_cr_als_profile.py` 离线生成，仅含 target-space 骨骼名、米制长度、目标轴、rotation 和 CR 数值；运行时不执行 source/target basis bridge。
- 验证：`tools/import/audit_cr_als.py` 使用上述原生输出逐项验证完整图合同及全部 79 个 final skeleton transform/parent 和 11 根 virtual bone 对目标 glTF，报告零差异。
