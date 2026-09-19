use std::collections::HashMap;

use als_core::character::AlsCharacter;
use als_core::mantle::AlsMantleState;
use als_core::roll::AlsRollState;
use als_core::state::{Gait, LocomotionState, OverlayState, RotationMode, Stance};
use als_ext::{SyncPhaseInput, SyncPhaseState, advance_sync_phase, set_gameplay_tag_input};
use bevy::ecs::hierarchy::ChildOf;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use bevy_animation_graph::core::pose::Pose;
use bevy_gameplay_tag::GameplayTag;

use crate::animation_events::AlsNotifyLocomotionAction;
use crate::character_assembly::AlsCharacterAvatar;
use crate::gait_layer::AlsGaitLayerSet;
use crate::locomotion_layer::AlsLocomotionLayerSet;
use crate::standing_layer::AlsStandingLayerSet;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsRootGraphSet;

pub struct AlsRootGraphPlugin;

const SECONDARY_MOTION_DURATION: f32 = 2.666667;
const OVERLAY_LAYER_BINDING: &str = "/ALS/ALS/Character/ALI_Overlay.ALI_Overlay_C:Overlay";

#[derive(Resource, Default)]
struct SecondaryMotionPhases {
    states: HashMap<Entity, SyncPhaseState>,
}

impl Plugin for AlsRootGraphPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(crate::animation_context_projection::AlsRootAnimationContextPlugin)
            .configure_sets(
                PostUpdate,
                AlsRootGraphSet
                    .after(AlsGaitLayerSet)
                    .after(AlsStandingLayerSet)
                    .before(AlsLocomotionLayerSet)
                    .before(als_ext::post_physics_animation::PostPhysicsAnimationSet::Evaluate),
            )
            .init_resource::<SecondaryMotionPhases>()
            .add_systems(PostUpdate, root_graph_writer.in_set(AlsRootGraphSet))
            .add_systems(
                PostUpdate,
                root_pose_invariant.after(AnimationGraphSet::Final),
            );
    }
}

fn root_pose_is_valid(pose: &Pose) -> bool {
    pose.bones.iter().all(|bone| {
        bone.translation.is_none_or(Vec3::is_finite)
            && bone
                .rotation
                .is_none_or(|rotation| rotation.is_finite() && rotation.length_squared() > 1e-8)
            && bone.scale.is_none_or(Vec3::is_finite)
            && bone
                .weights
                .as_ref()
                .is_none_or(|weights| weights.iter().all(|weight| weight.is_finite()))
    })
}

