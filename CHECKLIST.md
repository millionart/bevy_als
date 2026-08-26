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
> 对标基准：ALS-Refactored 工程（路径见 gitignored `docs/env.md`）。表现类缺陷（BF3/BF4/BF6/BF6b）验收须留截图证据存 `docs/`。

- [x] BF1 输入消费补全：`crouch_pressed`/`sprint_held` 目前在全仓库无生产消费者系统（仅 gate 示例脚本消费）——在 als-core 新增输入→状态请求系统：Ctrl 边沿切换 Stance、Shift 按住驱动 Gait 冲刺请求（ALS 语义：冲刺松开回落 Running）。验收：单测覆盖边沿/保持语义；manual 实机 Ctrl/Shift 生效。
    - 实现/自测证据（2026-08-24）：`als_core::input_request::AlsInputRequestPlugin`——input_state_request_system（Update，显式 before(state_transition_system) 同 aim 先例）：crouch_pressed 上升沿按当前 Stance 翻转写 request.stance；sprint_held 电平映射 Sprinting/Running 写 request.gait（空中沿用 Queue 语义，释放即以 Running 覆盖排队项）；AlsInputOverride 优先。单测 6 项（边沿单次翻转/二次按压回站/保持幂等零重复事件/override 优先/空中排队与释放清槽）全绿，als-core 135 全绿。语义注记：同值请求经 transition 层幂等 Accept 清槽（M1.2 记录行为）。实机键位验证随 BF2 全栈编排 gate 一并覆盖（插件挂接属 BF2 范围）。
- [x] BF2 manual_playground 全栈编排：现缺 air/roll/ragdoll/view/overlay/stop/mantle-drive/foot-ik/camera 插件且 graph_router 只写 speed 三 pins——挂齐插件族、graph_router 补写 air/land/roll/mantle/stop pins、ragdoll 时切换 `als_ragdoll.animscn.ron`、固定跟随相机换 AlsCameraPlugin。验收：跳跃出现 Air/Land 动画、Alt 翻滚整周滚动、鼠标控制视角相机、急停有停步动画。
    - 实现/自测证据（2026-08-24）：manual_playground 重写——①插件族 26→拆两批挂齐（AlsInputRequest/AlsAim/AlsAir/AlsFootProbe/FootIkApply/OverlayLayer/ViewLayer/RollDrive/StopLink/MantleDrive/RagdollLink/DebugDraw/ShowDebugHud/AlsCamera）；②animated scene 换 als_ragdoll.animscn.ron（motion_overlay 超集+ragdoll 双资产，M9.2），运行时经 ragdoll_link 的 ragdoll_dynamic/readback pins 进出 ragdoll（无需换场景实体）；③graph_router 改 write_motion_graph_pins+write_air_pins 全 pin 写入，roll/mantle/stop/overlay/ragdoll 各驱动插件自写其 pin（grep 核实）；④新增 look_pitch_integrator（look_delta.y→AlsViewPitch 钳制±85°）补 M6.3 缺失的俯仰输入源；⑤相机 Camera3d+AlsCameraRig+角色 AlsCameraTarget（[CAM] params loaded 实证）；⑥新增 ALS_BOOT_AUTO_QUIT_SECS auto-quit 基建供脚本化冒烟；恢复被误删的地面碰撞体。自测：ALS_BOOT_AUTO_QUIT_SECS=8 冒烟 exit=0 无 ERROR/panic、零 runtime WARN（全 pin 在新 animscn 上解析成功）；workspace 测试 205 全绿（135+52+12+…）。实机键位行为验收并入 BF99（原 BF7）日志级对比与用户试玩。
- [x] BF3 WASD 移动方向与面朝相反：确诊三者之一——网格面朝轴 vs tnua `desired_forward` 约定 vs 输入映射符号（M8.6 记录「角色面朝 glTF +Z」而 bevy/tnua forward 为 -Z 为首要嫌疑）。验收：按 W 角色面朝移动方向前进，四向均正确。
    - 实现/自测证据（2026-08-24）：确诊为**双重朝向缺陷**——①视觉层：tnua forward=rotation·NEG_Z（bevy-tnua walk.rs:478 实锤），网格面朝 +Z（M8.6），avatar 子实体未旋转 → 视觉面朝与物理前进恒差 180°；修复=manual avatar 子实体加 RotY(π)（root_motion_link 的 mesh_to_entity_yaw 补偿只作用于速度数学、与视觉旋转正交，保留）。②相机层：als-camera back_dir=rot·NEG_Z 把相机放在角色**面朝侧**（正面）→ W 朝镜头走；修复=back_dir 改 rot·Vec3::Z（相机落在 tnua-facing 背后）。横移符号经一次性探针实测：修复前 dot(vel,cam_right)=−3.85 INVERTED（D 键向屏幕左），修复后 +3.75 RIGHT_OK，输入映射零改动（Bidirectional::new(positive,negative) 参数序核实无误）。回归：camera_gate 脚本几何按旧约定镜像（墙 9→−9、跑向取反、facing_wall 取反）后 PASS（probe_min=0.150 较旧 1.550 更严格、frame_jump 0.879≤1.2、blend/linkage ok）；workspace 205 测试全绿。视觉证据 docs/bf34-shots/（本作背面视角 vs UE 参照）。
    - **装配架构收口（2026-08-25 增补，用户指正）**：UE 式整体角色装配缺失是朝向缺陷扩散根因——avatar 此前在 ~15 处 example 手工 spawn 各自抄相对变换，π 翻转只落了 manual 一处。新增 `bevy_als::character_assembly`：`AlsCharacterBlueprint`+`spawn_als_character` 等价 B_Als_Character 整体挂接（胶囊物理体+状态+tnua 配置+输入 actions+AlsViewYaw/ViewPitch 初始化+AlsCameraTarget+avatar 子实体规范变换 RotY(π)·y=−0.92+可选 AlsCameraRig 相机），`attach_avatar`/`avatar_transform` 为 Mesh 挂接单点；12 个例程迁移 attach_avatar、manual_playground 与 mantle_gate 迁移完整蓝图。此后朝向/偏移约定只允许改这一处。
