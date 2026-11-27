use avian3d::prelude::{
    Collider, CollisionHooks, NarrowPhasePlugin, Physics, PhysicsPlugins, PhysicsSchedule,
    PhysicsSchedulePlugin, PhysicsStepSystems, PhysicsSystems,
};
use bevy::app::MainScheduleOrder;
use bevy::ecs::schedule::{ConditionWithAccess, InternedScheduleLabel, ScheduleLabel};
use bevy::ecs::system::{IntoSystem, System, SystemParam, SystemParamItem};
use bevy::prelude::*;
use bevy_animation_graph::{
    AnimationGraphPlugin,
    core::plugin::{AnimationGraphCorePlugin, AnimationGraphSet},
};
use bevy_tnua::{TnuaControllerPlugin, TnuaPipelineSystems, TnuaSystems};
use bevy_tnua_avian3d::TnuaAvian3dPlugin;

use crate::als::aim::AlsAimPlugin;
use crate::als::character::AlsCharacterPlugin;
use crate::als::control_scheme::AlsControlScheme;
use crate::als::input_request::AlsInputRequestPlugin;
use crate::als::motion::AlsMotionPlugin;
use crate::als::transition::AlsStateTransitionPlugin;
use crate::als_camera::AlsCameraPlugin;
use crate::extensions::{AlsExtPlugin, RagdollCollisionHooks};

use crate::character_runtime::AlsCharacterRuntimePlugin;

pub use crate::character_assembly::{
    ALS_LINKED_ANIMATED_SCENE, AVATAR_MESH_ROOT_WORLD_Y, AlsCharacterAvatar, AlsCharacterBlueprint,
    spawn_als_character,
};

pub struct AlsPlayableCharacterPlugin;

/// Schedule that drives Avian and the ALS systems surrounding each physics step.
/// Insert this resource before adding `AlsPlugin` to override `FixedPostUpdate`.
#[derive(Resource, Debug, Clone, Copy)]
pub enum AlsPhysicsSchedule {
    /// Run ALS on the host's Avian physics schedule.
    Avian(InternedScheduleLabel),
    /// Use the original per-frame controller and capped world physics step.
    SourceFrameTiming,
}

impl Default for AlsPhysicsSchedule {
    fn default() -> Self {
        Self::new(FixedPostUpdate)
    }
}

impl AlsPhysicsSchedule {
    /// Selects the schedule that the host runs to advance Avian.
    pub fn new(schedule: impl ScheduleLabel) -> Self {
        Self::Avian(schedule.intern())
    }

    /// Returns the label used for the physics preparation and writeback systems.
    pub fn label(&self) -> InternedScheduleLabel {
        match self {
            Self::Avian(schedule) => *schedule,
            Self::SourceFrameTiming => PostUpdate.intern(),
        }
    }

    pub(crate) fn uses_physics_steps(app: &App) -> bool {
        matches!(app.world().get_resource::<Self>(), Some(Self::Avian(_)))
    }

    pub(crate) fn outer(app: &App) -> InternedScheduleLabel {
        app.world()
            .get_resource::<Self>()
            .map_or_else(|| PostUpdate.intern(), Self::label)
    }

    pub(crate) fn prepare(app: &App) -> InternedScheduleLabel {
        if Self::uses_physics_steps(app) {
            Self::outer(app)
        } else {
            Update.intern()
        }
    }

    pub(crate) fn frame_input(app: &App) -> InternedScheduleLabel {
        if !Self::uses_physics_steps(app) {
            return Update.intern();
        }
        let outer = Self::outer(app);
        let fixed_loop = RunFixedMainLoop.intern();
        let order = app.world().resource::<MainScheduleOrder>();
        if order.labels.iter().take_while(|&&label| label != fixed_loop).any(|&label| label == outer) {
            outer
        } else {
            fixed_loop
        }
    }

    pub(crate) fn controller(app: &App) -> InternedScheduleLabel {
        if Self::uses_physics_steps(app) {
            PhysicsSchedule.intern()
        } else {
            Update.intern()
        }
    }