fn root_pose_invariant(
    avatars: Query<(&ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    characters: Query<(), With<AlsCharacter>>,
    players: Query<&AnimationGraphPlayer>,
) {
    for (parent, instance) in &avatars {
        if !characters.contains(parent.parent()) {
            continue;
        }
        let Ok(player) = players.get(instance.player_entity()) else {
            continue;
        };
        let Some(DataValue::Pose(pose)) = player.get_outputs().get("pose") else {
            continue;
        };
        if !root_pose_is_valid(pose) {
            error!(
                "[AB-ALS-POSE] invalid root pose entity={:?} bones={}",
                parent.parent(),
                pose.bones.len()
            );
        }
    }
}

pub fn overlay_layer_index(overlay: OverlayState) -> f32 {
    match overlay {
        OverlayState::Default => 0.0,
        OverlayState::Masculine => 1.0,
        OverlayState::Feminine => 2.0,
        OverlayState::Injured => 3.0,
        OverlayState::HandsTied => 4.0,
        OverlayState::Rifle => 5.0,
        OverlayState::PistolOneHanded => 6.0,
        OverlayState::PistolTwoHanded => 7.0,
        OverlayState::Bow => 8.0,
        OverlayState::Torch => 9.0,
        OverlayState::Binoculars => 10.0,
        OverlayState::Box => 11.0,
        OverlayState::Barrel => 12.0,
    }
}

pub(crate) fn gait_tag(gait: Gait) -> GameplayTag {
    GameplayTag::new(match gait {
        Gait::Walking => "Als.Gait.Walking",
        Gait::Running => "Als.Gait.Running",
        Gait::Sprinting => "Als.Gait.Sprinting",
    })
}

pub(crate) fn locomotion_mode_tag(locomotion: LocomotionState) -> GameplayTag {
    GameplayTag::new(match locomotion {
        LocomotionState::Grounded => "Als.LocomotionMode.Grounded",
        LocomotionState::Air => "Als.LocomotionMode.InAir",
        LocomotionState::Custom | LocomotionState::None => "",
    })
}

pub(crate) fn stance_tag(stance: Stance) -> GameplayTag {
    GameplayTag::new(match stance {
        Stance::Standing => "Als.Stance.Standing",
        Stance::Crouching => "Als.Stance.Crouching",
    })
}

fn locomotion_action_index(
    notify_action: Option<&AlsNotifyLocomotionAction>,
    mantle: Option<&AlsMantleState>,
    roll: Option<&AlsRollState>,
) -> f32 {
    match notify_action {
        Some(AlsNotifyLocomotionAction::Mantling) => 1.0,
        Some(AlsNotifyLocomotionAction::GettingUp) => 2.0,
        Some(AlsNotifyLocomotionAction::Rolling) => 3.0,
        Some(AlsNotifyLocomotionAction::Ragdolling) => 4.0,
        Some(AlsNotifyLocomotionAction::None) => 0.0,
        None if mantle.is_some_and(|state| state.active) => 1.0,
        None if roll.is_some_and(|state| state.active) => 3.0,
        None => 0.0,
    }
}

pub(crate) fn locomotion_action_tag(
    notify_action: Option<&AlsNotifyLocomotionAction>,
    mantle: Option<&AlsMantleState>,
    roll: Option<&AlsRollState>,
) -> GameplayTag {
    GameplayTag::new(match notify_action {
        Some(AlsNotifyLocomotionAction::Mantling) => "Als.LocomotionAction.Mantling",
        Some(AlsNotifyLocomotionAction::GettingUp) => "Als.LocomotionAction.GettingUp",
        Some(AlsNotifyLocomotionAction::Rolling) => "Als.LocomotionAction.Rolling",
        Some(AlsNotifyLocomotionAction::Ragdolling) => "Als.LocomotionAction.Ragdolling",
        Some(AlsNotifyLocomotionAction::None) => "",
        None if mantle.is_some_and(|state| state.active) => "Als.LocomotionAction.Mantling",
        None if roll.is_some_and(|state| state.active) => "Als.LocomotionAction.Rolling",
        None => "",
    })
}

pub(crate) fn rotation_mode_tag(rotation_mode: RotationMode) -> GameplayTag {
    GameplayTag::new(match rotation_mode {
        RotationMode::VelocityDirection => "Als.RotationMode.VelocityDirection",
        RotationMode::ViewDirection => "Als.RotationMode.ViewDirection",
        RotationMode::Aiming => "Als.RotationMode.Aiming",
    })
}

fn set_missing_f32(player: &mut AnimationGraphPlayer, key: &str) {
    if player.get_global_input_data(key).is_none() {
        player.set_global_input_data(key, 0.0_f32);
    }
}

#[allow(clippy::type_complexity)]
fn root_graph_writer(
    time: Res<Time>,
    assets: Res<AssetServer>,
    mut overlay_graphs: Local<HashMap<OverlayState, Handle<bevy_animation_graph::core::animation_graph::AnimationGraph>>>,
    mut secondary_motion: ResMut<SecondaryMotionPhases>,
    characters: Query<
        (
            Entity,
            &OverlayState,
            &Stance,
            &Gait,
            &LocomotionState,
            &RotationMode,
            Option<&AlsNotifyLocomotionAction>,
            Option<&AlsMantleState>,
            Option<&AlsRollState>,
        ),
        With<AlsCharacter>,
    >,
    avatars: Query<(&ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    if overlay_graphs.is_empty() {
        overlay_graphs.extend(crate::overlay_object::OVERLAYS.iter().map(|(overlay, definition)| (*overlay, assets.load(definition.graph.clone()))));
    }
    for (
        entity,
        overlay,
        stance,
        gait,
        locomotion,
        rotation_mode,
        notify_action,
        mantle,
        roll,
    ) in &characters
    {
        for (parent, instance) in &avatars {
            if parent.parent() != entity {
                continue;
            }
            let Ok(mut player) = players.get_mut(instance.player_entity()) else {
                continue;
            };
            als_ext::source_gameplay_tag_input::set_linked_graph_input(
                instance.player_entity(), OVERLAY_LAYER_BINDING,
                overlay_graphs.get(overlay).filter(|graph| assets.is_loaded_with_dependencies(graph.id())).cloned(),
            );
            set_gameplay_tag_input(instance.player_entity(), "gait", gait_tag(*gait));
            set_gameplay_tag_input(
                instance.player_entity(),
                "locomotion_mode",
                locomotion_mode_tag(*locomotion),
            );
            set_gameplay_tag_input(instance.player_entity(), "stance", stance_tag(*stance));
            set_gameplay_tag_input(
                instance.player_entity(),
                "locomotion_action",
                locomotion_action_tag(notify_action, mantle, roll),
            );
            set_gameplay_tag_input(
                instance.player_entity(),
                "rotation_mode",
                rotation_mode_tag(*rotation_mode),
            );
            let secondary_motion_phase = advance_sync_phase(
                secondary_motion
                    .states
                    .entry(instance.player_entity())
                    .or_default(),
                SyncPhaseInput {
                    active: true,
                    reset: false,
                    rate: 1.0,
                    duration: SECONDARY_MOTION_DURATION,
                    start_phase: 0.0,
                    looped: true,
                    delta_seconds: time.delta_secs(),
                },
            );
            for (key, value) in [
                ("als_root_overlay_index", overlay_layer_index(*overlay)),
                ("als_animation_delta_seconds", time.delta_secs()),
                ("als_secondary_motion_phase", secondary_motion_phase),
                (
                    "als_overlay_locomotion_action_index",
                    locomotion_action_index(notify_action, mantle, roll),
                ),
            ] {
                player.set_global_input_data(key, value);
            }
            for key in [
                "als_root_transition_slot_weight",
                "als_root_post_locomotion_slot_weight",
                "als_root_transition_slot_layer_weight",
                "als_root_post_locomotion_slot_layer_weight",
            ] {
                set_missing_f32(&mut player, key);
            }
            player.set_global_input_data("als_root_defaults_enabled", true);
            let skeleton = player.skeleton().clone();
            let elapsed = player.elapsed();
            player.set_global_input_data(
                "als_root_reference_pose",
                DataValue::Pose(Pose {
                    skeleton,
                    timestamp: elapsed,
                    ..default()
                }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_overlay_map_selects_every_generated_class_without_restarting_the_parent() {
        use bevy::ecs::system::RunSystemOnce;
        use crate::character_assembly::{AlsCharacterAssembly, AlsCharacterBlueprint, spawn_als_character};
        let (mut app, _, _, _) = crate::camera_component::tests::loaded_camera_scene();
        let character = app.world_mut().run_system_once(|mut commands: Commands, assets: Res<AssetServer>,
            mut params: ResMut<Assets<als_core::params::AlsMovementParams>>,
            mut configs: ResMut<Assets<als_core::control_scheme::AlsControlSchemeConfig>>| {
            spawn_als_character(&mut commands, &assets, &mut params, &mut configs,
                AlsCharacterBlueprint { spawn_input: false, ..AlsCharacterBlueprint::new(Transform::from_xyz(0.0, 3.0, 0.0)) })
        }).unwrap();
        app.world_mut().entity_mut(character).insert((avian3d::prelude::RigidBody::Kinematic, bevy_tnua::TnuaToggle::Disabled));
        let avatar = app.world().get::<AlsCharacterAssembly>(character).unwrap().avatar;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(40);
        let player_entity = loop {
            app.update();
            let server = app.world().resource::<AssetServer>();
            let all_ready = crate::overlay_object::OVERLAYS.values().all(|definition| server.get_handle::<bevy_animation_graph::core::animation_graph::AnimationGraph>(&definition.graph)
                .is_some_and(|handle| server.is_loaded_with_dependencies(handle.id())));
            if all_ready && let Some(instance) = app.world().get::<AnimatedSceneInstance>(avatar)
                && app.world().get::<AnimationGraphPlayer>(instance.player_entity()).unwrap().get_default_output_pose().is_some() {
                break instance.player_entity();
            }
            assert!(std::time::Instant::now() < deadline, "Generated overlay dependencies did not become ready");
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        let mut parent_instance = None;
        let mut frame = 0;
        for overlay in crate::overlay_mode_menu::OVERLAY_TAG_CHILD_NODES.into_iter().chain([OverlayState::Rifle, OverlayState::Bow, OverlayState::Default]) {
            *app.world_mut().get_mut::<OverlayState>(character).unwrap() = overlay;
            app.update();
            app.update();
            assert_eq!(app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity(), player_entity);
            let player = app.world().get::<AnimationGraphPlayer>(player_entity).unwrap();
            assert!(player.get_error().is_none(), "{overlay:?}: {:?}", player.get_error());
            let expected = &crate::overlay_object::OVERLAYS[&overlay].graph;
            let arena = player.get_context_arena().unwrap();
            assert!(arena.frame_index() > frame);
            frame = arena.frame_index();
            let instance = arena.get_toplevel().node_states.get::<als_ext::source_instance::SourceInstanceProperties>(
                bevy_animation_graph::core::animation_graph::NodeId::default(), bevy_animation_graph::core::context::node_states::StateKey::Default).unwrap().instance_id;
            assert_eq!(*parent_instance.get_or_insert(instance), instance, "Parent animation restarted for {overlay:?}");
            let server = app.world().resource::<AssetServer>();
            let graphs = app.world().resource::<Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>>();
            assert!(arena.iter_context_ids().any(|id| {
                let Some(context) = arena.get_context(id) else { return false; };
                if server.get_path(context.get_graph_id()).is_none_or(|path| path.path() != std::path::Path::new(expected)) { return false; }
                graphs.get(context.get_graph_id()).is_some_and(|graph| graph.nodes.values().any(|node| context.node_caches.is_updated(node.id, bevy_animation_graph::core::context::node_states::StateKey::Default)))
            }), "Requested generated overlay was not evaluated: {overlay:?}");
            let pose = player.get_default_output_pose().unwrap();
            assert!(pose.bones.len() > 50 && root_pose_is_valid(pose));
        }
    }

    #[test]
    fn overlay_indices_follow_the_root_implementation_order() {
        assert_eq!(overlay_layer_index(OverlayState::Default), 0.0);
        assert_eq!(overlay_layer_index(OverlayState::Bow), 8.0);
        assert_eq!(overlay_layer_index(OverlayState::Torch), 9.0);
        assert_eq!(overlay_layer_index(OverlayState::Barrel), 12.0);
    }

    #[test]
    fn gait_tag_uses_the_source_hierarchy() {
        assert_eq!(gait_tag(Gait::Walking).as_str(), "Als.Gait.Walking");
        assert_eq!(gait_tag(Gait::Sprinting).as_str(), "Als.Gait.Sprinting");
    }

    #[test]
    fn locomotion_mode_and_stance_tags_use_the_source_hierarchy() {
        assert_eq!(
            locomotion_mode_tag(LocomotionState::Grounded).as_str(),
            "Als.LocomotionMode.Grounded"
        );
        assert_eq!(
            locomotion_mode_tag(LocomotionState::Air).as_str(),
            "Als.LocomotionMode.InAir"
        );
        assert!(!locomotion_mode_tag(LocomotionState::None).is_valid());
        assert!(!locomotion_mode_tag(LocomotionState::Custom).is_valid());
        assert_eq!(stance_tag(Stance::Standing).as_str(), "Als.Stance.Standing");
        assert_eq!(
            stance_tag(Stance::Crouching).as_str(),
            "Als.Stance.Crouching"
        );
    }

    #[test]
    fn action_and_rotation_tags_use_the_source_hierarchy() {
        assert_eq!(
            locomotion_action_tag(
                Some(&AlsNotifyLocomotionAction::Rolling),
                None,
                None,
            )
            .as_str(),
            "Als.LocomotionAction.Rolling"
        );
        assert_eq!(
            locomotion_action_tag(Some(&AlsNotifyLocomotionAction::Ragdolling), None, None).as_str(),
            "Als.LocomotionAction.Ragdolling"
        );
        assert_eq!(
            rotation_mode_tag(RotationMode::Aiming).as_str(),
            "Als.RotationMode.Aiming"
        );
    }

    #[test]
    fn notify_action_overrides_legacy_action_inference() {
        assert_eq!(
            locomotion_action_index(Some(&AlsNotifyLocomotionAction::Mantling), None, None),
            1.0
        );
        assert_eq!(
            locomotion_action_index(Some(&AlsNotifyLocomotionAction::GettingUp), None, None),
            2.0
        );
        assert_eq!(
            locomotion_action_index(Some(&AlsNotifyLocomotionAction::Rolling), None, None),
            3.0
        );
        assert_eq!(
            locomotion_action_index(Some(&AlsNotifyLocomotionAction::None), None, None),
            0.0
        );
    }

    #[test]
    fn secondary_motion_phase_uses_the_source_idle_loop_duration() {
        let mut state = SyncPhaseState::default();
        let input = |delta_seconds| SyncPhaseInput {
            active: true,
            reset: false,
            rate: 1.0,
            duration: SECONDARY_MOTION_DURATION,
            start_phase: 0.0,
            looped: true,
            delta_seconds,
        };
        assert!(advance_sync_phase(&mut state, input(0.0)).abs() < 1e-6);
        assert!(
            (advance_sync_phase(&mut state, input(SECONDARY_MOTION_DURATION * 0.25)) - 0.25).abs()
                < 1e-6
        );
        assert!(
            (advance_sync_phase(&mut state, input(SECONDARY_MOTION_DURATION * 0.75))).abs() < 1e-6
        );
    }

}
