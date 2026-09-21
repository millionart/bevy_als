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
