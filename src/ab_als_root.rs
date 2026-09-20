use als_core::character::AlsCharacter;
use als_core::mantle::AlsMantleState;
use als_core::roll::AlsRollState;
use als_core::state::{Gait, LocomotionState, OverlayState, RotationMode, Stance};
use als_ext::set_gameplay_tag_input;
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

const OVERLAY_LAYER_BINDING: &str = "/ALS/ALS/Character/ALI_Overlay.ALI_Overlay_C:Overlay";

impl Plugin for AlsRootGraphPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<crate::overlay_object::OverlayLibraryPlugin>() { app.add_plugins(crate::overlay_object::OverlayLibraryPlugin); }
        app.add_plugins(crate::animation_context_projection::AlsRootAnimationContextPlugin)
            .configure_sets(
                PostUpdate,
                AlsRootGraphSet
                    .after(AlsGaitLayerSet)
                    .after(AlsStandingLayerSet)
                    .before(AlsLocomotionLayerSet)
                    .before(als_ext::post_physics_animation::PostPhysicsAnimationSet::Evaluate),
            )
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

pub(crate) fn gait_tag(gait: Gait) -> GameplayTag {
    gait.tag()
}

pub(crate) fn locomotion_mode_tag(locomotion: LocomotionState) -> GameplayTag {
    GameplayTag::new(match locomotion {
        LocomotionState::Grounded => "Als.LocomotionMode.Grounded",
        LocomotionState::Air => "Als.LocomotionMode.InAir",
        LocomotionState::Custom | LocomotionState::None => "",
    })
}

pub(crate) fn stance_tag(stance: Stance) -> GameplayTag {
    stance.tag()
}

pub(crate) fn locomotion_action_tag(
    notify_action: Option<&AlsNotifyLocomotionAction>,
    mantle: Option<&AlsMantleState>,
    roll: Option<&AlsRollState>,
) -> GameplayTag {
    GameplayTag::new(match notify_action {
        Some(action) => action.as_str(),
        None if mantle.is_some_and(|state| state.active) => "Als.LocomotionAction.Mantling",
        None if roll.is_some_and(|state| state.active) => "Als.LocomotionAction.Rolling",
        None => "",
    })
}

pub(crate) fn rotation_mode_tag(rotation_mode: RotationMode) -> GameplayTag {
    rotation_mode.tag()
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
    library_handle: Res<crate::overlay_object::OverlayLibraryHandle>,
    libraries: Res<Assets<crate::overlay_object::OverlayLibrary>>,
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
                libraries.get(&library_handle.0).and_then(|library| library.0.get(overlay))
                    .filter(|definition| assets.is_loaded_with_dependencies(definition.graph_handle.id())).map(|definition| als_ext::source_gameplay_tag_input::LinkedGraphInput {
                    graph: definition.graph_handle.clone(), blend_in: definition.blend_in,
                    blend_out: definition.blend_out,
                }),
            );
            set_gameplay_tag_input(instance.player_entity(), "gait", gait_tag(*gait));
            set_gameplay_tag_input(instance.player_entity(), "overlay_mode", overlay.tag());
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
            for (key, value) in [
                ("als_animation_delta_seconds", time.delta_secs()),
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
        crate::overlay_object::OverlayObjectPlugin.build(&mut app);
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
            let all_ready = server.is_loaded_with_dependencies(app.world().resource::<crate::overlay_object::OverlayLibraryHandle>().0.id());
            if all_ready && let Some(instance) = app.world().get::<AnimatedSceneInstance>(avatar)
                && app.world().get::<AnimationGraphPlayer>(instance.player_entity()).unwrap().get_default_output_pose().is_some() {
                break instance.player_entity();
            }
            assert!(std::time::Instant::now() < deadline, "Generated overlay dependencies did not become ready");
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        let library_handle = app.world().resource::<crate::overlay_object::OverlayLibraryHandle>().0.clone();
        let custom = OverlayState::new("Als.OverlayMode.CustomRifle").unwrap();
        {
            let mut libraries = app.world_mut().resource_mut::<Assets<crate::overlay_object::OverlayLibrary>>();
            let mut library = libraries.get_mut(&library_handle).unwrap();
            let rifle = library.0[&OverlayState::Rifle].clone();
            library.0.insert(custom, rifle);
        }
        let mut parent_instance = None;
        let mut frame = 0;
        let tags = crate::overlay_mode_menu::tests::tag_manager(&[custom.as_str()]);
        let modes = crate::overlay_mode_menu::menu_display_order(&tags);
        assert!(modes.contains(&custom));
        app.insert_resource(tags);
        let mut custom_visits = 0;
        for overlay in modes.into_iter().chain([custom, custom, OverlayState::Rifle, OverlayState::Bow, OverlayState::Default]) {
            if overlay == custom {
                custom_visits += 1;
                if custom_visits == 3 {
                    let mut libraries = app.world_mut().resource_mut::<Assets<crate::overlay_object::OverlayLibrary>>();
                    let mut library = libraries.get_mut(&library_handle).unwrap();
                    let torch = library.0[&OverlayState::Torch].clone();
                    library.0.insert(custom, torch);
                }
            }
            app.world_mut().get_mut::<als_core::transition::AlsStateRequest>(character).unwrap().overlay_state = Some(overlay);
            app.update();
            app.update();
            assert_eq!(*app.world().get::<OverlayState>(character).unwrap(), overlay);
            assert_eq!(app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity(), player_entity);
            let player = app.world().get::<AnimationGraphPlayer>(player_entity).unwrap();
            assert!(player.get_error().is_none(), "{overlay:?}: {:?}", player.get_error());
            let library = app.world().resource::<Assets<crate::overlay_object::OverlayLibrary>>().get(&library_handle).unwrap();
            let expected = &library.0[&overlay].graph;
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
            if overlay == custom {
                let state = app.world().get::<crate::overlay_object::AlsOverlayObjectState>(character).unwrap();
                let object = app.world().get::<crate::overlay_object::AlsOverlayObject>(state.prop.unwrap()).unwrap();
                assert_eq!(object.overlay, custom);
                assert_eq!(&object.definition, library.0[&custom].object.as_ref().unwrap());
            }
        }
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
        let action = AlsNotifyLocomotionAction::new("Als.LocomotionAction.Custom").unwrap();
        assert_eq!(locomotion_action_tag(Some(&action), None, None), action.tag());
        assert!(!locomotion_action_tag(Some(&AlsNotifyLocomotionAction::None), None, None).is_valid());
        assert_eq!(gait_tag(Gait::new("Als.Gait.Custom").unwrap()).as_str(), "Als.Gait.Custom");
        assert_eq!(stance_tag(Stance::new("Als.Stance.Custom").unwrap()).as_str(), "Als.Stance.Custom");
        assert_eq!(rotation_mode_tag(RotationMode::new("Als.RotationMode.Custom").unwrap()).as_str(), "Als.RotationMode.Custom");
    }

}