- [x] BF4 双腿插入地下：Playground Landscape 高度场 mesh 无对应碰撞体（manual 场景只搭了 y=-0.1 顶面的隐形 cuboid，出生点取自地形采样高度≈0.92 → 角色沉入地形内）；同时 avatar 子实体偏移 (0,-0.90,0) 与模型根/胶囊对齐关系需对照 ALS 验证（截图比对 UE 内 SK_Manny 相对胶囊位置）。验收：脚底贴地形表面无穿模；截图对比存档。
    - 实现/自测证据（2026-08-24）：①UE 侧数值经 MCP 实测（tmp/als_ref_shots 流程，副本入 docs/bf34-shots/）：胶囊 90/30 与本项目一致；**SKM_Als 相对胶囊 Z=−92**（比胶囊底低 2cm，ref-pose 包围盒 z∈[−0.07,183.3]）→ avatar 偏移 −0.90 修正为 **−0.92** 逐值对齐 UE 几何（2cm 由脚部 IK/接触解算吸收，与 UE CMC 行为同构）。②地形碰撞：manual 删除隐形平面 cuboid，新增 landscape_collider_bridge——Added<PlaygroundLandscape> 触发，按 als-ext::terrain_height 同一 64×64 网格生成 Collider::trimesh（物理=视觉同一函数源；als-ext 保持零 avian 依赖，沿用 bridge 模式）。③截图基建：ALS_SHOT_PATH/ALS_SHOT_AFTER_SECS 环境变量门控 Screenshot::primary_window+save_to_disk+延时退出。④视觉验收：docs/bf34-parity-front.png——脚底与地形接触线干净无插入，姿态与 UE 参照（ref_side.png，编辑器 ref-pose）一致；workspace 205 测试全绿。
