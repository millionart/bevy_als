//! M1.3 输入抽象层。
//!
//! 基于 `bevy_enhanced_input`（对应 UE Enhanced Input）定义 ALS-Refactored
//! `AlsCharacterInputComponent`（IA_Move/Look/Jump/Crouch/Sprint/Aim/Roll）的等价物，
//! 并桥接为设备无关的 [`AlsInputCommand`] 组件，供 M1.4 角色组装与 M3 tnua 参数映射消费。
//!
//! 结构与 UE 的对应关系：
//! - [`MoveAction`] 等七个 action ≈ UE `UALSInputConfig` 中的 IA_* 资产；
//! - [`OnFoot`] 上下文 ≈ UE ALS 的 OnFoot 输入上下文（IMC）；
//! - [`als_input_bridge_system`] ≈ UE InputComponent 绑定的输入回调聚合；
//! - [`AlsInputCommand`] ≈ UE 中散落在 Character 上的 bSprintRequest 等输入标志的集中化。
//!
//! 使用方式：先添加 `EnhancedInputPlugin`，再添加 [`AlsInputPlugin`]；
//! 玩家实体生成后调用 [`spawn_als_actions`] 完成一键绑定。

use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;
// 与 bevy::prelude 的同名类型区分，显式引入 enhanced_input 版本（官方建议写法）。
use bevy_enhanced_input::prelude::Press as EIPress;

/// 移动输入，输出平面位移向量。
///
/// x 轴右正（D/左摇杆右），y 轴前正（W/左摇杆上），对应 UE `IA_Move`（Axis2D）。
#[derive(InputAction)]
#[action_output(Vec2)]
pub struct MoveAction;

/// 视角输入，输出本帧视角增量向量。
///
/// 来源为鼠标移动增量或右摇杆偏移，对应 UE `IA_Look`（Axis2D）。
#[derive(InputAction)]
#[action_output(Vec2)]
pub struct LookAction;

/// 跳跃按键。未附加条件（等价 `Down`）：按住期间持续 Fired。
///
/// 对应 UE `IA_Jump`；边沿信息由桥接系统从 [`ActionEvents::START`] 提取。
#[derive(InputAction)]
#[action_output(bool)]
pub struct JumpAction;

/// 蹲姿切换键。附加 `Press` 条件：每次按下只触发一次（边沿语义），
/// 由下游状态机（M1.2/M1.4）执行站/蹲切换，对应 UE `IA_Crouch`（ToggleStance）。
#[derive(InputAction)]
#[action_output(bool)]
pub struct CrouchAction;

/// 冲刺按键。无条件按住语义，对应 UE `IA_Sprint`（bSprintRequest 按住式）。
#[derive(InputAction)]
#[action_output(bool)]
pub struct SprintAction;

/// 瞄准按键。无条件按住语义，对应 UE `IA_Aim`（RotationMode 切换为 Aiming 的持续请求）。
#[derive(InputAction)]
#[action_output(bool)]
pub struct AimAction;

/// 翻滚触发键。附加 `Press` 条件（边沿），对应 UE ALS 的 Rolling 行为入口。
#[derive(InputAction)]
#[action_output(bool)]
pub struct RollAction;

/// 地面行走上下文标记，对应 ALS 的 OnFoot 层级。
///
/// 必须经 [`AlsInputPlugin`] 注册（内部调用 `add_input_context::<OnFoot>`）
/// 后其 action 才会被求值；后续载具等上下文可平行扩展新标记组件。
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OnFoot;

/// 设备无关的语义输入命令集合（挂在玩家实体上）。
///
/// 字段分两类：
/// - **连续量**（`move_dir` / `look_delta` / `*_held`）：由桥接系统每帧整体覆写，
///   直接反映当前输入状态，消费方只读不清理；
/// - **边沿量**（`*_pressed`）：仅触发帧为 `true`。桥接系统每帧先重置再写入，
///   因此天然具备"当帧有效"语义；消费方不得依赖跨帧残留值。
///
/// 对应 UE ALS 中 `bJumpRequested` 等一次性请求标志与轴输入的组合。
#[derive(
    Component, Debug, Default, Clone, Copy, PartialEq, Reflect,
)]
#[reflect(Default)]
pub struct AlsInputCommand {
    /// 连续量：移动方向（x 右正，y 前正），来自 [`MoveAction`]。
    pub move_dir: Vec2,
    /// 连续量：视角增量，来自 [`LookAction`]。
    pub look_delta: Vec2,
    /// 连续量：跳跃按住中（tnua 可变跳高等场景使用）。
    pub jump_held: bool,
    /// 边沿量：本帧按下跳跃。
    pub jump_pressed: bool,
    /// 边沿量：本帧按下蹲姿切换。
    pub crouch_pressed: bool,
    /// 边沿量：本帧触发翻滚。
    pub roll_pressed: bool,
    /// 连续量：冲刺按住中。
    pub sprint_held: bool,
    /// 连续量：瞄准按住中。
    pub aim_held: bool,
}