    pub(crate) fn root_motion(app: &App) -> InternedScheduleLabel {
        if Self::uses_physics_steps(app) {
            PhysicsSchedule.intern()
        } else {
            PostUpdate.intern()
        }
    }
}

/// ALS prepares the movement state here, before Avian solves the next step.
/// Order host movement requests before this set on the selected physics schedule.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsPhysicsPrepareSet;

/// When running physics in `PreUpdate`, order host input writers before this set.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsFrameInputSet;

#[derive(Resource)]
pub(crate) struct AlsHostPhysicsWorld;

#[derive(SystemParam)]
pub(crate) struct AlsStepTime<'w> {
    time: Option<Res<'w, Time>>,
    virtual_time: Res<'w, Time<Virtual>>,
    schedule: Option<Res<'w, AlsPhysicsSchedule>>,
}

impl AlsStepTime<'_> {
    pub(crate) fn delta_secs(&self) -> f32 {
        if let (Some(AlsPhysicsSchedule::Avian(_)), Some(time)) =
            (self.schedule.as_deref(), &self.time)
        {
            time.delta_secs()
        } else {
            self.virtual_time.delta_secs()
        }
    }

    pub(crate) fn is_paused(&self) -> bool {
        self.virtual_time.is_paused()
    }
}

fn install_tnua(app: &mut App, schedule: InternedScheduleLabel) {
    let host_backend = app.is_plugin_added::<TnuaAvian3dPlugin>();
    let host_controller = app.is_plugin_added::<TnuaControllerPlugin<AlsControlScheme>>();
    if !host_backend {
        app.add_plugins(TnuaAvian3dPlugin::new(schedule));
    }
    if !host_controller {
        app.add_plugins(TnuaControllerPlugin::<AlsControlScheme>::new(schedule));
    }
    assert!(app.get_schedule(schedule).is_some_and(|schedule| schedule.graph().system_sets.contains(TnuaPipelineSystems::Logic)),
        "Install the ALS Tnua controller on {schedule:?}");

    if schedule != PhysicsSchedule.intern() && !host_backend {
        return;
    }
    // Identify the backend's own condition without replacing conditions added by the host.
    let mut backend = App::new();
    backend.add_plugins(TnuaAvian3dPlugin::new(schedule));
    let sets = &backend.get_schedule(schedule).unwrap().graph().system_sets;
    let key = sets.get_key(TnuaSystems.intern()).unwrap();
    let [guard] = sets.get_conditions(key).unwrap() else {
        panic!("Expected one run condition from the Tnua Avian backend");
    };
    let guard = guard.system_type();
    let sets = &mut app.get_schedule_mut(schedule).unwrap().graph_mut().system_sets;
    let condition = sets.get_key(TnuaSystems.intern())
        .and_then(|key| sets.get_conditions_mut(key))
        .and_then(|conditions| conditions.iter_mut().find(|condition| condition.system_type() == guard))
        .unwrap_or_else(|| panic!("Install the Tnua Avian backend on {schedule:?} before ALS"));
    if schedule == PhysicsSchedule.intern() {
        // Avian only enters this schedule for a nonzero step, even when manually stepping a paused clock.
        *condition = ConditionWithAccess::new(Box::new(IntoSystem::into_system(
            |time: Res<Time<Physics>>| !time.delta().is_zero(),
        )));
    }
}

impl AlsPlayableCharacterPlugin {
    /// Creates the default fixed-step Avian plugin group with ALS collision hooks.
    pub fn physics_plugins() -> impl PluginGroup {
        Self::physics_plugins_in(FixedPostUpdate)
    }

    /// Creates the Avian plugin group for a host-selected schedule.
    pub fn physics_plugins_in(schedule: impl ScheduleLabel) -> impl PluginGroup {
        PhysicsPlugins::new(schedule).with_collision_hooks::<RagdollCollisionHooks>()
    }