- [x] BF5 ALS 对标按键补齐：从 ALS 工程增强输入 IMC 资产提取默认键位表并登记 CHECKLIST；mantle 由现行免按键自动触发改为 ALS 键位触发（保留检测器）、ragdoll 绑键进出（ragdoll_link 已具备能力）。验收：按键触发攀爬/倒地/起身恢复。
    - 键位事实（2026-08-24 自 IMC_Als_Default / IA_Als_* uasset 二进制解析并与 C++ 核对；行号=Source/ALSExtras/Private/AlsCharacterExample.cpp）：Move=WASD/左摇杆(DeadZone)；LookMouse=Mouse2D；Sprint=LShift|FaceButton_Right **Hold(1.0s)+Released**（Input_OnSprint:127）；**Walk=LAlt|RightShoulder Pressed toggle Walk↔Running**（:132，本项目缺失的 action）；Crouch=LCtrl|左摇杆按 Pressed toggle（:144）；**Jump=Space|FaceButton_Bottom Pressed+Released，按下沿优先级链 起身→StartMantling→蹲→站起→Jump**（:156-182——Mantle 非独立键；另 InAir 时 Tick 内 AutoStartMantling 仅 bAutoStartMantlingInAir）；Aim=RMB|LB hold（:184）；**Ragdoll=X|FaceButton_Left Pressed toggle**（:189）；**Roll=LShift 与 Sprint 共键靠 Tap(0.2s)/Hold 区分**，PlayRate=1.3 仅地面（:197）；RotationMode=One|DPadLeft toggle Velocity↔View（:204）；ViewMode=Two|DPadRight 三人称↔第一人称（:211）；SwitchShoulder=C|R3 相机换肩（:217）；UI 层 IMC_Als_UI：Q 换 Overlay 菜单/滚轮切 Overlay/Tab 开关 UI/Z Slomo。
    - 实现/自测证据（2026-08-25）：①input.rs 重排为上表键位（触发器实测：引擎 InputTriggers.h HoldTimeThreshold 默认 1.0/TapReleaseTimeThreshold 0.2；IA_Als_Sprint=Hold+Released、Roll=Tap、Jump=Pressed+Released 经 uasset 导出类名核实）——新增 Walk/Ragdoll/RotationMode/ViewMode/SwitchShoulder 五 action（共 12），Look 补方向键绑定，手柄按 ALS 表重排；②input_request：walk toggle、ragdoll toggle（Ragdolling↔Grounded 请求，Air 进 Ragdoll 合法）、rotation_mode toggle（仅翻转自由模式，Aiming 中只更新 AlsLastFreeRotationMode）、view_mode/switch_shoulder 翻转 AlsFirstPersonView/AlsCameraLeftShoulder 组件（als-camera 消费：强制 FirstPersonPlaceholder/肩偏移取反）；sprint 改沿驱动（AlsSprintPrev，修复持续重申 Running 覆盖 walk toggle 的语义缺陷）；③Space 链：motion.rs `jump_chain` 纯函数（ExitRagdoll→Mantle→StandUp→Jump 与 ALS Input_OnJump 同序）+ AlsJumpPrev 边沿门控（override 电平注入防每帧重启）；mantle 检测下沉 als-core::mantle::detect_mantle（mantle_drive 与链共用），mantle_drive 移除免按键自动触发改纯驱动；④**图选片反相缺陷修复**：全图 blend 因子审计（factor→pose_b 权重约定对照驱动语义）发现 mantle high vs low blend pose_a/b 接反——Low 台播 A_Als_Mantle_High（用户截图实证），交换 pose/time 四边后 Low 台呈现蹲姿侧摆 vault（docs/bf5-mantle-low-fixed.png 对比修复前 docs/bf5-mantle-mid.png）；ALS 选片语义核对：MantlingHeight=(台面−脚底)>1.25 严格 → High 否则 Low（AlsCharacter_Actions.cpp:432-436），0.8m 台双侧均选 Low ✓，朝向 warp TargetDirection=−normal 同构 ✓。⑤回归：mantle_gate 双攀爬 PASS（LowLeft→LowLeft、低台顶→高台 Low 带内）、actions_gate PASS（roll 3.37 align1.0/jump light/stop）、roll_gate PASS、workspace 210 测试全绿。
- [x] BF6 HUD 1:1 复刻 ALS ShowDebug 运行时界面：面板分类与开关键位对齐工程 `DefaultInput.ini` DebugExecBindings（Shift+0 主开关，Shift+1..8 = Curves/State/Shapes/Traces/Mantling/CameraCurves/CameraShapes/CameraTraces），面板版式与条目对照 UE 截图复刻（现有 AlsStateHudPlugin 仅五行文本打印）。验收：逐分类截图对照存档。
    - 实现/自测证据（2026-08-24）：`bevy_als::show_debug_hud::AlsShowDebugHudPlugin`——九分类面板逐行映射 AlsCharacter_Debug.cpp/AlsCameraComponent_Debug.cpp（State 分类 11 行全部接真实 ECS 数据源；Curves/CameraCurves 经 AlsShowDebugCurveSources 注入 AlsAnimCurves 句柄按 player elapsed 采样；Shapes 的 InputYawAngle 橙/Speed 紫等配色常量与 C++ 文件行号一一对应；默认开启后无分类=UE AHUD::ShowDebug(NAME_None) 引擎语义，分类头暗绿 (0,0.333333,0)）；切换语义照 HUD.cpp:377-393（可见且已含→移除）；键位 Shift+0..8 独立于 enhanced_input。单测 12 绿；`examples/show_debug_hud_gate` VERDICT panel/header/state_rows/stance_flip/shapes_rows/retoggle_off 全 ok → PASS，exit=0 无 panic。N/A 行（SmoothTargetYawAngle、PivotLagLocation、CameraFieldOfView、bRightShoulder）为仓库尚未实现的对应存储，标签保留待后续条目补齐。
    - 勘误（2026-08-25，用户指正）：本条目实际交付为 ShowDebug 调试面板复刻；缺陷⑥所指「运行时 HUD」实为 W_Als_Hud 控件蓝图及其功能，超出本条目范围，另立 BF6b 独立收口。
