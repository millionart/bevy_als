# Shift+0～9 Debug 链路对标

本报告对应 BF124。按每个快捷键的实际调用树核对，不以右侧图例出现、计数非零或编译成功代替分支验收。`ALS` 表示本地 ALS-Refactored 插件，`UE` 表示本地 UE 5.8 源码；全部路径相对各自工程，不记录机器路径。状态以各分支末尾的结论为准。

保留用户明确指定的移植差异：Bevy 字体、Overlay 菜单屏幕停靠、无点击音；UE PlayerState/Controller 等身份诊断显示真实 ECS 实体、归属和输入上下文。截图与性能验收继续暂停，真实运行只声明功能验收。

<details open>
<summary>公共根与 Shift+0：输入、类别、目标、HUD、包围盒、生命周期</summary>

```text
DefaultInput.ini / Engine.PlayerInput ⇒ show_debug_hud::keyboard_system
├─ Shift+Zero → ShowDebug（没有类别参数）
│  ├─ PlayerInput::GetBind ⇒ ShiftLeft || ShiftRight；不排除 Ctrl/Alt/Super
│  ├─ ExecInputCommands / IE_Pressed ⇒ keys.just_pressed；重复/释放不切换
│  └─ AHUD::ShowDebug(None) ⇒ AlsShowDebugHudState::toggle_master
│     ├─ DebugDisplay 类别集合保留
│     └─ 可见状态变化 → 清空缓存目标，保留导航索引
├─ Shift+One～Eight → ShowDebug <Als.Category>
│  ├─ 已显示且类别已选 → 删除类别；其余 → AddUnique ⇒ toggle_category_engine
│  ├─ bShowDebugInfo = true
│  └─ SaveConfig ⇒ AlsShowDebugSettingsFile；只保存类别，visible 跳过序列化
├─ GetCurrentDebugTargetActor ⇒ debug_target::select_target / AlsDebugTarget
│  ├─ 缓存目标；空时回退当前 ViewTarget
│  ├─ PageUp / PageDown → NextDebugTarget / PreviousDebugTarget
│  │  ├─ 当前 World 中有效、最近渲染的 Pawn；列表重建后重找当前索引
│  │  ├─ WasRecentlyRendered ⇒ AlsLastDebugRender；容差 max(0.2, dt+0.0001)
│  │  ├─ 空列表索引 = INDEX_NONE；目标回退 ViewTarget
│  │  └─ 下一项取模；上一项减一，负数回到末项
│  └─ UAlsDebugUtility::ShouldDisplayDebugForActor
│     └─ 主开关 && 类别 && 目标相等 ⇒ is_active && With/Has<AlsDebugTarget>
└─ AHUD::PostRender → ShowDebugInfo ⇒ PostUpdate / hud_sync_system + AlsDebugDisplaySet
   ├─ DebugCanvas：全屏黑色 alpha 0.2，UMG 之上
   ├─ Tiny 字体红色目标说明/左右 Shift 分栏提示
   ├─ GetActorBounds(onlyColliding=true) ⇒ draw_bounds_box_system / mesh_debug_bounds
   │  ├─ CollisionCylinder：正常时参与；Ragdoll 时 NoCollision，不参与
   │  ├─ CharacterMesh0：QueryOnly；Ragdoll 时 QueryAndPhysics，均参与
   │  │  └─ SkeletalMesh::CalcMeshBound → PhysicsAsset::CalcAABB
   │  │     ├─ 各骨骼当前世界变换 × 物理形状（球/盒/胶囊）
   │  │     └─ PA_Als 21体/22形状；bConsiderForBounds 排除 root 辅助体
   │  ├─ Camera / Arrow：NoCollision，不参与
   │  └─ 合并世界 AABB 后扩张 1.1，绿色，线宽 2，当前帧；默认 cvar=1
   ├─ AlsCharacterExample::DisplayDebug
   │  ├─ Camera.IsActive → Camera::DisplayDebug → Shift+6～8 子树
   │  └─ AlsCharacter::DisplayDebug → Shift+1～5 子树
   │     └─ Character → Pawn → Actor / Controller 父类诊断
   └─ 关闭主开关
      ├─ 停止 HUD、即时形状及新的分类事件记录
      └─ 已产生的限时世界绘制按原时钟到期，不清空历史
```

源事实入口：`Config/DefaultInput.ini:87–95`；`UE/Engine/Config/BaseInput.ini`；`UE/Engine/Source/Runtime/Engine/Private/UserInterface/PlayerInput.cpp::{GetBind,ExecInputCommands}`；`HUD.cpp::{ShowDebug,GetCurrentDebugTargetActor,GetDebugActorList,NextDebugTarget,PreviousDebugTarget,ShowDebugInfo}`；`Actor.cpp::{WasRecentlyRendered,GetComponentsBoundingBox}`；`ALS/Source/ALS/Private/Utility/AlsDebugUtility.cpp::ShouldDisplayDebugForActor`。

