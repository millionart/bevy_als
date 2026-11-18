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

## 构建入口

`als_extras/levels/als_character.rs` 提供最小接入示例。启用 `playground` 后可从 `manual_playground` example 启动完整场景。保留的自动驱动、报告与回归均在 `src/tests/`，JSON 夹具位于 `src/tests/fixtures/`。

`bevy_als` 作为单一 Cargo 库发布公开角色 API。RON 中的 `als_ext::NodeName` 是固定的反射类型标识。