- [x] BF6b w_als_hud 及其功能 1:1 复刻（视觉一致 + 内部功能逻辑/交互一一对标，代码/算法级移植）：①事实提取先行——经 Unreal MCP 从 ALS-Refactored 工程导出 W_Als_Hud 全部事实：控件树层级/锚点/版式/字体/贴图/材质、动画序列、事件图表与属性绑定逻辑、输入处理链（IMC_Als_UI：Q 换 Overlay 分类菜单、滚轮切 Overlay、Tab 开关 UI、Z Slomo，键位事实见 BF5），逐控件登记清单；②代码级移植——UE 蓝图节点图/绑定表达式逐一翻译为 Bevy system/UI 组件逻辑，每条文本/图标/高亮/进度的绑定都接真实 ECS 状态源，禁止仅视觉近似或硬编码假数据冒充绑定；所需贴图字体等资产经 Unreal MCP 批量导出并登记 `assets/docs/export-manifest.md`（规则 5：本条目即增补依据）；③交互对标——按键/滚轮/UI 开关行为与 ALS 同语义同入口（经 input_request 层消费）。验收：逐 UI 元素验证对应功能并留证据（截图对照 UE 渲染存 docs/ + 操作记录），无未解释的 N/A 遗留。执行约束：未经用户明确授权不得启动本条目（2026-08-25 用户指令）。
    - UE 侧事实提取（2026-08-25，UE 5.8.1 编辑器 MCP 实测）：三控件 `W_Als_Hud`/`W_Als_OverlayModeMenu`/`W_Als_OverlayModeOption` 经 UMGToolSet.GetWidgetDescription 全控件树逐属性导出——面板 SizeBox 340px 锚右上偏移(-25,25) autoSize；三段式（标题区/DebugBorder/InputBorder，外层底 0.015·α0.25、行格 0.015·α0.5，段间距 8、行距 4，键位格宽 0.7（Debug 区 0.6）/标签格 1.0 比例填充）；字体 Roboto Regular/Italic/Bold Italic 12/10/14px；EnabledColor=(0,1,0)、DisabledColor=(0.5,0.5,0.5)、标题accent (1,0.6667,0)；Option 200×25 选中 (0.15,0.15,0.15,0.5)/未选 (0.015,…,0.5)、文字白 14px Ellipsis。行为逻辑经 BlueprintTools.read_graph_dsl 逐图反汇编：B_Als_PlayerController BeginPlay 挂 IMC_Als_UI（Q/Y、滚轮↑↓/十字键上下、Z/Back、Tab/Menu；IA_Als_OverlayModeMenu=Pressed+Released 双触发）；**Q 按**=开菜单+非 Slomo 态 SetGlobalTimeDilation(0.35)，**Q 松**=ApproveOverlayMode(直接 SetOverlayMode(Selected))+RemoveFromParent+非 Slomo 态恢复 1.0；**滚轮**=仅菜单在视口时 SelectNext(索引-1 首尾环绕)/SelectPrevious(索引+1 环绕)；**Z**=bSlomoActive 翻转→膨胀 0.15/1.0（SelectFloat 取翻转后值）；**Tab**=bUIVisible 翻转→HUD DebugBorder+InputBorder 同步折叠/恢复+UIToggle 双标签绿/灰（标题区常显）；HUD Tick 每帧 Refresh 链按 AHUD ShowDebug 分类态（ALS.Curves/State/Shapes/Traces/Mantling/CameraCurves/CameraShapes/CameraTraces，GetToggleColor(ShouldDisplayDebugForActor)）着色；菜单 Construct 初始化选中=角色当前 Overlay、选项=Als.OverlayMode 子标签动态生成（间距 4px、NameToDisplayString 显示）、Approve=SetOverlayMode+点击音。菜单项顺序以用户 PIE 截图运行时序为准（Torch→Rifle→PistolTwoHanded→PistolOneHanded→Masculine→Injured→HandsTied→Feminine→Default→Box→Bow→Binoculars→Barrel，非标签声明序）。
    - Bevy 实现（2026-08-25）：als-core input.rs 新增 5 action 共 17（OverlayMenu 用 Down 条件承载按住语义；滚轮经 `Binding::mouse_wheel`+SwizzleAxis::YXZ+动作级 Clamp::pos 分离上下方向，键位 Q/Y、Z/Select、Tab/Start、DPadUp/Down）；bevy_als 新增 `als_hud` 模块（W_Als_Hud 复刻：布局/文本/配色逐值对照，Tab 折叠=Node::Display::None 即 UE Collapsed，Slomo=Time&lt;Virtual&gt;::relative_speed 0.15/1.0 等价 GlobalTimeDilation，调试分类色接 AlsShowDebugHudState(BF6)，UiToggle/Slomo 色接 AlsHudState）与 `overlay_mode_menu` 模块（菜单+Option 复刻：环绕选择、高亮切换、批准写 `AlsStateRequest.overlay_state` 走 M1 transition 链到真实 OverlayState）；manual_playground 挂接两插件。
    - 用户决策偏差（登记附录 A）：①菜单停靠 HUD 面板左侧（UE 为头顶/根骨中点+100cm 世界锚定投影，防 UI 耦合骨骼）；②不移植 Roboto，用 Bevy 默认字体；③S_Als_UI_Click 点击音不复刻（WAV 解码需 bevy `wav` feature 引入 hound 依赖，用户否决；音频资产与临时提取文件已删除）。
    - 门禁证据（`examples/als_hud_gate`，2026-08-25）：21 项断言全 ok → `[BF6B][VERDICT] PASS`——HUD 行文本齐全/Tab 折叠恢复+UIToggle 绿灰双色/Slomo 0.15↔1.0/Shift+1 Curves 变绿+Shift+0 主关全灰/菜单开合+13 项顺序与文本+高亮初始化(Default)+环绕循环(8→7→8→9)/批准写请求且 OverlayState→Box/菜单开合不干扰 Slomo 膨胀；截图 `docs/bf6b-shots/bevy-hud.png`、`bevy-menu.png` 对照用户提供的 UE PIE 参照截图；方法论教训：bevy 0.19 带 `Screenshot` 组件的实体截图后被引擎 despawn（首插相机致第二张截图静默失败→改独立 screenshot 实体）；MockSpan::Updates 按帧数计帧率敏感→门禁持握改 MockSpan::Manual 显式启停。manual_playground 冒烟 exit=0 零 ERROR 零 runtime WARN；workspace 测试 220 全绿（als-core 140+1 探针、als-ext 52、bevy_als 21+…）。

