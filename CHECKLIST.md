# bevy_als — 执行清单

> 本文件是唯一权威执行清单；执行纪律见 `agents.md`（仅规则），启动方式见 `goal.md`。
> 完成一项即勾选并一次提交：实现 → 自测 → 勾选 `[x]` → `git commit -m "<前缀><编号>: <条目摘要>"`（勾选与实现同一提交）。
> 基座模块提交前缀沿用 `M<编号>`；缺陷修复清单用 `BF<编号>`。

## 归档注记（2026-08-24）

- M0–M11 全部清单项与 Gate 已完成，整体归档至 [`archive/2026-08-24/`](archive/2026-08-24/)：
  - `CHECKLIST-M0-M11.md` —— 原 M0–M11 清单逐字快照（全部条目实现细节与 Gate 证据正文）；
  - `PROJECT-STATE.md` —— 状态浓缩（新会话恢复上下文的入口，先读它）；
  - `logs/` —— 全部门禁运行日志；`parity-report.md` —— 与 ALS 差异对照。
- 本文件此后只登记进行中的工作；已完成基座仅保留下方索引。附录 A/B 为规范性内容，持续有效。

## 背景与基础事实（已核实，持续有效）

- 本机 bevy 源码：`0.19.1`（release-0.19.1）。`bevy_animation_graph` 0.11.0 与 `bevy-tnua` 均基于 Bevy 0.19，兼容。
- bevy_animation_graph 已有节点：ClipNode、ChainNode、BlendNode（含骨骼 mask/Difference/Additive）、BlendSpace1D/2D、布尔运算节点等；图资产 RON 序列化、支持编辑器热重载；Pose 以 `BoneId` 路径表达；RootMotionMode 提取已可用（M8.8）。曲线/ramp/距离匹配等由 als-ext 经公开扩展点补齐。
- bevy-tnua 已有 basis：Walk（coyote time）、Jump（高度/悬挂控制）、Crouch、Dash、Climb、Knockback、WallSlide；物理后端 avian3d 0.12.1 / rapier3d 0.17.0。
- ALS-Refactored 关键结构：4 模块（ALS / ALSCamera / ALSExtras / ALSEditor）；状态体系用 GameplayTags（非枚举）；自定义 AnimNode_CurvesBlend；ControlRig 输入结构 `AlsControlRigInput`；IK 相关骨骼 `foot_l/foot_r`、`ik_foot_l/ik_foot_r`、`VB foot_l/VB foot_r`；关键曲线名 `LayerHandLeft/Right`、`HandLeftIk/HandRightIk`、`FootLeftIk/FootLeftLock/FootRightIk/...`；动画资产分组：Grounded / Air / Actions / Transitions / TurnInPlace / RotateInPlace / Overlays / View / Base。
- 本地环境路径、引擎版本等隐私信息见 `docs/env.md`（已 gitignore）。

## 模块依赖顺序

```
M0 环境与资产管线 ─► M1 ECS核心状态模型 ─► M2 曲线基础设施 ─► M3 tnua运动基座
─► M4 动画图基座 ─► M5 Locomotion状态机动画 ─► M6 Overlay/Aim ─► M7 FootIK/CR等价物
─► M8 特殊动作(跳跃翻滚) ─► M9 Ragdoll ─► M10 相机 ─► M11 调试与总验收
```

## 已完成基座索引（详情一律见 `archive/2026-08-24/CHECKLIST-M0-M11.md`）

- [x] M0 环境与资产管线（Gate：skeleton_boot 骨架+动画加载播放 OK）
- [x] M1 ECS 核心状态模型（Gate：state_switch 九步序列合法转移全符合预期）
- [x] M2 曲线与数据驱动基础设施（Gate：curve_gate 曲线驱动数值逐点吻合）
- [x] M3 tnua 运动基座（Gate：motion_gate 三档速度精确达标无回归）
- [x] M4 动画图基座（Gate：速度自动混合+热重载+timescale 帧率无关）
- [x] M5 Locomotion 状态机动画（Gate：locomotion_gate 七阶段全程无脚滑跳变）
- [x] M6 Overlay 与 Aim/View 系统（Gate：overlay_gate 叠加解耦+aim 四象限平滑）
- [x] M7 Foot IK 与 ControlRig 等价物（Gate：foot_ik_gate 三 VERDICT 全 PASS）
- [x] M8 特殊动作（Gate：actions_gate 滚/跳/停步综合 PASS + root motion 链路验收）
- [x] M9 Ragdoll（Gate：ragdoll_gate 自然倒地恢复 PASS）
- [x] M10 相机（Gate：camera_gate 不穿模/aim 切换平滑/数据联动 PASS）
- [x] M11 调试工具与总验收（Gate：workspace 测试全绿 + parity-report 落地）

---

## BUGFIX-2026-08-24（用户实测报告缺陷，按依赖序修复；部分须经 ALS-Refactored 截图对比验证）

