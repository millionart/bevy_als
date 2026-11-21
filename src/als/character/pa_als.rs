use crate::extensions::{RagdollPhysicsProfile, SourcePhysicsJointPlugin};
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;

use crate::character_assembly::AlsCharacterAvatar;

const PA_ALS_PROFILE: &str = include_str!("../../../assets/ALS/Character/PA_Als.physics.json");
const WORLD_SOLVER_ITERATIONS: [u32; 3] = include!("../settings/world_solver_iterations.rs");

#[derive(Resource, Clone)]
pub struct PaAlsPhysicsProfileTemplate(pub RagdollPhysicsProfile);

impl FromWorld for PaAlsPhysicsProfileTemplate {
    fn from_world(_world: &mut World) -> Self {
        let mut profile = parse_pa_als_profile();
        let [position, velocity, projection] = WORLD_SOLVER_ITERATIONS;
        profile.solver_settings.position_iterations = position;
        profile.solver_settings.velocity_iterations = velocity;
        profile.solver_settings.projection_iterations = projection;
        for joint in &mut profile.joints {
            joint.definition.settings.position_iterations = position;
            joint.definition.settings.velocity_iterations = velocity;
            joint.definition.settings.projection_iterations = projection;
        }
        Self(profile)
    }
}

pub struct PaAlsPhysicsPlugin;

impl Plugin for PaAlsPhysicsPlugin {
    fn build(&self, app: &mut App) {
        let prepare = crate::AlsPhysicsSchedule::prepare(app);
        app.add_plugins(SourcePhysicsJointPlugin)
            .init_resource::<PaAlsPhysicsProfileTemplate>()
            .add_systems(
                prepare,
                attach_pa_als_physics_profile.in_set(crate::playable_character::AlsPhysicsPrepareSet),
            );
    }
}

pub fn parse_pa_als_profile() -> RagdollPhysicsProfile {
    RagdollPhysicsProfile::from_json(PA_ALS_PROFILE).expect("PA_Als target physics profile parses")
}

fn attach_pa_als_physics_profile(
    mut commands: Commands,
    avatars: Query<&AnimatedSceneInstance, With<AlsCharacterAvatar>>,
    players: Query<
        (),
        (
            With<bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer>,
            Without<RagdollPhysicsProfile>,
        ),
    >,
    template: Res<PaAlsPhysicsProfileTemplate>,
) {
    for instance in &avatars {
        let player = instance.player_entity();
        if !players.contains(player) {
            continue;
        }
        commands.entity(player).insert(template.0.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_ragdoll_template_uses_world_solver_iterations() {
        let mut world = World::new();
        let template = PaAlsPhysicsProfileTemplate::from_world(&mut world);
        let settings = template.0.solver_settings;
        assert_eq!(
            (
                settings.position_iterations,
                settings.velocity_iterations,
                settings.projection_iterations
            ),
            (8, 2, 1)
        );
        let joints = template
            .0
            .joints
            .iter()
            .filter(|joint| joint.enabled)
            .collect::<Vec<_>>();
        assert_eq!(joints.len(), 19);
        assert!(joints.iter().all(|joint| (
            joint.definition.settings.position_iterations,
            joint.definition.settings.velocity_iterations,
            joint.definition.settings.projection_iterations
        ) == (8, 2, 1)));
        let asset = parse_pa_als_profile();
        assert_eq!(
            (
                asset.solver_settings.position_iterations,
                asset.solver_settings.velocity_iterations,
                asset.solver_settings.projection_iterations
            ),
            (6, 1, 1)
        );
    }

    #[test]
    fn loaded_world_ragdoll_joints_use_world_solver_iterations() {
        use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
        let (mut app, _, avatar) = crate::roll_drive::tests::loaded_roll_character();
        let (player, _) = crate::ragdoll_link::tests::loaded_ragdoll_bodies(&mut app, avatar);
        for _ in 0..120 {
            if app
                .world()
                .get::<crate::extensions::RagdollPhysicsProfileApplied>(player)
                .is_some_and(|applied| applied.source_joint_count == 19)
            {
                break;
            }
            app.update();
        }
        let joints = app
            .world()
            .get::<AnimationGraphPlayer>(player)
            .unwrap()
            .get_spawned_ragdoll()
            .unwrap()
            .joints
            .values()
            .copied()
            .collect::<Vec<_>>();
        let active = joints
            .into_iter()
            .filter_map(|entity| {
                if app
                    .world()
                    .get::<avian3d::prelude::JointDisabled>(entity)
                    .is_some()
                {
                    return None;
                }
                app.world()
                    .get::<crate::extensions::source_physics_joint::SourcePhysicsJoint>(entity)
            })
            .collect::<Vec<_>>();
        assert_eq!(active.len(), 19);
        assert!(active.iter().all(|joint| (
            joint.settings.position_iterations,
            joint.settings.velocity_iterations,
            joint.settings.projection_iterations
        ) == (8, 2, 1)));
    }

    #[test]
    fn pa_als_profile_has_full_source_topology() {
        let profile = parse_pa_als_profile();
        assert_eq!(profile.bodies.len(), 21);
        assert_eq!(profile.colliders.len(), 22);
        assert!(
            profile
                .colliders
                .iter()
                .filter(|shape| shape.label.starts_with("root_"))
                .all(|shape| !shape.consider_for_bounds)
        );
        assert_eq!(
            profile
                .colliders
                .iter()
                .filter(|shape| shape.consider_for_bounds)
                .count(),
            21
        );
        assert_eq!(profile.joints.len(), 20);
        assert_eq!(profile.collision_disabled_pairs.len(), 49);
        assert!(profile.bodies.iter().any(|body| body.label == "root"
            && body.active_mode
                == bevy_animation_graph::core::ragdoll::definition::BodyMode::Kinematic));
        assert!(
            profile
                .joints
                .iter()
                .any(|joint| joint.label == "pelvis" && !joint.enabled)
        );
        assert_eq!(
            profile.joints.iter().filter(|joint| joint.enabled).count(),
            19
        );
        assert_eq!(
            profile
                .joints
                .iter()
                .filter(|joint| joint.definition.linear_locked)
                .count(),
            19,
            "only the source-free pelvis joint is excluded from locked-anchor diagnostics"
        );
    }

    #[test]
    fn pa_als_plugin_registers_source_joint_solvers_after_physics() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, avian3d::prelude::PhysicsPlugins::default()))
            .add_plugins(PaAlsPhysicsPlugin);
    }
}