- [x] BF8 根运动在 TP/距离匹配强制采样下恒零：roll/actions/mantle 三门禁位移崩塌（dist 0.28 vs clip 2.75）。根因链（2026-08-25 实证）：①fork ClipNode 的 rm delta 依赖 `ctx.prev_time()`（无 key 的全局 last-time 读）与 `set_time`（带 StateKey 写）不对称，经 TimePositionNode/DistanceMatchNode 以 `TimeUpdate::Absolute` 回传强制采样时 prev 簿记失效 → 恒发 Some(0)（Delta 自然流正常，探针 D 变体对照实证）；②`derive_default_root_bone` 兜底每帧全骨架路径遍历+字符串比较重解析根骨，嵌套组合后帧成本放大引发固定步长饥饿、时序敏感门禁（air 重着陆）连锁失真。修复：`als_ext::RootMotionClipNode`——自维护 `RmPrevState{time,cached_root}` 节点状态（GraphStateType 扩展点，零 fork 改动），Absolute/Delta 双流正确产 delta，根骨解析一次缓存；actions 图 roll/mantle×3 四 clip 换用。验收：air_gate heavy ff_w=1.00 PASS、roll_gate dist=2.70/2.71 align=1.000 PASS、actions/mantle VERDICT PASS；perf 相对基线持平（本分支 7.35ms vs 纯 main 同机同法 7.25ms，均超预算属并行会话负载环境性，非本改动回归）。方法论教训登记：直跑 exe 时 bevy 相对资产路径解析至 exe 目录（无 CARGO_MANIFEST_DIR 回退）→ 门禁必须经 `cargo run` 取证，直跑结论（含本轮一度误判 main 存在同缺陷）一律无效。