    /// Selects the host's Avian collision hooks. Delegate to `RagdollCollisionHooks`
    /// in the host filter to keep ALS pawn and ragdoll exclusions.
    pub fn with_collision_hooks<H: CollisionHooks + 'static>() -> impl Plugin
    where
        for<'w, 's> SystemParamItem<'w, 's, H>: CollisionHooks,
    {
        Self::build_with_hooks::<H>
    }

    fn build_with_hooks<H: CollisionHooks + 'static>(app: &mut App)
    where
        for<'w, 's> SystemParamItem<'w, 's, H>: CollisionHooks,
    {
        app.init_resource::<AlsPhysicsSchedule>();
        let schedule = app.world().resource::<AlsPhysicsSchedule>().label();
        if app.is_plugin_added::<PhysicsSchedulePlugin>() {
            assert!(
                app.is_plugin_added::<NarrowPhasePlugin<Collider, H>>()
                    && app.get_schedule(schedule).is_some_and(|schedule| schedule
                        .graph()
                        .system_sets
                        .contains(PhysicsSystems::StepSimulation)),
                "Register Avian with the configured collision hooks and the schedule selected by AlsPhysicsSchedule ({schedule:?})"
            );
            app.insert_resource(AlsHostPhysicsWorld);
        } else {
            app.add_plugins(PhysicsPlugins::new(schedule).with_collision_hooks::<H>());
            if schedule == FixedPostUpdate.intern() {
                app.register_required_components::<crate::AlsCharacter, avian3d::interpolation::TransformInterpolation>();
            }
        }
        if AlsPhysicsSchedule::uses_physics_steps(app) {
            let frame_input = AlsPhysicsSchedule::frame_input(app);
            if frame_input == RunFixedMainLoop.intern() {
                app.configure_sets(frame_input, AlsFrameInputSet.in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop));
            }
            if frame_input == schedule {
                app.configure_sets(schedule, AlsFrameInputSet.before(crate::als::input::begin_input_step));
            }
            app.configure_sets(
                schedule,
                AlsPhysicsPrepareSet
                    .run_if(|time: Res<Time>, clock: Res<Time<Physics>>| {
                        crate::als::input::physics_step_due(&time, &clock)
                    })
                    .before(AnimationGraphSet::PrePhysics)
                    .before(PhysicsSystems::First),
            )
            .register_required_components::<crate::AlsCharacter, crate::als::input::AlsStepInput>()
            .add_systems(
                frame_input,
                crate::als::input::buffer_input.in_set(AlsFrameInputSet),
            )
            .add_systems(
                schedule,
                crate::als::input::begin_input_step.before(AlsPhysicsPrepareSet),
            );
            app.configure_sets(
                PhysicsSchedule,
                (
                    crate::mantle_drive::AlsMantleDriveSet,
                    crate::animation_events::AlsAnimationEventSet::Refresh,
                    crate::animation_events::AlsAnimationEventSet::Dispatch,
                    crate::roll_drive::AlsRollDriveSet,
                )
                    .chain()
                    .after(bevy_tnua::TnuaPipelineSystems::Motors)
                    .after(crate::als::motion::auto_mantle_after_movement)
                    .before(PhysicsStepSystems::First),
            );
        }
        let controller = AlsPhysicsSchedule::controller(app);
        install_tnua(app, controller);
        app.add_plugins((
            AlsInputRequestPlugin,
            AlsAimPlugin,
            AlsStateTransitionPlugin,
            AlsCharacterPlugin,
            AlsMotionPlugin,
            crate::als::air::AlsAirPlugin,
            crate::extensions::post_physics_animation::PostPhysicsAnimationPlugin,
        ));
        if let Some(graph) = app.get_added_plugins::<AnimationGraphCorePlugin>().first() {
            assert!(graph.physics_schedule == schedule && graph.final_schedule == PostUpdate.intern(),
                "Install the animation graph plugin with physics on {schedule:?} and pose writeback on PostUpdate");
        } else {
            app.add_plugins(AnimationGraphPlugin::from_physics_schedule(schedule));
        }
        app.add_plugins((
            AlsExtPlugin,
            AlsCharacterRuntimePlugin,
            AlsCameraPlugin,
        ));
    }
}