目标映射：`src/show_debug_hud.rs::{keyboard_system,apply_show_debug_key,AlsShowDebugHudState,hud_sync_system}`；`src/show_debug_hud/canvas.rs`；`src/debug_target.rs::{select_target,record_rendered_characters}`；`src/debug_display.rs::{reset_stats_system,draw_bounds_box_system}`。Input 在 Update；目标解析在 Input 之后；HUD 与世界绘制在 PostUpdate，读取最终运动/动画/相机数据。

差异与修复：主开关包围盒原先误挂在 Shapes 类别，现直接受主开关控制，独立原生 Gizmo 线宽为 2；空列表导航索引改为 -1；类别通过手动入口的 `saved/als_debug.ron` 跨启动保留，主开关不序列化，自动验收不读写该文件。旧 Gate 仅覆盖有 Shapes 的场景与一直非空的候选列表，没有覆盖这三个生命周期边界。

组件事实来自本次原生 `B_Als_Character` 与 `PA_Als` 提取；22个形状均通过现有离线 `gen_ragdoll.target_shape` 与已安装目标物理配置复算，原资产未改。网格包围盒消费者复用当前 Avatar → AnimationGraphPlayer → SpawnedRagdoll 的形状/变换及物理配置中的 `bConsiderForBounds`；不把 broad phase 的速度扩张框当作实际形状边界。

验收：36项范围回归通过；主开关独立绘制、空列表重新出现、重启恢复类别的旧实现均被反例拒绝；取消root排除及线宽分组后，两项几何合同均失败。删除临时Vec和自写union，直接使用Avian `ColliderAabb::merged` 后检查仍通过。真实 `Interface` 24.880秒功能通过，覆盖0～8开关两态与9不变性共19种状态，主开关/目标盒/HUD显示联动和原布局、菜单、实际值检查通过，无fault、graph error或temporal anomaly。

验收工具同步修正：只在Interface运行HUD布局观察器；读取真实Node/Visibility，不能用根实体存在性判断显示；每键合并全部输入时间窗，Tab/Z/Q固定2/2/1次按下（旧实现7/7/1）。保留真实HUD插件下隐藏根反例与按下次数检查。完整库初轮1140项通过/1项上述观察器失败，修正后110项相关测试及最终33项HUD范围回归通过；不将暂停的性能预算宣称为通过。

边界：通用 UE 控制台的 Reticle、Game、Animation、Physics 等命令不是 Shift+0～9 的可达分支；不据此制造额外快捷键或引擎控制台。项目不做网络同步，因此不展开 SimulatedProxy 的远端 Controller 分支。

</details>

<details>
<summary>Shift+1 — Als.Curves：43项骨架元数据 → 最终Pose曲线 → 源语义显示</summary>

```text
Shift+One → ShowDebug Als.Curves ⇒ 公共输入树 / DebugCategory::Curves
├─ DisplayDebugCurves 的名称集合
│  ├─ Mesh.SkeletalMesh.Skeleton.GetCurveMetaDataNames
│  │  ⇒ SK_Als 本次原生提取 / AssetUserData 所附 AnimCurveMetaData
│  ├─ FName::LexicalLess 排序 ⇒ import_debug_curves.py / als_character_curves.runtime.json 的43项名称
│  └─ 不能用“本帧非零曲线”或其他骨架的元数据替代完整集合
├─ GetAnimInstance.GetCurveValue(name)
│  ├─ ParallelEvaluateAnimation 的最终 EvaluationContext.Curve
│  ├─ Proxy.UpdateCurvesToEvaluationContext：清旧表，写 AttributeCurve
│  └─ GetCurveValue：当前值，缺失为0
│     ⇒ AnimationGraphSet::Final → project_root_animation_context
│        ├─ 当前图唯一Pose输出边，按DataSpec定位，不依赖编辑器节点名
│        ├─ wrapper的 Generated AB_Als / LayeredGraph(capture_context=true)
│        │  └─ AB_Als最终 Output Pose ← Ragdolling ← Control Rig ← Head
│        │     └─ Layering
│        │        ├─ Locomotion输入 ← PostLocomotion Slot ← Locomotion
│        │        │  └─ Transition Slot ← Grounded ← Standing / Crouching
│        │        └─ Overlay输入 ← Inertialization ← 当前Overlay Linked Layer
│        └─ graph_output_context_state → 当前Player的AlsRootAnimationContext
├─ hud_sync_system：所选目标 → assembly.avatar → 当前AnimatedSceneInstance.player
│  ├─ 验证Avatar归属；换实例不继续读仍存活的旧实例
│  └─ 已有实例缺曲线=0；当前实例不可取得时明确N/A
├─ 坐标语义
│  ├─ RotationYawSpeed：源度/秒；源Actor.Yaw += Curve × DeltaTime
│  │  ⇒ 离线pose_curves_ron / montage目标曲线反号；显示时反号还原
│  ├─ RotationYawOffset：源Grounded旋转偏移角
│  │  ⇒ gen_standing_curves反转输出符号；显示时反号还原
│  └─ 其余41项标量保持原值；零值保持0.00
└─ 当前帧绘制/关闭
   ├─ NameToDisplayString；两位小数；灰→白 lerp(clamp(sourceValue,0,1))
   ├─ 字体/行距/列宽/初始偏移走BF123 Canvas合同
   ├─ 分类开启产生独立第一列；关闭只保留深绿色标题（其他分类开启时）
   └─ 不累积历史；仅HUD格式/值缓存，输入值或布局变化即替换
```

