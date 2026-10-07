# 源码与 ALS-Refactored 的对应关系

源码统一在此目录组织。底层引擎/插件能力与 ALS 移植并列；ALS 的分区以本地 ALS-Refactored 的 Source 模块和 Content 目录为依据。Rust 不复制 C++ 的 Public/Private 头文件层。

```text
src/
  animation_graph_ext/         bevy_animation_graph 求值、节点、同步、曲线及加载扩展
  avian_ext/                   物理关节、驱动、PhysicsAsset 和查询辅助
  bevy_ext/                    材质、贴花、文本和时间辅助
  control_rig/                 UE RigVM、层级、数学与 IK 语义
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

## 关卡入口与自动场景

`als_extras/levels/manual_playground.rs` 是 `manual_playground` example 的程序入口：组装原地形、角色、相机、环境和 UI，供 `run_manual.bat` 启动。

`tools/manual_playground_integration/mod.rs` 提供移植过程中的自动驱动 `ManualPlaygroundIntegrationPlugin`。只有设置 `ALS_MANUAL_INTEGRATION_GATE` 才注册它；它驱动移动、蹲起、动作、镜头、UI 等场景，检查状态/姿态并发布覆盖率、失败与进展报告。公共 runner 的动态场景仍依赖它；它不是另一个 Playground。

`tools/animation_visual_validation.rs` 收集公共视觉/数值报告，`tools/anim_probe.rs` 提供诊断。这些工具通过 `src/lib.rs` 的显式路径保留现有模块访问关系，继续读取私有遥测并被库测试复用；移除验收工具时同时移除这三处模块声明和关卡的工具接线。原先无消费者的旧 `debug_hud` 状态面板已删除，源 ALS 的 `show_debug_hud` 继续保留。

## Cargo 与资产身份

项目只有一个 Cargo 包 `bevy_als`，不再保留 `crates` 目录。`als/mod.rs`、`als_camera/mod.rs` 和 `extensions.rs` 是内部模块/类型注册入口。专项程序位于 `tools/examples`，集成测试位于 `tools/tests`，源码单元测试仍与被测代码同文件；Cargo.toml 显式声明 example/test 路径，现有命令保持不变。

Rust 调用使用 `bevy_als::als`、`bevy_als::als_camera`、`bevy_als::extensions`，不再引用旧独立 crate。RON 中已有的 `als_ext::NodeName` 是显式固定的反射类型名，继续用于资产兼容，不代表磁盘上仍有 als-ext crate。`cargo run --example manual_playground` 和 `bevy_als::manual_playground_integration` 保持原入口。
