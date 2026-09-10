use als_core::character::AlsCharacter;
use als_core::start_stop::{StartStopState, start_stop_detector_system};
use bevy::prelude::*;
use bevy_gameplay_tag::GameplayTagsPlugin;

use crate::ab_als_root::AlsRootGraphPlugin;
use crate::actual_motion::AlsActualMotionPlugin;
use crate::air_jump_layer::AirJumpLayerPlugin;
use crate::animation_events::AlsAnimationEventPlugin;
use crate::control_rig_apply::ControlRigApplyPlugin;
use crate::crouching_layer::CrouchingLayerPlugin;
use crate::direction_layer::DirectionLayerPlugin;
use crate::foot_ik_apply::FootIkApplyPlugin;
use crate::gait_layer::GaitLayerPlugin;
use crate::generated_ab_als_provider::GeneratedAbAlsProviderPlugin;
use crate::generated_transition_notify::GeneratedTransitionNotifyPlugin;
use crate::grounded_layer::GroundedLayerPlugin;
use crate::head_layer::HeadLayerPlugin;
use crate::layering_layer::LayeringLayerPlugin;
use crate::locomotion_layer::LocomotionLayerPlugin;
use crate::locomotion_lifecycle::LocomotionLifecyclePlugin;
use crate::mantle_drive::AlsMantleDrivePlugin;
use crate::pa_als::PaAlsPhysicsPlugin;
use crate::ragdoll_link::AlsRagdollLinkPlugin;
use crate::ragdolling_layer::RagdollingLayerPlugin;
use crate::roll_drive::AlsRollDrivePlugin;
use crate::source_physics_timing::SourcePhysicsTimingPlugin;
use crate::stance_layer::StanceLayerPlugin;
use crate::standing_layer::StandingLayerPlugin;
use crate::stop_link::AlsStopLinkPlugin;
use crate::turn_layer::TurnLayerPlugin;
use crate::view_layer::ViewLayerPlugin;

pub struct AlsCharacterRuntimePlugin;

impl Plugin for AlsCharacterRuntimePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(GameplayTagsPlugin::with_data_path(
            "assets/als_gameplay_tags.json",
        ))
        .add_plugins(AlsActualMotionPlugin)
        .add_plugins(SourcePhysicsTimingPlugin)
        .add_plugins((
            als_core::foot_ik::AlsFootProbePlugin,
            ControlRigApplyPlugin,
            FootIkApplyPlugin,
            GaitLayerPlugin,
            DirectionLayerPlugin,
            StandingLayerPlugin,
            CrouchingLayerPlugin,
            GroundedLayerPlugin,
            AlsAnimationEventPlugin,
            AirJumpLayerPlugin,
            StanceLayerPlugin,
            ViewLayerPlugin,
            TurnLayerPlugin,
            AlsRollDrivePlugin,
            AlsStopLinkPlugin,
        ))
        .add_plugins((
            LocomotionLayerPlugin,
            LocomotionLifecyclePlugin,
            LayeringLayerPlugin,
            HeadLayerPlugin,
            RagdollingLayerPlugin,
            AlsRootGraphPlugin,
            GeneratedAbAlsProviderPlugin,
            GeneratedTransitionNotifyPlugin,
            AlsMantleDrivePlugin,
            PaAlsPhysicsPlugin,
            AlsRagdollLinkPlugin,
            crate::get_up::AlsGetUpPlugin,
        ))
        .add_systems(
            Update,
            (
                ensure_start_stop_state,
                start_stop_detector_system.after(ensure_start_stop_state),
            ),
        )
        .configure_sets(PostUpdate, (
            crate::actual_motion::AlsActualMotionSet,
            crate::gait_layer::AlsGaitLayerSet,
            crate::standing_layer::AlsStandingLayerSet,
            crate::crouching_layer::AlsCrouchingLayerSet,
            crate::grounded_layer::AlsGroundedLayerSet,
            crate::ab_als_root::AlsRootGraphSet,
            crate::generated_ab_als_provider::GeneratedAbAlsProviderSet,
            crate::locomotion_layer::AlsLocomotionLayerSet,
            crate::layering_layer::AlsLayeringLayerSet,
            crate::head_layer::AlsHeadLayerSet,
            crate::ragdolling_layer::AlsRagdollingLayerSet,
            crate::foot_ik_apply::FootIkApplySet::UpdateInputs,
        ).in_set(als_ext::post_physics_animation::PostPhysicsAnimationSet::Inputs))
        .add_systems(PostUpdate, mark_character_animation_phase
            .before(bevy_animation_graph::core::plugin::AnimationGraphSet::PrePhysics));
    }
}

fn mark_character_animation_phase(
    mut commands: Commands,
    avatars: Query<&bevy_animation_graph::core::animated_scene::AnimatedSceneInstance,
        (With<crate::character_assembly::AlsCharacterAvatar>, Added<bevy_animation_graph::core::animated_scene::AnimatedSceneInstance>)>,
) {
    for instance in &avatars {
        commands.entity(instance.player_entity()).insert(als_ext::post_physics_animation::PostPhysicsAnimation);
    }
}

fn ensure_start_stop_state(
    mut commands: Commands,
    characters: Query<Entity, (With<AlsCharacter>, Without<StartStopState>)>,
) {
    for character in &characters {
        commands.entity(character).insert(StartStopState::default());
    }
}

#[cfg(test)]
mod tests {
    use bevy_gameplay_tag::{GameplayTag, GameplayTagsSettings};

    #[test]
    fn source_gameplay_tag_table_accepts_als_gait() {
        assert!(
            GameplayTagsSettings::parse_tag_table(include_str!("../assets/als_gameplay_tags.json"))
                .is_ok()
        );
        let tag = GameplayTag::try_new("Als.Gait.Walking").unwrap();
        assert!(tag.is_valid());
        assert!(tag.matches_tag_exact(&GameplayTag::new("Als.Gait.Walking")));
    }
}
