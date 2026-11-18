# 源码与 ALS-Refactored 的对应关系

源码统一在此目录组织。底层引擎/插件能力与 ALS 移植并列；ALS 的分区以本地 ALS-Refactored 的 Source 模块和 Content 目录为依据。Rust 不复制 C++ 的 Public/Private 头文件层。

```text
src/
  animation_graph_ext/         bevy_animation_graph 求值、节点、曲线、Rig 调用与 PhysicsAsset 装配
  avian_ext/                   源物理差异的补足，含 SceneQuery、几何查询、刚体与关节求解
  bevy_ext/                    投射贴花、文本、后处理和时间辅助
  control_rig_ext/             源工程自定义 Control Rig 单位及测试
  tnua_ext/                    控制器辅助与项目物理时钟适配
  als/
    character/
      actions/                AlsCharacter_Actions 与动作驱动
      animation_instances/    AlsAnimationInstance 与 Character/AnimationInstances
      movement/               AlsCharacterMovementComponent 的 ALS 运动策略和 Tnua 接线
    nodes/                    ALS 专用动画节点
    notifies/                 动画事件、脚步效果和转换通知
    root_motion_sources/      Mantling 与根运动驱动
    settings/                 ALS 配置与离线转换的目标常量
    state/                    ALS State、标签、状态转换与状态计算
    utility/                  ALS 骨骼、方向和原生 Debug Display
    overlay_objects/          ALS/OverlayObjects
  als_camera/                 ALSCamera 组件、设置与相机震动
  als_extras/
    core/                     GameMode、PlayerStart 与示例输入
    environment/              Playground 环境材质
    levels/                   Playground 场景、碰撞与可运行入口
    ui/                       W_Als_Hud、W_Als_OverlayModeMenu/Option 的实际实现
```

`ALSExtras/Levels`、`Environment`、`Core`、`UI` 是源工程的实际目录。`als_extras/ui` 包含 HUD 和 Overlay 菜单代码；源 `AlsCharacter_Debug.cpp::DisplayDebug` 对应的调试画布位于 `als/utility/show_debug_hud`，它与 Extras 的 HUD 分属不同来源。UI 的离线生成数据位于 `assets/ALSExtras/UI`。

`als/character/movement` 中的速度、步阶、蹲起、旋转与源物理时钟/角色整帧策略对标源 `AlsCharacterMovementComponent`，所以属于 ALS。通用控制器辅助在 `tnua_ext`。`animation_graph_ext` 承载图基础能力；源 ALS 自定义节点、动作和骨骼选择属于 `als`。同文件中现存的交叉依赖未因移动文件自动消失，后续反哺上游按具体能力的依赖闭包提取。

角色四向速度混合位于动画实例的 `actual_motion`。角色、相机及 Overlay 道具的源材质槽接线位于 `als/character/overlay_materials.rs`。碰撞命中的物理表面位于 `SceneQuery` 所在模块，供脚步效果消费。

## 关卡入口与自动场景

`playground` feature 提供 `manual_playground` example、Playground 场景和 UI。`als_extras/levels/manual_playground.rs` 组装原地形、角色、相机、环境和 UI，由 `run_manual.ps1`（Windows 包装为 `run_manual.bat`）启动。

`tools/manual_playground_integration/mod.rs` 提供移植过程中的自动驱动 `ManualPlaygroundIntegrationPlugin`。只有设置 `ALS_MANUAL_INTEGRATION_GATE` 才注册它；它驱动移动、蹲起、动作、镜头、UI 等场景，检查状态/姿态并发布覆盖率、失败与进展报告。公共 runner 的动态场景仍依赖它；它不是另一个 Playground。

`tools/animation_visual_validation.rs` 收集公共视觉和数值报告，`tools/anim_probe.rs` 提供诊断。两者与自动驱动通过 `playground` feature 或库单元测试编译，可直接读取 crate 内遥测。角色输入和状态系统由 `AlsPlugin` 注册，供宿主独立使用。

## Cargo 与资产身份

项目使用单一 Cargo 包 `bevy_als`。`als/mod.rs`、`als_camera/mod.rs` 和 `extensions.rs` 是模块与类型注册入口。专项程序位于 `tools/examples`，集成测试位于 `tools/tests`，源码单元测试与被测代码同文件。Cargo.toml 声明 example 和 test 路径，以及依赖 Playground 的示例所需 feature。

RigVM、层级、数学、IK 和已有的 UE 原生 Control Rig 单位来自锁定的 `bevy_control_rig` Git 依赖。`control_rig_ext` 按源标识执行 ALS 自定义单位。角色层提供 Rig 的查询上下文和单位接线，`animation_graph_ext` 的图节点负责传递当前帧的 Pose、曲线、调用时机和结果。PhysicsAsset 的骨骼刚体装配和动画关节目标也由 `animation_graph_ext` 管理。

消费方从 `bevy_als` 根模块导入 `AlsPlugin`、角色生成和状态请求。RON 中已有的 `als_ext::NodeName` 是显式固定的反射类型名，用于资产兼容。