> 缺陷来源：run_manual.bat（examples/manual_playground.rs）实机试玩反馈，原文照录：
> ① WASD 移动角色面朝方向和移动方向是相反的；② 角色双腿插入地下，需确认胶囊体尺寸和模型位置与 ALS-Refactored 是否一致；
> ③ 鼠标视角、Ctrl 蹲、Shift 冲刺、Alt 翻滚全部无效；④ 跳跃只有向上推力没有对应动画状态；
> ⑤ 没有对标 ALS-Refactored 的按键，无法攀爬无法 ragdoll；⑥ 缺少 ALS 运行时 HUD 界面 1:1 复刻。
> 对标基准：ALS-Refactored 工程（路径见 gitignored `docs/env.md`）。表现类缺陷（BF3/BF4/BF6）验收须留截图证据存 `docs/`。

- [ ] BF1 输入消费补全：`crouch_pressed`/`sprint_held` 目前在全仓库无生产消费者系统（仅 gate 示例脚本消费）——在 als-core 新增输入→状态请求系统：Ctrl 边沿切换 Stance、Shift 按住驱动 Gait 冲刺请求（ALS 语义：冲刺松开回落 Running）。验收：单测覆盖边沿/保持语义；manual 实机 Ctrl/Shift 生效。
- [ ] BF2 manual_playground 全栈编排：现缺 air/roll/ragdoll/view/overlay/stop/mantle-drive/foot-ik/camera 插件且 graph_router 只写 speed 三 pins——挂齐插件族、graph_router 补写 air/land/roll/mantle/stop pins、ragdoll 时切换 `als_ragdoll.animscn.ron`、固定跟随相机换 AlsCameraPlugin。验收：跳跃出现 Air/Land 动画、Alt 翻滚整周滚动、鼠标控制视角相机、急停有停步动画。
- [ ] BF3 WASD 移动方向与面朝相反：确诊三者之一——网格面朝轴 vs tnua `desired_forward` 约定 vs 输入映射符号（M8.6 记录「角色面朝 glTF +Z」而 bevy/tnua forward 为 -Z 为首要嫌疑）。验收：按 W 角色面朝移动方向前进，四向均正确。
- [ ] BF4 双腿插入地下：Playground Landscape 高度场 mesh 无对应碰撞体（manual 场景只搭了 y=-0.1 顶面的隐形 cuboid，出生点取自地形采样高度≈0.92 → 角色沉入地形内）；同时 avatar 子实体偏移 (0,-0.90,0) 与模型根/胶囊对齐关系需对照 ALS 验证（截图比对 UE 内 SK_Manny 相对胶囊位置）。验收：脚底贴地形表面无穿模；截图对比存档。
- [ ] BF5 ALS 对标按键补齐：从 ALS 工程增强输入 IMC 资产提取默认键位表并登记 CHECKLIST；mantle 由现行免按键自动触发改为 ALS 键位触发（保留检测器）、ragdoll 绑键进出（ragdoll_link 已具备能力）。验收：按键触发攀爬/倒地/起身恢复。
    - 键位事实（2026-08-24 自 IMC_Als_Default / IA_Als_* uasset 二进制解析并与 C++ 核对；行号=Source/ALSExtras/Private/AlsCharacterExample.cpp）：Move=WASD/左摇杆(DeadZone)；LookMouse=Mouse2D；Sprint=LShift|FaceButton_Right **Hold(1.0s)+Released**（Input_OnSprint:127）；**Walk=LAlt|RightShoulder Pressed toggle Walk↔Running**（:132，本项目缺失的 action）；Crouch=LCtrl|左摇杆按 Pressed toggle（:144）；**Jump=Space|FaceButton_Bottom Pressed+Released，按下沿优先级链 起身→StartMantling→蹲→站起→Jump**（:156-182——Mantle 非独立键；另 InAir 时 Tick 内 AutoStartMantling 仅 bAutoStartMantlingInAir）；Aim=RMB|LB hold（:184）；**Ragdoll=X|FaceButton_Left Pressed toggle**（:189）；**Roll=LShift 与 Sprint 共键靠 Tap(0.2s)/Hold 区分**，PlayRate=1.3 仅地面（:197）；RotationMode=One|DPadLeft toggle Velocity↔View（:204）；ViewMode=Two|DPadRight 三人称↔第一人称（:211）；SwitchShoulder=C|R3 相机换肩（:217）；UI 层 IMC_Als_UI：Q 换 Overlay 菜单/滚轮切 Overlay/Tab 开关 UI/Z Slomo。