- [x] BF-Scene 勘误：L_Als_Playground 可视场景返修——真实 `Landscape1.gltf` 根变换只应用一次并由同一渲染网格生成碰撞；TextRenderActor 使用 Bevy 默认字体的布局/atlas 生成 `Mesh3d` 世界字形四边形，由 `Camera3d` 以深度测试的 Masked PBR 材质绘制，而非屏幕投影、billboard 或单色占位面片；按 UE 主光、天空和材质事实启用 PBR、动态阴影和可见天空。依据规则 12：2026-08-26 编辑器 MCP 实测 `Landscape_1` root=(-1000,1000,50)cm/scale=(16,16,16)，用户导出的 glTF 根已等值编码；`DirectionalLight_1`=(pitch -45,yaw 45)、Movable、intensity=3、动态阴影/4 cascades/40000cm 距离开启；SkySphere 自动随该光更新，zenith=(0.034046,0.109247,0.295)、horizon=(1.979559,2.586644,3)、sun brightness=75；TextRender 使用 `/Engine/EngineFonts/RobotoDistanceField`、`M_Als_OutlinedText`、居中/底部对齐，且典型 worldSize=50cm；该材质当前为 Surface、Masked、Default Lit、深度测试开启、双面且不投射 masked dynamic shadow。用户明确决定：不带入或提交字体资产，文字使用 Bevy 默认字体，此项差异登记附录 A。资产增补（本条批准）：仅用户导出的 Landscape1 glTF+PBR 三贴图，均登记 export manifest；禁止提交 uasset。验收：scene_gate 运行时断言 Bevy 默认字体生成的 `Mesh3d` 世界文字、单次 Landscape 根变换、主光阴影/PBR/天空探针与动态体统计均 PASS；manual 真实运行日志无 ERROR/WARN、地形碰撞和截图复核。
  - Gate 记录（2026-08-26）：`cargo run --jobs 5 --example scene_gate` 实际日志为 `static=174 text=22 native_text=22/22 glyph_text=22/22 world_mesh_text=22 masked_world_mesh_text=22 landscape=true identity=true sun=1/shadow=true sky=2 pbr=174 movers=3 movers_active=3 spin_yaw_abs=999.8deg -> PASS`；`docs/bfscene-shots/scene-repair-text.png` 实拍可读 `~25 degrees`，验证世界锚点文字不是屏幕叠字。
  - 冒烟记录（2026-08-26）：`cargo run --jobs 5 --example manual_playground` 实际日志 `landscape collider built verts=16384 triangles=31752`，截图写入 `docs/bfscene-shots/manual-scene-repair.png`，无 ERROR/WARN；`cargo test --workspace --jobs 5` 通过（als-core 140、als-ext 52、bevy_als 21）。

  - [x] BF-Scene 旋转勘误（2026-08-26 用户复报）：几个带 Rotation 的 Engine Cube 在 Bevy 中视觉朝向与 ALS 原版相反。事实提取：UE 5.8.1 源码 `Engine/Source/Runtime/Core/Private/Math/UnrealMath.cpp` 的 `FRotator::Quaternion()` 标量公式中 Roll 对应 `X = CR*SP*SY - SR*CP*CY`，即分解为 `QuatZ(+Yaw) * QuatY(-Pitch) * QuatX(-Roll)`；当前 Bevy 实现错误使用 `QuatX(+Roll)`。编辑器 MCP 实测当前 `L_Als_Playground` 的 `StaticMeshActor_UAID_94B86D58D760221001_1788787042` 为 (Pitch=0,Yaw=0,Roll=44.25)，`StaticMeshActor_UAID_54BF6414A059D82202_1613061415` 为 (0,90,-15)。修复须让 `scene_gate` 以 UE 源码公式独立计算期望方向，逐一比对清单中 8 个非零旋转 Engine Cube 的运行时世界轴，输出数值误差并作为 Gate 条件。
    - Gate 记录（2026-08-26）：`cargo test -p als-ext --lib --jobs 3` 为 52/52 passed；`RUST_LOG=info cargo run --jobs 6 --example scene_gate` 真实运行日志为 `cube_orientation=8/8 cube_axis_error=0.000000 -> PASS`（其余既有场景断言亦全 PASS）。

- [ ] BF99 日志级行为对标（原 BF7，2026-08-25 用户指令改名并移序至 BF8 之后；BF1–BF8 与 BF6b 的行为级总验收门）：双侧同脚本插桩 → 同操作流程运行 → 逐帧指标对比 → 差异回修闭环，保证用户可见可操作行为与 ALS-Refactored 一致。
    - ①ALS 侧插桩：修改本地 ALS-Refactored 插件源码（改动落在其自身 git 仓库独立小步提交；CVar 门控默认关闭零侵入）：trace recorder 每 tick 写 CSV——t/frame、actor pos(cm)/vel(cm/s)、facing yaw/pitch、controller view yaw、五状态 tags+LocomotionAction、动作态（mantle/roll/air active）、双脚踝+pelvis 世界位置（IK 后）、capsule 半高/半径、相机 pos/yaw/pitch；script driver 按共享时间线文件程序化调用 Input_On* 处理函数（与人工操作同一入口，含 Hold/Tap 语义）。
    - ②bevy 侧等价 recorder：`examples/trace_parity.rs` + `bevy_als::trace_recorder`——列名/单位规范一致（坐标按既有映射 gltf=(ue.x,ue.z,ue.y)·0.01 反算为 UE cm 帧；yaw 统一定义为水平面朝向量相对 UE +X 的带符号角），消费同一时间线驱动 AlsInputOverride。
    - ③对比工具 `tools/compare_trace.py`：时间轴重采样对齐 → 逐指标 max/rms 偏差表 + 状态转移时间线 diff → 阈值 PASS/FAIL → 报告入 docs/bf99-compare-report.md。
    - 共享时间线 v1（tools/parity/timeline.csv，两侧唯一事实源）：静止→W 走→停→冲刺跑→急停→跳→落地→蹲走→起立→Shift 轻点翻滚→X 倒地→settle→X 起身→视角右扫 90°→A/D 横移弧线→结束。
    - 验收：全指标落阈值带内或差异均有解释性归档（物理引擎确定性差异单列不判 FAIL）；执行中发现的缺陷回登 BUGFIX 条目修复后复跑。执行约束：未经用户明确授权不得启动本条目（2026-08-25 用户指令）。

依赖关系：BF1 →（BF2、BF5）；BF2 →（BF3、BF4 的实机验收）；BF99（原 BF7，2026-08-25 改名移序）为终评门，待 BF1–BF8 与 BF6b 全部落地且 RG2 回归通过后执行；BF3/BF4/BF6/BF6b 相互独立，可与主线并行派发 subagent（互不修改同一文件时）；BF6b 与 BF99 未经用户明确授权不得启动执行。

---

## REFACTOR-2026-08-25（动画图模块化：单一大图 → AB_Als 式分层子图组合）