impl Plugin for AlsPlayableCharacterPlugin {
    fn build(&self, app: &mut App) {
        Self::build_with_hooks::<RagdollCollisionHooks>(app);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playable_character_samples_air_velocity_and_refreshes_the_sensor_before_tnua() {
        use bevy::ecs::schedule::LogLevel;
        use bevy_tnua_avian3d::TnuaAvian3dSensorShape;

        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            TransformPlugin,
            bevy::input::InputPlugin,
            bevy::gizmos::GizmoPlugin,
        ))
        .init_asset::<bevy::world_serialization::WorldAsset>()
        .init_asset::<Mesh>()
        .init_asset::<Image>()
        .add_plugins(crate::AlsPlugin);
        app.finish();
        app.cleanup();
        app.world_mut()
            .schedule_scope(PhysicsSchedule, |world, schedule| {
                let mut settings = schedule.get_build_settings();
                settings.ambiguity_detection = LogLevel::Warn;
                schedule.set_build_settings(settings);
                schedule.initialize(world).unwrap();
                let systems: std::collections::HashMap<_, _> = schedule
                    .systems()
                    .unwrap()
                    .map(|(id, system)| (id, system.name().to_string()))
                    .collect();
                let shape = world.component_id::<TnuaAvian3dSensorShape>().unwrap();
                let impact = world
                    .component_id::<crate::als::air::AlsPreImpactVelocity>()
                    .unwrap();
                let conflicts: Vec<_> = schedule
                    .graph()
                    .conflicting_systems()
                    .0
                    .iter()
                    .filter(|(_, _, components)| {
                        components.contains(&shape) || components.contains(&impact)
                    })
                    .map(|(a, b, _)| (&systems[a], &systems[b]))
                    .collect();
                assert!(
                    conflicts.is_empty(),
                    "Unordered sensor shape or pre-impact accesses: {conflicts:?}"
                );
            });
    }

    #[test]
    fn playable_character_animation_phases_initialize_with_the_real_physics_schedule() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            TransformPlugin,
            bevy::input::InputPlugin,
            bevy::gizmos::GizmoPlugin,
        ))
        .init_asset::<bevy::world_serialization::WorldAsset>()
        .init_asset::<Mesh>()
        .init_asset::<Image>()
        .add_plugins(crate::AlsPlugin::physics_plugins())
        .add_plugins(crate::AlsPlugin)
        .configure_sets(
            PostUpdate,
            bevy::camera::CameraUpdateSystems.before(bevy::asset::AssetEventSystems),
        );
        app.finish();
        app.cleanup();
        for label in [FixedPostUpdate.intern(), PostUpdate.intern()] {
            app.world_mut().schedule_scope(label, |world, schedule| {
                schedule
                    .initialize(world)
                    .expect("playable character animation phases must initialize");
            });
        }
    }

    #[test]
    fn selected_host_hooks_can_share_preinstalled_fixed_step_physics() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            TransformPlugin,
            bevy::input::InputPlugin,
            bevy::gizmos::GizmoPlugin,
        ))
        .init_asset::<bevy::world_serialization::WorldAsset>()
        .init_asset::<Mesh>()
        .init_asset::<Image>()
        .add_plugins(PhysicsPlugins::new(FixedPostUpdate).with_collision_hooks::<()>())
        .add_plugins(crate::AlsPlugin::with_collision_hooks::<()>());
        assert!(app.is_plugin_added::<NarrowPhasePlugin<Collider, ()>>());
        assert!(app.is_plugin_added::<AlsCharacterRuntimePlugin>());
    }

    #[test]
    #[should_panic(expected = "the schedule selected by AlsPhysicsSchedule")]
    fn playable_character_rejects_physics_on_a_different_schedule() {
        App::new()
            .add_plugins((
                MinimalPlugins,
                AssetPlugin::default(),
                TransformPlugin,
                PhysicsPlugins::new(PostUpdate).with_collision_hooks::<RagdollCollisionHooks>(),
            ))
            .add_plugins(crate::AlsPlugin);
    }
}
