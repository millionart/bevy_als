use crate::als::character::AlsCharacter;
use crate::als::start_stop::{StartStopState, start_stop_detector_system};
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
use crate::ab_als_provider::AbAlsProviderPlugin;
use crate::transition_notify::TransitionNotifyPlugin;
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
            "assets/ALS/Data/als_gameplay_tags.json",
        ))
        .add_plugins(AlsActualMotionPlugin)
        .add_plugins(crate::camera_component::AlsSourceCameraPlugin)
        .add_plugins(crate::footprint::FootprintPlugin)
        .add_plugins(SourcePhysicsTimingPlugin)
        .add_plugins((
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
            AbAlsProviderPlugin,
            TransitionNotifyPlugin,
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
            crate::ab_als_provider::AbAlsProviderSet,
            crate::locomotion_layer::AlsLocomotionLayerSet,
            crate::layering_layer::AlsLayeringLayerSet,
            crate::head_layer::AlsHeadLayerSet,
            crate::ragdolling_layer::AlsRagdollingLayerSet,
            crate::foot_ik_apply::FootIkApplySet::UpdateInputs,
        ).in_set(crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Inputs))
        .configure_sets(PostUpdate, crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Inputs
            .after(crate::als::motion::AlsActualVelocitySet))
        .configure_sets(PostUpdate, crate::als_camera::AlsCameraSet
            .after(crate::control_rig_apply::AlsControlRigPropagationSet))
        .add_systems(PostUpdate, mark_character_animation_phase
            .before(bevy_animation_graph::core::plugin::AnimationGraphSet::PrePhysics));
    }
}