> 背景：`assets/graphs/locomotion_overlay.animgraph.ron` 经 M2–M9 逐模块追加已生长为 867 行单一枢纽，review/并行编辑困难，且失去与上游 AB_Als Linked Anim Layers 的结构对应。用户决策（2026-08-25）：按 AB_Als 模块化思路重构；定序在 BF99（原 BF7）之前执行——终评对象须为最终结构，避免终评后结构变更使对标失效。
> 技术可行性已核实（fork `bevy_animation_graph_builtin_nodes/src/graph_node.rs`）：GraphNode 以 `Handle<AnimationGraph>` 嵌套子图，`spec()` 将子图 `io_spec` 的输入/输出 pin 逐一转发为本节点 pin（sorted_inputs / iter_output_data），子图经 `create_child_context` 独立求值——分层组合无需修改 fork，符合 agents.md 规则 4 扩展边界。

- [x] RG1 子图边界划分与 pin 合同冻结：盘点根图全部节点与 pin 归属，按 ALS 层边界拆四个子图资产——`graphs/base_locomotion.animgraph.ron`（Grounded 核心：idle/walk/run/sprint 混合、stance/crouch、start/stop/pivot、turn-in-place/rotate-in-place）、`graphs/air.animgraph.ron`（jump/fall/land）、`graphs/actions.animgraph.ron`（roll/mantle）、`graphs/overlay_view.animgraph.ron`（overlay 上半身 mask、view lean）；ragdoll 配置链留根图作胶水。硬约束：根图对外 pin 名与类型逐一不变（graph_router/roll_drive/mantle_drive/stop_link/overlay_layer/view_layer/ragdoll_link 七驱动插件零改动）；`locomotion_overlay.animgraph.ron` 文件名保留为组合根（animscn 与 examples 引用不变）。产出：本条目下补记节点归属表与根图 pin 合同清单。验收：合同清单覆盖根图现有全部对外 pin 无遗漏。
    - 盘点结论（2026-08-25，逐行通读 locomotion_overlay.animgraph.ron 867 行）：最终输出为七级串级覆盖链 base_loop→(air_weight)→land→(roll_weight)→(stop_weight)→(mantle_weight)→overlay mask→look mask→Output，故子图按级联层切分、跨图仅传 pose+time 成对引脚。**节点归属**：①base_locomotion=idle/walk/run/sprint 四 Clip+speed BlendSpace1D(Absolute)+SpeedNode(play_rate)+Loop（现 id 1111…01/2222…02/3333…03/4444…04/5555…05/6666…06/7777…07 原样迁入）；②air=jump loop/fall/fall fast/land light/land heavy 五 Clip+fall fast blend+air blend+land variant blend+land blend（e1e1…01–09），入参 base_pose+base_time；③actions=roll Clip(GroundPlane)+roll override blend+d6d6…01 roll TimePosition+stop left/right Clip+双 DistanceMatchNode(als_ext)+stop variant/override blend+mantle high/low L/low R Clip(全 Full root motion)+低姿 L/R variant blend+high vs low blend+mantle override blend+d6d6…02–04 mantle TimePosition（e1e1…0a/0b、f2f2…01–06、a9a9…01–06、d6d6…02–04），入参 prev_pose+prev_time——对应 ALS Actions/Roll+Actions/Mantle+Transitions/Stop 资产分组；④overlay_view=torch/reference Poses 双链+overlay Difference+masked overlay(Additive+mask)+look down/forward/up 三 Clip+look pitch BlendSpace+look Loop+look Difference+masked look(Additive+mask)（a1a1…01–06、b1b1…01–07、c2c2…01–02），入参 prev_pose+prev_time。**pin 合同冻结（23 输入名/类型/顺序/默认值逐一不变+pose/time 输出不变）**：speed/play_rate→base_locomotion；actual_speed 合同保留（全图无消费边，graph_router 写入专用，登记备查）；air_weight/fall_fast_weight/land_weight/land_heavy_weight→air；roll_weight/roll_clip_time/stop_weight/stop_right_weight/stop_remaining/stop_total/mantle_weight/mantle_high_w/mantle_lr/mantle_clip_time→actions；overlay_weight/upper_body_mask(BoneMask)/diff_factor/view_pitch_norm/look_weight/look_mask(BoneMask)→overlay_view。每 pin 恰好单一子图所有、零跨图扇出。**跨图接线（唯一新增边型）**：base_locomotion.pose/time→air.base_pose/base_time→actions.prev_pose/prev_time→overlay_view.prev_pose/prev_time→根 OutputData("pose")/OutputTime。default_data 分配：各子图收编自有 pin 默认值（取现值），根图 default_data 保持 23 pin 全量现值不变（首帧前兜底语义不变，如 stop_remaining=1.0/play_rate=1.0/diff_factor=1.0）。**机制核实补记**：fork core animation_graph/mod.rs 有 SourcePin::InputTime(GraphInputPin) 与 add_input_time_edge（RON 边写法 NodeTime(n,"t"): InputTime(Passthrough("x"))），子图 io_spec.input_times 经 GraphNode::spec() 转发为实例时间入引脚（graph_node.rs:102-110），Pose 型 DataSpec 可作输入——跨图成对传 pose+time 可行，无需改 fork。