impl AlsInputCommand {
    /// 清零全部字段（连续量归零、边沿量置 false）。
    ///
    /// 桥接系统每帧自动调用；手动驱动（回放/AI 注入）后也应调用以避免边沿残留。
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// 键位映射表（纯数据，可在生成 action 前任意覆写以支持自定义映射）。
///
/// 默认值为 ALS 惯例键位；手柄映射为项目约定而非 ALS 原始默认，
/// Roll 在 ALS v4 中无独立键位（行为触发），此处给出占位默认。
#[derive(Debug, Clone)]
pub struct AlsInputMap {
    /// 前进键。
    pub move_north: Binding,
    /// 后退键。
    pub move_south: Binding,
    /// 左移键。
    pub move_west: Binding,
    /// 右移键。
    pub move_east: Binding,
    /// 左摇杆 [x, y]。
    pub move_stick: [Binding; 2],
    /// 鼠标视角绑定。
    pub look_mouse: Binding,
    /// 右摇杆 [x, y]。
    pub look_stick: [Binding; 2],
    /// 跳跃按键组。
    pub jump: Vec<Binding>,
    /// 蹲姿切换按键组。
    pub crouch: Vec<Binding>,
    /// 冲刺按键组。
    pub sprint: Vec<Binding>,
    /// 瞄准按键组。
    pub aim: Vec<Binding>,
    /// 翻滚按键组。
    pub roll: Vec<Binding>,
}

impl Default for AlsInputMap {
    fn default() -> Self {
        Self {
            move_north: KeyCode::KeyW.into(),
            move_south: KeyCode::KeyS.into(),
            move_west: KeyCode::KeyA.into(),
            move_east: KeyCode::KeyD.into(),
            move_stick: [
                GamepadAxis::LeftStickX.into(),
                GamepadAxis::LeftStickY.into(),
            ],
            look_mouse: Binding::mouse_motion(),
            look_stick: [
                GamepadAxis::RightStickX.into(),
                GamepadAxis::RightStickY.into(),
            ],
            jump: vec![KeyCode::Space.into(), GamepadButton::South.into()],
            crouch: vec![
                KeyCode::ControlLeft.into(),
                GamepadButton::RightTrigger.into(),
            ],
            sprint: vec![KeyCode::ShiftLeft.into(), GamepadButton::LeftThumb.into()],
            aim: vec![
                MouseButton::Right.into(),
                GamepadButton::LeftTrigger2.into(),
            ],
            roll: vec![KeyCode::AltLeft.into(), GamepadButton::East.into()],
        }
    }
}

/// 为玩家实体一键生成 [`OnFoot`] 上下文、全部七个 action 及默认键位绑定，
/// 并插入清零的 [`AlsInputCommand`]（由 [`als_input_bridge_system`] 每帧填充）。
///
/// 覆盖映射的三种方式：
/// 1. 修改传入的 [`AlsInputMap`] 字段后再调用本函数（推荐，如读档应用玩家自定义键位）；
/// 2. 仅替换某组 `Vec<Binding>`（保留其他默认）；
/// 3. 完全自定义：对带 [`OnFoot`] 的实体自行用 `actions!`/`bindings!` 生成
///    本模块的 [`Action`] 实体，[`als_input_bridge_system`] 会照常桥接。
pub fn spawn_als_actions(cmds: &mut Commands, player: Entity, map: &AlsInputMap) {
    cmds.entity(player).insert((
        OnFoot,
        AlsInputCommand::default(),
        actions!(OnFoot[
            (
                Action::<MoveAction>::new(),
                Bindings::spawn((
                    Bidirectional::new(map.move_east, map.move_west),
                    Bidirectional::new(map.move_north, map.move_south)
                        .with(SwizzleAxis::YXZ),
                    Axial::new(map.move_stick[0], map.move_stick[1])
                        .with(DeadZone::default()),
                )),
            ),
            (
                Action::<LookAction>::new(),
                Bindings::spawn((
                    vec![map.look_mouse],
                    Axial::new(map.look_stick[0], map.look_stick[1])
                        .with(DeadZone::default()),
                )),
            ),
            (
                Action::<JumpAction>::new(),
                Bindings::spawn(map.jump.clone()),
            ),
            (
                Action::<CrouchAction>::new(),
                EIPress::default(),
                Bindings::spawn(map.crouch.clone()),
            ),
            (
                Action::<SprintAction>::new(),
                Bindings::spawn(map.sprint.clone()),
            ),
            (
                Action::<AimAction>::new(),
                Bindings::spawn(map.aim.clone()),
            ),
            (
                Action::<RollAction>::new(),
                EIPress::default(),
                Bindings::spawn(map.roll.clone()),
            ),
        ]),
    ));
}

/// 桥接系统：把 enhanced_input 的 action 状态写入同实体的 [`AlsInputCommand`]。
///
/// 运行于 `PreUpdate`、排在 `EnhancedInputSystems::Apply` 之后（见 [`AlsInputPlugin`]）。
/// 对每个带 [`AlsInputCommand`] 与 [`Actions<OnFoot>`] 的实体：先重置命令再逐项写入；
/// 缺失某个 action（未生成/已移除）时对应字段保持零值，不报错。
/// 下游系统应在本系统之后（同帧 Update/FixedMain）读取命令组件。
pub fn als_input_bridge_system(
    mut receivers: Query<(&mut AlsInputCommand, &Actions<OnFoot>)>,
    moves: Query<&Action<MoveAction>>,
    looks: Query<&Action<LookAction>>,
    jumps: Query<(&TriggerState, &ActionEvents), With<Action<JumpAction>>>,
    sprints: Query<&TriggerState, With<Action<SprintAction>>>,
    aims: Query<&TriggerState, With<Action<AimAction>>>,
    crouches: Query<&ActionEvents, With<Action<CrouchAction>>>,
    rolls: Query<&ActionEvents, With<Action<RollAction>>>,
) {
    for (mut cmd, actions) in &mut receivers {
        cmd.reset();

        if let Some(mv) = moves.iter_many(actions).next() {
            cmd.move_dir = **mv;
        }

        if let Some(lk) = looks.iter_many(actions).next() {
            cmd.look_delta = **lk;
        }

        if let Some((state, events)) = jumps.iter_many(actions).next() {
            cmd.jump_held = *state == TriggerState::Fired;
            cmd.jump_pressed = events.contains(ActionEvents::START);
        }

        if let Some(state) = sprints.iter_many(actions).next() {
            cmd.sprint_held = *state == TriggerState::Fired;
        }

        if let Some(state) = aims.iter_many(actions).next() {
            cmd.aim_held = *state == TriggerState::Fired;
        }

        if let Some(events) = crouches.iter_many(actions).next() {
            cmd.crouch_pressed = events.contains(ActionEvents::START);
        }

        if let Some(events) = rolls.iter_many(actions).next() {
            cmd.roll_pressed = events.contains(ActionEvents::START);
        }
    }
}

/// ALS 输入插件：注册 [`OnFoot`] 输入上下文并安装桥接系统。
///
/// 必须在 `EnhancedInputPlugin` 之后添加（其 build 依赖前者插入的注册表资源，
/// 且上下文注册须发生在该插件 finish 之前）。键位绑定不在插件内完成——
/// 由玩家实体生成方调用 [`spawn_als_actions`] 并传入（可定制的）[`AlsInputMap`]。
pub struct AlsInputPlugin;

impl Plugin for AlsInputPlugin {
    fn build(&self, app: &mut App) {
        app.add_input_context::<OnFoot>().add_systems(
            PreUpdate,
            als_input_bridge_system.after(EnhancedInputSystems::Apply),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{ecs::system::RunSystemOnce, MinimalPlugins};

    #[test]
    fn default_command_is_zeroed() {
        let cmd = AlsInputCommand::default();
        assert_eq!(cmd.move_dir, Vec2::ZERO);
        assert_eq!(cmd.look_delta, Vec2::ZERO);
        assert!(!cmd.jump_held);
        assert!(!cmd.jump_pressed);
        assert!(!cmd.crouch_pressed);
        assert!(!cmd.roll_pressed);
        assert!(!cmd.sprint_held);
        assert!(!cmd.aim_held);
    }

    #[test]
    fn reset_clears_continuous_and_edge_fields() {
        let mut cmd = AlsInputCommand {
            move_dir: Vec2::ONE,
            look_delta: Vec2::NEG_ONE,
            jump_held: true,
            jump_pressed: true,
            crouch_pressed: true,
            roll_pressed: true,
            sprint_held: true,
            aim_held: true,
        };
        cmd.reset();
        assert_eq!(
            cmd,
            AlsInputCommand::default(),
            "reset 后应与默认值完全一致"
        );
    }

    #[test]
    fn bridge_tolerates_missing_actions_and_entities() {
        let mut world = World::new();

        // 玩家实体尚未生成的情形不应影响系统运行。
        world.run_system_once(als_input_bridge_system).unwrap();

        // 有玩家但没有 action（或 action 未被求值）时：脏数据应被清零且不 panic。
        let player = world
            .spawn((
                AlsInputCommand {
                    move_dir: Vec2::ONE,
                    jump_pressed: true,
                    sprint_held: true,
                    ..Default::default()
                },
                Actions::<OnFoot>::default(),
            ))
            .id();
        world.run_system_once(als_input_bridge_system).unwrap();

        let cmd = world.get::<AlsInputCommand>(player).unwrap();
        assert_eq!(*cmd, AlsInputCommand::default());
    }

    #[test]
    fn spawn_creates_seven_actions_with_bindings() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(EnhancedInputPlugin)
            .add_plugins(AlsInputPlugin);
        // 直接用 update() 驱动时需手动完成插件初始化（finish 装配上下文系统）。
        app.finish();

        let player = app.world_mut().spawn_empty().id();
        let mut cmds = app.world_mut().commands();
        spawn_als_actions(&mut cmds, player, &AlsInputMap::default());
        app.update();

        let world = app.world_mut();
        assert_eq!(world.get::<Actions<OnFoot>>(player).unwrap().len(), 7);

        // 每个 action 都应有至少一个 binding 实体。
        for action in world.get::<Actions<OnFoot>>(player).unwrap().iter() {
            let bindings = world.get::<Bindings>(action).unwrap();
            assert!(!bindings.is_empty(), "action {action:?} 应有绑定");
        }

        // 重复调用不应 panic（换绑场景）。
        let mut cmds = app.world_mut().commands();
        spawn_als_actions(&mut cmds, player, &AlsInputMap::default());
        app.update();
    }

    #[test]
    fn full_chain_from_action_mock_to_command() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(EnhancedInputPlugin)
            .add_plugins(AlsInputPlugin);
        app.finish();

        let player = app.world_mut().spawn_empty().id();
        let mut cmds = app.world_mut().commands();
        spawn_als_actions(&mut cmds, player, &AlsInputMap::default());
        app.update();

        // 经官方推荐的 mock 通路模拟一帧输入（绕过物理设备，事件照常派发）：
        // 跳跃/蹲姿/翻滚为单次触发，冲刺/瞄准按住，移动持续向右上。
        app.world_mut()
            .commands()
            .entity(player)
            .mock_once::<OnFoot, JumpAction>(TriggerState::Fired, true)
            .mock_once::<OnFoot, CrouchAction>(TriggerState::Fired, true)
            .mock_once::<OnFoot, RollAction>(TriggerState::Fired, true)
            .mock::<OnFoot, SprintAction>(
                TriggerState::Fired,
                true,
                MockSpan::Updates(50),
            )
            .mock::<OnFoot, AimAction>(
                TriggerState::Fired,
                true,
                MockSpan::Updates(50),
            )
            .mock::<OnFoot, MoveAction>(
                TriggerState::Fired,
                Vec2::new(1.0, 0.5),
                MockSpan::Updates(50),
            );

        app.update();
        let cmd = *app.world().get::<AlsInputCommand>(player).unwrap();
        assert!(cmd.jump_pressed && cmd.jump_held, "跳跃边沿+按住");
        assert!(cmd.crouch_pressed, "蹲姿切换边沿");
        assert!(cmd.roll_pressed, "翻滚边沿");
        assert!(cmd.sprint_held, "冲刺按住");
        assert!(cmd.aim_held, "瞄准按住");
        assert!(
            (cmd.move_dir - Vec2::new(1.0, 0.5)).length() < 1e-5,
            "move_dir 应等于 mock 值，实际 {:?}",
            cmd.move_dir
        );

        // 第二帧：边沿必须清零，长按量保持。
        app.update();
        let cmd = *app.world().get::<AlsInputCommand>(player).unwrap();
        assert!(!cmd.jump_pressed);
        assert!(!cmd.jump_held);
        assert!(!cmd.crouch_pressed);
        assert!(!cmd.roll_pressed);
        assert!(cmd.sprint_held);
        assert!(cmd.aim_held);
        assert!(
            (cmd.move_dir - Vec2::new(1.0, 0.5)).length() < 1e-5,
            "长按 span 内 move_dir 应保持"
        );

        // 手动关闭全部 mock 后第三帧应全部归零。
        let actions = app
            .world()
            .get::<Actions<OnFoot>>(player)
            .unwrap()
            .iter()
            .collect::<Vec<_>>();
        for action in actions {
            if let Some(mut mock) = app.world_mut().get_mut::<ActionMock>(action) {
                mock.enabled = false;
            }
        }
        app.update();
        let cmd = *app.world().get::<AlsInputCommand>(player).unwrap();
        assert_eq!(cmd, AlsInputCommand::default());
    }
}
