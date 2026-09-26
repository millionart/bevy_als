use crate::extensions::{RagdollPhysicsProfile, SourcePhysicsJointPlugin};
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;

use crate::character_assembly::AlsCharacterAvatar;

const PA_ALS_PROFILE: &str = include_str!("../../../assets/ragdolls/pa_als.physics.json");

#[derive(Resource, Clone)]
pub struct PaAlsPhysicsProfileTemplate(pub RagdollPhysicsProfile);

impl FromWorld for PaAlsPhysicsProfileTemplate {
    fn from_world(_world: &mut World) -> Self {
        Self(parse_pa_als_profile())
    }
}

pub struct PaAlsPhysicsPlugin;

impl Plugin for PaAlsPhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SourcePhysicsJointPlugin)
            .init_resource::<PaAlsPhysicsProfileTemplate>()
            .add_systems(Update, attach_pa_als_physics_profile);
    }
}

pub fn parse_pa_als_profile() -> RagdollPhysicsProfile {
    RagdollPhysicsProfile::from_json(PA_ALS_PROFILE).expect("PA_Als target physics profile parses")
}

fn attach_pa_als_physics_profile(
    mut commands: Commands,
    avatars: Query<&AnimatedSceneInstance, With<AlsCharacterAvatar>>,
    players: Query<(), (With<bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer>, Without<RagdollPhysicsProfile>)>,
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
    fn pa_als_profile_has_full_source_topology() {
        let profile = parse_pa_als_profile();
        assert_eq!(profile.bodies.len(), 21);
        assert_eq!(profile.colliders.len(), 22);
        assert!(profile.colliders.iter().filter(|shape| shape.label.starts_with("root_")).all(|shape| !shape.consider_for_bounds));
        assert_eq!(profile.colliders.iter().filter(|shape| shape.consider_for_bounds).count(), 21);
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