源入口：`ALS/Private/AlsCharacter_Debug.cpp::DisplayDebugCurves`、`AlsCharacter.cpp::ApplyRotationYawSpeedAnimationCurve`、`AlsAnimationInstance.cpp::RefreshRotationYawOffsets`；UE `AnimInstance.cpp::GetCurveValue` 与 `AnimInstanceProxy.cpp::UpdateCurvesToEvaluationContext`。目标入口：`src/animation_context_projection.rs`、`crates/als-ext/src/{animation_context,layered_graph}.rs`、`src/show_debug_hud.rs::evaluated_character_curve_rows`。完整动画算法沿已闭合的生成图执行，本项只核对到最终曲线的生产/捕获边，不改动画图。

发现并修复：两项角度曲线的HUD原先误用了目标坐标符号，同时反转了正负值对应的颜色。旧Gate直接拿target曲线值比较，漏掉显示语义；新增±42/零/非角度标量反例及真实左右Turn资产采样。骨架原生导入/check/self-test通过，43项名称无差异；30项范围回归及真实Interface24.721秒功能通过，67值、布局、菜单和19种逐键状态均通过，无运行守卫异常。未修改动画图或曲线资产。

</details>

<details>
<summary>Shift+2 — Als.State：持久意图、实际角色状态与视图模式</summary>

```text
Shift+Two → ShowDebug Als.State ⇒ 公共输入树 / DebugCategory::State
└─ AlsCharacter::DisplayDebugState ⇒ build_state_body（11行）
   ├─ ViewMode：角色自己的Tag ⇒ AlsFirstPersonView
   │  └─ 输入切换即更新；独立于相机激活、渲染Camera和相机动画混合进度
   ├─ LocomotionMode ⇒ LocomotionState（Grounded/InAir/Custom/None/自定义Tag）
   │  └─ 地面/物理支持、动作请求 → state_transition_system → 实际状态
   ├─ bDesiredAiming ⇒ 已消费输入 AlsInputOverride 优先于 AlsInputCommand.aim_held
   ├─ DesiredRotationMode ⇒ AlsLastFreeRotationMode 的持久自由旋转选择
   │  └─ InputRequest切换并保存；Aim/第一人称/Sprint临时选择实际RotationMode
   ├─ RotationMode ⇒ RotationMode组件（当前实际模式）
   ├─ DesiredStance ⇒ AlsDesiredStance
   │  └─ 请求被接受后保留；不能从已清空的AlsStateRequest或实际Stance倒推
   ├─ Stance ⇒ Stance组件（实际碰撞姿态）
   │  └─ 空中强制站立、Roll强制蹲下、头顶受阻等可与DesiredStance不同
   ├─ DesiredGait ⇒ AlsDesiredGait（Walk/Run切换及Sprint按下/释放后的意图）
   ├─ Gait ⇒ Gait组件（速度/姿态/允许步态确定的实际值）
   ├─ OverlayMode ⇒ OverlayState组件（菜单选择经请求消费后的实际Tag）
   └─ LocomotionAction ⇒ AlsNotifyLocomotionAction
      ├─ 动作生命周期置入/结束清空，独立于LocomotionMode
      └─ 没有动作显示None；自定义Tag取真实叶名称
```

共同显示规则：源 `GetSimpleTagName → FName::NameToDisplayString` 对应Tag叶名称/空值None/驼峰显示；全部白色，布尔为True/False，使用BF123的State列宽120、行距12、字体与Canvas合同。PostUpdate在请求消费和物理姿态更新后读取；分类关闭撤下行，无事件历史。

源入口为 `ALS/Private/AlsCharacter_Debug.cpp::DisplayDebugState` 及 `AlsCharacter.cpp::{SetDesiredStance,ApplyDesiredStance,RefreshRotationMode}`；目标生产入口为 `crates/als-core/src/{input_request,aim,gait,transition}.rs`，消费入口为 `src/show_debug_hud.rs`。动作通知桥在PrePhysics后、Roll驱动前，HUD在Final后读取，未增加延迟缓存。

修复与验收：DesiredStance/DesiredRotationMode改读已有持久组件，ViewMode改读角色标记，删除角色布局的相机参数与临时请求回推。25项回归通过，包含期望/实际不同、待处理请求、自定义Tag和无渲染相机反例。Interface24.759秒功能通过，11项State行全部加入数据检查（总计78值），19种按键状态与既有布局/菜单均通过，无运行守卫异常。未改变角色行为。

</details>