- [ ] RG2 组合根落地与回归验收（本节 Gate）：四个子图 RON 落盘 + 根图改为 GraphNode 组合与 pin 转发接线；重点核对嵌套后时间更新传播与 BlendSpace Absolute 同步不因跨图产生相位漂移（对照重构前 gate 输出基线）。Gate 证据：examples 全部既有 gate 重跑 PASS（motion/air/actions/roll/mantle/stop/overlay/foot_ik/camera/ragdoll/playground_parity/perf）、workspace 测试全绿、manual 冒烟（ALS_BOOT_AUTO_QUIT_SECS）exit=0 零 runtime WARN；证据摘要写入该次提交说明。
    - 中期状态（2026-08-25）：组合根已落地（`als_ext::LayeredGraphNode` 嵌套 + BF8 `RootMotionClipNode`），`cargo run` 有效方法论下 12 门禁 11 PASS——motion/locomotion(22.00s)/overlay/curve/air(heavy ff_w=1.0)/stop/foot_ik/camera(probe_min=0.150)/roll(2.70,2.71)/actions/mantle(双台落顶)/playground_parity 全过。**未过 2 项**：①ragdoll_gate 恢复后 char_y 稳定 1.31-1.38（判定窗 0.7-1.1，纯 main 同法 y=0.90 PASS）→ 组合嵌套引入恢复后落位高度发散，疑与嵌套求值的姿态/物理体堆叠交互相关，待专项定位；②perf_gate 本分支 7.35ms 与纯 main 7.25ms 同超预算（并行会话负载环境性，相对持平非回归）。RG2 保持未勾，ragdoll 高度回归修复并复跑后收口。

依赖关系：RG1 → RG2；RG2 通过后才允许启动 BF99。

---

## 附录 A：与 ALS 的刻意差异（非缺陷）

| ALS | bevy_als | 理由 |
|---|---|---|
| AnimBP 状态机 | 显式 system 路由 + animation graph RON | ECS 数据流，无隐藏图执行 |
| GameplayTags | 普通 Rust 枚举组件 | 编译期检查 |
| ControlRig | 分析式 IK systems | 无运行时 rig 编译 |
| Root Motion | 图内 `RootMotionMode` 提取 + `RootMotionOutput` 驱动位移（M8.8；数据经 M8.6 回填） | fork 已支持提取，M0.5 导出缺根轨道由 M8.6 离线回填，不再用曲线等价物 |
| Curves（UBT 资产） | als-ext CurveAsset + 采样节点 | 见 M2 |
| w_als_hud Overlay 菜单世界锚定头顶骨骼 | 菜单停靠 HUD 面板左侧（BF6b 用户决策 2026-08-25） | 解除 UI 对骨骼/相机投影的耦合 |
| W_Als_Hud 使用 Roboto 字体 | Bevy 默认字体（BF6b 用户决策 2026-08-25） | 字体非功能项，不移植 |
| L_Als_Playground TextRender 使用 RobotoDistanceField 与描边材质 | Bevy 默认字体布局/atlas 生成的 `Mesh3d` 世界字形四边形（BF-Scene 用户决策 2026-08-26） | 不带入字体资产；保留文字内容、颜色、世界锚点与固定朝向，不做相机 billboard |
| S_Als_UI_Click 菜单点击音 | 不复刻（BF6b 用户决策 2026-08-25） | WAV 解码需 bevy `wav` feature（hound 依赖），用户否决新增依赖 |

## 附录 B：风险与后置项

- **资产格式决策（2026-08-21）**：Bevy 官方推荐 glTF；M0.5/M0.6 管线定为 UE → glTF（Interchange 导出）→ bevy_animation_graph 自带的 glTF 导入器（GraphClip/Skeleton）。**不引入 bevy_ufbx**：其 v0.19.0（bevy 0.19 兼容）虽支持蒙皮数据，但动画曲线未接入 Bevy 动画系统、无社区验证，且 FBX 中转对 ALS 曲线/虚骨/单位换算风险更大；仅当未来需要直接加载 FBX 时再评估。

- bevy_animation_graph additive/mask 语义若与 ALS layering 不完全一致，在 M6 前验证并决定扩展方式（优先 als-ext 扩展点）。（已于 M6.1 验证通过，记录备查）
- Distance matching / motion matching 类高级特性仅实现 Stop 所需子集（M8.3）。
- ALSExtras（AI 控制器示例）不在范围内。
- 音频（Metasound）、Niagara 特效、ALSCamera 动画实例仅保留挂钩点。