- [x] BF6 HUD 1:1 复刻 ALS ShowDebug 运行时界面：面板分类与开关键位对齐工程 `DefaultInput.ini` DebugExecBindings（Shift+0 主开关，Shift+1..8 = Curves/State/Shapes/Traces/Mantling/CameraCurves/CameraShapes/CameraTraces），面板版式与条目对照 UE 截图复刻（现有 AlsStateHudPlugin 仅五行文本打印）。验收：逐分类截图对照存档。
    - 实现/自测证据（2026-08-24）：`bevy_als::show_debug_hud::AlsShowDebugHudPlugin`——九分类面板逐行映射 AlsCharacter_Debug.cpp/AlsCameraComponent_Debug.cpp（State 分类 11 行全部接真实 ECS 数据源；Curves/CameraCurves 经 AlsShowDebugCurveSources 注入 AlsAnimCurves 句柄按 player elapsed 采样；Shapes 的 InputYawAngle 橙/Speed 紫等配色常量与 C++ 文件行号一一对应；默认开启后无分类=UE AHUD::ShowDebug(NAME_None) 引擎语义，分类头暗绿 (0,0.333333,0)）；切换语义照 HUD.cpp:377-393（可见且已含→移除）；键位 Shift+0..8 独立于 enhanced_input。单测 12 绿；`examples/show_debug_hud_gate` VERDICT panel/header/state_rows/stance_flip/shapes_rows/retoggle_off 全 ok → PASS，exit=0 无 panic。N/A 行（SmoothTargetYawAngle、PivotLagLocation、CameraFieldOfView、bRightShoulder）为仓库尚未实现的对应存储，标签保留待后续条目补齐。

- [ ] BF7 日志级行为对标（BF1–BF6 的行为级总验收门）：双侧同脚本插桩 → 同操作流程运行 → 逐帧指标对比 → 差异回修闭环，保证用户可见可操作行为与 ALS-Refactored 一致。
    - ①ALS 侧插桩：修改本地 ALS-Refactored 插件源码（改动落在其自身 git 仓库独立小步提交；CVar 门控默认关闭零侵入）：trace recorder 每 tick 写 CSV——t/frame、actor pos(cm)/vel(cm/s)、facing yaw/pitch、controller view yaw、五状态 tags+LocomotionAction、动作态（mantle/roll/air active）、双脚踝+pelvis 世界位置（IK 后）、capsule 半高/半径、相机 pos/yaw/pitch；script driver 按共享时间线文件程序化调用 Input_On* 处理函数（与人工操作同一入口，含 Hold/Tap 语义）。
    - ②bevy 侧等价 recorder：`examples/trace_parity.rs` + `bevy_als::trace_recorder`——列名/单位规范一致（坐标按既有映射 gltf=(ue.x,ue.z,ue.y)·0.01 反算为 UE cm 帧；yaw 统一定义为水平面朝向量相对 UE +X 的带符号角），消费同一时间线驱动 AlsInputOverride。
    - ③对比工具 `tools/compare_trace.py`：时间轴重采样对齐 → 逐指标 max/rms 偏差表 + 状态转移时间线 diff → 阈值 PASS/FAIL → 报告入 docs/bf7-compare-report.md。
    - 共享时间线 v1（tools/parity/timeline.csv，两侧唯一事实源）：静止→W 走→停→冲刺跑→急停→跳→落地→蹲走→起立→Shift 轻点翻滚→X 倒地→settle→X 起身→视角右扫 90°→A/D 横移弧线→结束。
    - 验收：全指标落阈值带内或差异均有解释性归档（物理引擎确定性差异单列不判 FAIL）；执行中发现的缺陷回登 BUGFIX 条目修复后复跑。

依赖关系：BF1 →（BF2、BF5）；BF2 →（BF3、BF4 的实机验收）；BF7 依赖 BF1/BF2/BF5 全部落地后执行终评；BF3/BF4/BF6 相互独立，可与主线并行派发 subagent（互不修改同一文件时）。

---

## 附录 A：与 ALS 的刻意差异（非缺陷）

| ALS | bevy_als | 理由 |
|---|---|---|
| AnimBP 状态机 | 显式 system 路由 + animation graph RON | ECS 数据流，无隐藏图执行 |
| GameplayTags | 普通 Rust 枚举组件 | 编译期检查 |
| ControlRig | 分析式 IK systems | 无运行时 rig 编译 |
| Root Motion | 图内 `RootMotionMode` 提取 + `RootMotionOutput` 驱动位移（M8.8；数据经 M8.6 回填） | fork 已支持提取，M0.5 导出缺根轨道由 M8.6 离线回填，不再用曲线等价物 |
| Curves（UBT 资产） | als-ext CurveAsset + 采样节点 | 见 M2 |

## 附录 B：风险与后置项

- **资产格式决策（2026-08-21）**：Bevy 官方推荐 glTF；M0.5/M0.6 管线定为 UE → glTF（Interchange 导出）→ bevy_animation_graph 自带的 glTF 导入器（GraphClip/Skeleton）。**不引入 bevy_ufbx**：其 v0.19.0（bevy 0.19 兼容）虽支持蒙皮数据，但动画曲线未接入 Bevy 动画系统、无社区验证，且 FBX 中转对 ALS 曲线/虚骨/单位换算风险更大；仅当未来需要直接加载 FBX 时再评估。

- bevy_animation_graph additive/mask 语义若与 ALS layering 不完全一致，在 M6 前验证并决定扩展方式（优先 als-ext 扩展点）。（已于 M6.1 验证通过，记录备查）
- Distance matching / motion matching 类高级特性仅实现 Stop 所需子集（M8.3）。
- ALSExtras（AI 控制器示例）不在范围内。
- 音频（Metasound）、Niagara 特效、ALSCamera 动画实例仅保留挂钩点。