fn mark_character_animation_phase(
    mut commands: Commands,
    avatars: Query<(Entity, &crate::character_assembly::AlsCharacterAvatar,
        &bevy_animation_graph::core::animated_scene::AnimatedSceneInstance)>,
    assemblies: Query<&crate::character_assembly::AlsCharacterAssembly, With<AlsCharacter>>,
    players: Query<Has<crate::extensions::post_physics_animation::PostPhysicsAnimation>,
        With<bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer>>,
    mut previous: Local<std::collections::HashSet<Entity>>,
) {
    let current = avatars.iter().filter(|(entity, avatar, _)|
        assemblies.get(avatar.character).is_ok_and(|assembly| assembly.avatar == *entity))
        .map(|(_, _, instance)| instance.player_entity()).filter(|player| players.get(*player).is_ok())
        .collect::<std::collections::HashSet<_>>();
    for player in previous.difference(&current) {
        if let Ok(mut entity) = commands.get_entity(*player) {
            entity.remove::<crate::extensions::post_physics_animation::PostPhysicsAnimation>();
        }
    }
    for player in &current {
        if players.get(*player).is_ok_and(|marked| !marked) {
            commands.entity(*player).insert(crate::extensions::post_physics_animation::PostPhysicsAnimation);
        }
    }
    *previous = current;
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
    use super::*;
    use crate::als::foot_ik::{AlsFootBones, AlsFootLockState};
    use bevy_animation_graph::core::animation_clip::EntityPath;
    use bevy_gameplay_tag::{GameplayTag, GameplayTagsSettings};

    #[test]
    fn animation_phase_follows_rebuilt_scene_on_retained_avatar() {
        use bevy::world_serialization::{WorldAsset, WorldInstanceReady, WorldInstanceSpawner};
        use bevy_animation_graph::core::{animated_scene::{AnimatedSceneHandle, AnimatedSceneInstance}, animation_graph_player::AnimationGraphPlayer};
        use crate::{character_assembly::{AlsCharacterAssembly, AlsCharacterAvatar}, extensions::post_physics_animation::PostPhysicsAnimation};

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy_animation_graph::AnimationGraphPlugin::default()));
        app.add_systems(Update, mark_character_animation_phase);
        let character = app.world_mut().spawn(AlsCharacter).id();
        let avatar = app.world_mut().spawn((AlsCharacterAvatar { character }, AnimatedSceneHandle::default())).id();
        app.world_mut().entity_mut(character).insert(AlsCharacterAssembly { avatar, camera: None });
        let ready = |app: &mut App| {
            app.world_mut().trigger(WorldInstanceReady {
                entity: avatar,
                instance_id: WorldInstanceSpawner::default().spawn(Handle::<WorldAsset>::default()),
            });
            app.world_mut().flush();
            app.world_mut().run_schedule(Update);
        };
        let old_player = app.world_mut().spawn((AnimationGraphPlayer::new(default()), ChildOf(avatar))).id();
        ready(&mut app);
        assert!(app.world().get::<PostPhysicsAnimation>(old_player).is_some());
        app.world_mut().entity_mut(old_player).remove::<ChildOf>();
        let player = app.world_mut().spawn((AnimationGraphPlayer::new(default()), ChildOf(avatar))).id();
        ready(&mut app);
        assert_eq!(app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity(), player);
        assert!(app.world().get::<PostPhysicsAnimation>(player).is_some());
        assert!(app.world().get::<PostPhysicsAnimation>(old_player).is_none());
        app.world_mut().entity_mut(avatar).remove::<AlsCharacterAvatar>();
        app.world_mut().run_schedule(Update);
        assert!(app.world().get::<PostPhysicsAnimation>(player).is_none());
        let unrelated = app.world_mut().spawn((AnimationGraphPlayer::new(default()), PostPhysicsAnimation)).id();
        app.world_mut().entity_mut(avatar).insert(AlsCharacterAvatar { character });
        app.world_mut().run_schedule(Update);
        assert!(app.world().get::<PostPhysicsAnimation>(player).is_some());
        assert!(app.world().get::<PostPhysicsAnimation>(unrelated).is_some());
        app.world_mut().entity_mut(character).remove::<AlsCharacter>();
        app.world_mut().run_schedule(Update);
        assert!(app.world().get::<PostPhysicsAnimation>(player).is_none());
        app.world_mut().entity_mut(character).insert(AlsCharacter);
        app.world_mut().run_schedule(Update);
        assert!(app.world().get::<PostPhysicsAnimation>(player).is_some());
        app.world_mut().entity_mut(player).remove::<PostPhysicsAnimation>();
        app.world_mut().run_schedule(Update);
        assert!(app.world().get::<PostPhysicsAnimation>(player).is_some());
        app.world_mut().entity_mut(avatar).despawn();
        app.world_mut().run_schedule(Update);
        assert!(app.world().get::<PostPhysicsAnimation>(old_player).is_none());
        assert!(app.world().get::<PostPhysicsAnimation>(unrelated).is_some());
    }

    #[test]
    fn foot_bones_follow_selected_avatar_when_old_bones_remain_alive() {
        fn spawn_feet(world: &mut World, avatar: Entity) -> [Entity; 6] {
            ["thigh_l", "calf_l", "foot_l", "thigh_r", "calf_r", "foot_r"].map(|name| {
                world
                    .spawn((
                        Name::new(name),
                        EntityPath::from(vec![name.to_owned()]).id().animation_target_id(),
                        ChildOf(avatar),
                    ))
                    .id()
            })
        }

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_systems(Update, crate::foot_ik_apply::refresh_current_foot_bones);
        let character = app.world_mut().spawn(AlsCharacter).id();
        let old_avatar = app.world_mut().spawn(ChildOf(character)).id();
        let old_feet = spawn_feet(app.world_mut(), old_avatar);
        app.world_mut().entity_mut(character).insert(
            crate::character_assembly::AlsCharacterAssembly {
                avatar: old_avatar,
                camera: None,
            },
        );
        app.update();
        assert_eq!(app.world().get::<AlsFootBones>(character).unwrap().left, old_feet[2]);
        app.world_mut().entity_mut(character).insert(AlsFootLockState {
            valid: true,
            ..default()
        });

        let new_avatar = app.world_mut().spawn(ChildOf(character)).id();
        let new_feet = spawn_feet(app.world_mut(), new_avatar);
        app.world_mut()
            .get_mut::<crate::character_assembly::AlsCharacterAssembly>(character)
            .unwrap()
            .avatar = new_avatar;
        app.update();
        assert!(app.world().entities().contains(old_feet[2]));
        assert_eq!(app.world().get::<AlsFootBones>(character).unwrap().left, new_feet[2]);
        assert!(!app.world().get::<AlsFootLockState>(character).unwrap().valid);

        for bone in new_feet {
            app.world_mut().entity_mut(bone).remove::<ChildOf>();
        }
        let replacement_feet = spawn_feet(app.world_mut(), new_avatar);
        app.update();
        assert!(app.world().entities().contains(new_feet[2]));
        assert_eq!(
            app.world().get::<AlsFootBones>(character).unwrap().left,
            replacement_feet[2]
        );

        app.world_mut().entity_mut(new_avatar).despawn();
        app.update();
        assert!(app.world().get::<AlsFootBones>(character).is_none());
        let last_avatar = app.world_mut().spawn(ChildOf(character)).id();
        let last_feet = spawn_feet(app.world_mut(), last_avatar);
        app.world_mut()
            .get_mut::<crate::character_assembly::AlsCharacterAssembly>(character)
            .unwrap()
            .avatar = last_avatar;
        app.update();
        assert_eq!(app.world().get::<AlsFootBones>(character).unwrap().left, last_feet[2]);
    }

    #[test]
    fn source_gameplay_tag_table_accepts_als_gait() {
        assert!(
            GameplayTagsSettings::parse_tag_table(include_str!("../../../assets/ALS/Data/als_gameplay_tags.json"))
                .is_ok()
        );
        let tag = GameplayTag::try_new("Als.Gait.Walking").unwrap();
        assert!(tag.is_valid());
        assert!(tag.matches_tag_exact(&GameplayTag::new("Als.Gait.Walking")));
    }
}
