use crate::als::character::AlsCharacter;
use crate::als::start_stop::{StartStopState, start_stop_detector_system};
use bevy::{ecs::entity::EntityHashSet, prelude::*};
use bevy_gameplay_tag::{GameplayTagsManager, GameplayTagsSettings, gameplay_tags_manager::GameplayTagNode};

use crate::ab_als_provider::AbAlsProviderPlugin;
use crate::ab_als_root::AlsRootGraphPlugin;
use crate::actual_motion::AlsActualMotionPlugin;
use crate::air_jump_layer::AirJumpLayerPlugin;
use crate::animation_events::AlsAnimationEventPlugin;
use crate::control_rig_apply::ControlRigApplyPlugin;
use crate::crouching_layer::CrouchingLayerPlugin;
use crate::direction_layer::DirectionLayerPlugin;
use crate::foot_ik_apply::FootIkApplyPlugin;
use crate::gait_layer::GaitLayerPlugin;
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
use crate::transition_notify::TransitionNotifyPlugin;
use crate::turn_layer::TurnLayerPlugin;
use crate::view_layer::ViewLayerPlugin;

pub struct AlsCharacterRuntimePlugin;

fn install_gameplay_tags(world: &mut World) {
    let bundled = crate::als_asset_bytes("ALS/Data/als_gameplay_tags.json")
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .expect("Bundled ALS gameplay tags must be UTF-8");
    let settings = world.remove_resource::<GameplayTagsSettings>();
    if settings.is_none() && !world.contains_resource::<GameplayTagsManager>() {
        world.insert_resource(GameplayTagsSettings { json_data: bundled.into(), data_path: None });
        world.init_resource::<GameplayTagsManager>();
        return;
    }

    let mut rows = settings.map_or_else(Vec::new, |settings| {
        let result = match settings.data_path {
            Some(path) => GameplayTagsSettings::load_tag_table_from_path(path),
            None => GameplayTagsSettings::parse_tag_table(&settings.json_data),
        };
        result.unwrap_or_else(|error| {
            error!("Could not load host gameplay tags: {error}");
            Vec::new()
        })
    });
    let mut names: std::collections::HashSet<_> = rows.iter().map(|row| row.tag_name.clone()).collect();
    rows.extend(GameplayTagsSettings::parse_tag_table(bundled)
        .expect("Bundled ALS gameplay tags must be valid")
        .into_iter()
        .filter(|row| names.insert(row.tag_name.clone())));
    if let Some(manager) = world.get_resource::<GameplayTagsManager>() {
        rows.retain(|row| !manager.has_tag_name(&row.tag_name));
        if rows.is_empty() {
            return;
        }
    }
    world.insert_resource(GameplayTagsSettings {
        json_data: serde_json::to_string(&rows).expect("Gameplay tag rows must serialize"),
        data_path: None,
    });

    if !world.contains_resource::<GameplayTagsManager>() {
        world.init_resource::<GameplayTagsManager>();
        return;
    }
    let added = GameplayTagsManager::from_world(world);
    world.resource_scope(|world, mut manager: Mut<GameplayTagsManager>| {
        for (tag, container) in added.tag_map {
            manager.tag_map.entry(tag).or_insert(container);
        }
        merge_tag_subtree(world, added.root, manager.root);
    });
}

fn merge_tag_subtree(world: &mut World, source: Entity, target: Entity) {
    let children = world.get::<Children>(source).map_or_else(Vec::new, |children| children.to_vec());
    for child in children {
        let matching = world.get::<Children>(target).into_iter().flatten().copied().find(|&entity| {
            world.get::<GameplayTagNode>(entity).is_some()
                && world.get::<Name>(entity) == world.get::<Name>(child)
        });
        if let Some(matching) = matching {
            merge_tag_subtree(world, child, matching);
        } else {
            world.entity_mut(child).insert(ChildOf(target));
        }
    }
    world.despawn(source);
}

impl Plugin for AlsCharacterRuntimePlugin {
    fn build(&self, app: &mut App) {
        let prepare = crate::AlsPhysicsSchedule::prepare(app);
        install_gameplay_tags(app.world_mut());
        if crate::AlsPhysicsSchedule::uses_physics_steps(app) {
            app.add_observer(mark_new_animation_scene_phase);
        }
        app.add_plugins(AlsActualMotionPlugin)
        .add_plugins(crate::animation_visibility::AlsAnimationVisibilityPlugin)
        .add_plugins(crate::animation_lod::AlsAnimationLodPlugin)
        .add_plugins(crate::animation_rate::AlsAnimationRatePlugin)
        .add_plugins(crate::camera_component::AlsSourceCameraPlugin)
        .add_plugins(crate::footprint::FootprintPlugin);
        if !crate::AlsPhysicsSchedule::uses_physics_steps(app)
            && !app.is_plugin_added::<SourcePhysicsTimingPlugin>()
        {
            app.add_plugins(SourcePhysicsTimingPlugin);
        }
        app.add_plugins((
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
            prepare,
            (
                ensure_start_stop_state,
                start_stop_detector_system.after(ensure_start_stop_state),
            ).in_set(crate::playable_character::AlsPhysicsPrepareSet),
        )
        .configure_sets(
            PostUpdate,
            (
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
            )
                .in_set(crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Inputs),
        )
        .configure_sets(
            PostUpdate,
            crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Inputs
                .after(crate::als::motion::AlsActualVelocitySet),
        )
        .configure_sets(
            PostUpdate,
            crate::als_camera::AlsCameraSet
                .after(crate::control_rig_apply::AlsControlRigPropagationSet),
        )
        .add_systems(
            PostUpdate,
            mark_character_animation_phase
                .before(bevy_animation_graph::core::plugin::AnimationGraphSet::PrePhysics),
        )
        .add_systems(
            PostUpdate,
            mark_birth_animation_players
                .after(mark_character_animation_phase)
                .before(bevy_animation_graph::core::plugin::AnimationGraphSet::PrePhysics),
        )
        .add_systems(
            PostUpdate,
            release_birth_animation_players.after(
                crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate,
            ),
        );
    }

    fn finish(&self, app: &mut App) {
        if app.world().contains_resource::<GameplayTagsSettings>() {
            install_gameplay_tags(app.world_mut());
        }
    }
}

fn mark_new_animation_scene_phase(
    event: On<Insert<bevy_animation_graph::core::animated_scene::AnimatedSceneInstance>>,
    mut commands: Commands,
    scenes: Query<(
        &bevy_animation_graph::core::animated_scene::AnimatedSceneInstance,
        Option<&crate::character_assembly::AlsCharacterAvatar>,
        Has<crate::camera_component::AlsSourceCameraComponent>,
        Option<&ChildOf>,
    )>,
    assemblies: Query<&crate::character_assembly::AlsCharacterAssembly, With<AlsCharacter>>,
    props: Query<&crate::overlay_object::AlsOverlayObject>,
) {
    use crate::extensions::post_physics_animation::{ManualAnimationPlayer, PostPhysicsAnimation};

    let Ok((scene, avatar, camera, parent)) = scenes.get(event.entity) else {
        return;
    };
    if camera || parent
        .and_then(|parent| props.get(parent.parent()).ok())
        .is_some_and(|prop| prop.content == event.entity)
    {
        commands.entity(scene.player_entity()).try_insert_if_new(ManualAnimationPlayer);
    } else if avatar
        .and_then(|avatar| assemblies.get(avatar.character).ok())
        .is_some_and(|assembly| assembly.avatar == event.entity)
    {
        commands.entity(scene.player_entity()).try_insert_if_new(PostPhysicsAnimation);
    }
}

fn mark_birth_animation_players(
    mut commands: Commands,
    assets: Res<AssetServer>,
    library_handle: Res<crate::overlay_object::OverlayLibraryHandle>,
    libraries: Res<Assets<crate::overlay_object::OverlayLibrary>>,
    avatars: Query<(
        Entity,
        &crate::character_assembly::AlsCharacterAvatar,
        &bevy_animation_graph::core::animated_scene::AnimatedSceneInstance,
    )>,
    characters: Query<
        (
            &crate::character_assembly::AlsCharacterAssembly,
            &crate::als::state::OverlayState,
        ),
        With<crate::als::character::AlsBirthAnimationPending>,
    >,
    players: Query<(
        &bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer,
        Option<&crate::extensions::post_physics_animation::InitialAnimation>,
    )>,
) {
    use bevy_animation_graph::core::animation_graph_player::AnimationSource;
    for (avatar_entity, avatar, instance) in &avatars {
        let Ok((assembly, overlay)) = characters.get(avatar.character) else {
            continue;
        };
        if assembly.avatar != avatar_entity {
            continue;
        }
        let Ok((player, initial)) = players.get(instance.player_entity()) else {
            continue;
        };
        let graph_ready = match player.get_animation_source() {
            AnimationSource::Graph(graph) => assets.is_loaded_with_dependencies(graph.id()),
            _ => false,
        };
        let overlay_ready = libraries.get(&library_handle.0).is_some_and(|library| {
            library.0.get(overlay).is_none_or(|definition| {
                assets.is_loaded_with_dependencies(definition.graph_handle.id())
            })
        });
        let ready = graph_ready
            && assets.is_loaded_with_dependencies(player.skeleton().id())
            && overlay_ready;
        if initial.is_none_or(|initial| initial.ready != ready) {
            commands
                .entity(instance.player_entity())
                .insert(crate::extensions::post_physics_animation::InitialAnimation { ready });
        }
    }
}

fn release_birth_animation_players(
    mut commands: Commands,
    avatars: Query<(
        Entity,
        &crate::character_assembly::AlsCharacterAvatar,
        &bevy_animation_graph::core::animated_scene::AnimatedSceneInstance,
    )>,
    characters: Query<
        (
            &crate::character_assembly::AlsCharacterAssembly,
            Option<&bevy_tnua::TnuaToggle>,
            &bevy_tnua::TnuaSensorsEntities<crate::als::control_scheme::AlsControlScheme>,
        ),
        With<crate::als::character::AlsBirthAnimationPending>,
    >,
    players: Query<(
        &bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer,
        &crate::extensions::post_physics_animation::InitialAnimation,
    )>,
) {
    for (avatar_entity, avatar, instance) in &avatars {
        let Ok((assembly, toggle, sensors)) = characters.get(avatar.character) else {
            continue;
        };
        if assembly.avatar != avatar_entity {
            continue;
        }
        let Ok((player, initial)) = players.get(instance.player_entity()) else {
            continue;
        };
        if !initial.ready
            || player.get_error().is_some()
            || player.get_default_output_pose().is_none()
            || (toggle != Some(&bevy_tnua::TnuaToggle::Disabled) && sensors.ground.is_none())
        {
            continue;
        }
        commands
            .entity(avatar.character)
            .remove::<(
                crate::als::character::AlsBirthAnimationPending,
                avian3d::prelude::RigidBodyDisabled,
            )>()
            .insert(avian3d::dynamics::integrator::VelocityIntegrationData::default());
        if toggle == Some(&bevy_tnua::TnuaToggle::SenseOnly) {
            commands
                .entity(avatar.character)
                .remove::<bevy_tnua::TnuaToggle>();
        }
        commands
            .entity(instance.player_entity())
            .remove::<crate::extensions::post_physics_animation::InitialAnimation>();
    }
}

fn mark_character_animation_phase(
    mut commands: Commands,
    avatars: Query<(
        Entity,
        &crate::character_assembly::AlsCharacterAvatar,
        &bevy_animation_graph::core::animated_scene::AnimatedSceneInstance,
    )>,
    assemblies: Query<&crate::character_assembly::AlsCharacterAssembly, With<AlsCharacter>>,
    players: Query<
        Has<crate::extensions::post_physics_animation::PostPhysicsAnimation>,
        With<bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer>,
    >,
    mut ownership: Local<(EntityHashSet, EntityHashSet)>,
) {
    let (previous, current) = &mut *ownership;
    current.clear();
    for (entity, avatar, instance) in &avatars {
        if assemblies.get(avatar.character).is_ok_and(|assembly| assembly.avatar == entity)
            && let Ok(marked) = players.get(instance.player_entity())
            && current.insert(instance.player_entity())
            && !marked
        {
            commands
                .entity(instance.player_entity())
                .insert(crate::extensions::post_physics_animation::PostPhysicsAnimation);
        }
    }
    for player in previous.difference(current) {
        if let Ok(mut entity) = commands.get_entity(*player) {
            entity.remove::<crate::extensions::post_physics_animation::PostPhysicsAnimation>();
        }
    }
    std::mem::swap(previous, current);
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
    fn birth_pose_and_curves_precede_first_character_physics() {
        use crate::{
            als::{
                character::{AlsBirthAnimationPending, AlsCollisionLayer},
                state::{Gait, LocomotionState},
            },
            animation_context_projection::root_pose_source_node,
            character_assembly::{
                AlsCharacterAssembly, AlsCharacterAvatar, AlsCharacterBlueprint,
                spawn_als_character,
            },
            extensions::{
                SourceCurveName, player_animation_context_state,
                post_physics_animation::{
                    AnimationPoseUpdateGate, InitialAnimation, PostPhysicsAnimationSet,
                },
            },
        };
        use avian3d::prelude::{
            Collider, CollisionLayers, LayerMask, LinearVelocity, RigidBody, RigidBodyDisabled,
        };
        use bevy::ecs::system::RunSystemOnce;
        use bevy_animation_graph::core::{
            animated_scene::{AnimatedSceneHandle, AnimatedSceneInstance},
            animation_graph::AnimationGraph,
            animation_graph_player::{AnimationGraphPlayer, AnimationSource},
        };

        #[derive(Resource, Default)]
        struct Capture {
            initial: Option<(Vec3, LocomotionState, u64, [f32; 6], [f32; 3])>,
            ordinary: Option<(Vec3, LocomotionState, u64)>,
        }
        fn capture(
            mut output: ResMut<Capture>,
            actors: Query<
                (
                    &Transform,
                    &LocomotionState,
                    &Gait,
                    Has<AlsBirthAnimationPending>,
                    Has<RigidBodyDisabled>,
                    &crate::als::character::AlsCmcAdapterRuntime,
                ),
                With<AlsCharacter>,
            >,
            avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
            players: Query<(
                &AnimationGraphPlayer,
                &AnimationPoseUpdateGate,
                Option<&InitialAnimation>,
            )>,
            graphs: Res<Assets<AnimationGraph>>,
        ) {
            for (avatar, instance) in &avatars {
                let Ok((transform, mode, gait, pending, disabled, cmc)) =
                    actors.get(avatar.character)
                else {
                    continue;
                };
                if pending {
                    assert!(disabled);
                    assert!(cmc.grounded_movement_delta.is_none());
                    assert_eq!(*mode, LocomotionState::Grounded);
                    assert_eq!(*gait, Gait::Running);
                    assert!((transform.translation.y - 1.5).abs() < 1e-6);
                }
                let Ok((player, gate, initial)) = players.get(instance.player_entity()) else {
                    continue;
                };
                if player.get_default_output_pose().is_none() {
                    continue;
                }
                let frame = player.get_context_arena().unwrap().frame_index();
                if initial.is_some_and(|initial| initial.ready) && output.initial.is_none() {
                    assert!(
                        gate.pending_update,
                        "Zero-delta birth pose cleared pending before component registration completed"
                    );
                    assert_eq!(
                        player
                            .get_global_input_data("animation.pending_update")
                            .and_then(|value| value.as_bool().ok()),
                        Some(true)
                    );
                    let AnimationSource::Graph(graph) = player.get_animation_source() else {
                        panic!("Birth graph is absent");
                    };
                    let node = root_pose_source_node(graphs.get(graph).unwrap()).unwrap();
                    let context = player_animation_context_state(player, node).unwrap();
                    let curves = [
                        "PoseGrounded",
                        "PoseStanding",
                        "FootLeftIk",
                        "FootRightIk",
                        "FootLeftLock",
                        "FootRightLock",
                    ]
                    .map(|name| {
                        context
                            .curves
                            .get(&SourceCurveName::from(name))
                            .copied()
                            .unwrap_or(0.0)
                    });
                    let deltas = [
                        "animation.delta_seconds",
                        "animation.delta_seconds_without_time_dilation",
                        "als_animation_delta_seconds",
                    ]
                    .map(|name| {
                        player
                            .get_global_input_data(name)
                            .and_then(|value| value.as_f32().ok())
                            .unwrap()
                    });
                    output.initial = Some((transform.translation, *mode, frame, curves, deltas));
                } else if initial.is_none() && output.initial.is_some() && output.ordinary.is_none()
                {
                    assert_eq!(
                        player
                            .get_global_input_data("animation.pending_update")
                            .and_then(|value| value.as_bool().ok()),
                        Some(true)
                    );
                    assert!(
                        !gate.pending_update,
                        "Ordinary update did not clear registration pending"
                    );
                    output.ordinary = Some((transform.translation, *mode, frame));
                }
            }
        }

        let (mut app, _, _, _) = crate::camera_component::tests::loaded_camera_scene();
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f32(1.0 / 30.0),
        ))
        .init_resource::<Capture>()
        .add_systems(
            PostUpdate,
            capture
                .after(PostPhysicsAnimationSet::Evaluate)
                .before(release_birth_animation_players),
        );
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(100.0, 1.0, 100.0),
            Transform::from_xyz(0.0, -0.5, 0.0),
            CollisionLayers::new(
                [AlsCollisionLayer::Default, AlsCollisionLayer::Visibility],
                LayerMask::ALL,
            ),
        ));
        let actor = app
            .world_mut()
            .run_system_once(
                |mut commands: Commands,
                 assets: Res<AssetServer>,
                 mut params: ResMut<Assets<crate::als::params::AlsMovementParams>>,
                 mut configs: ResMut<
                    Assets<crate::als::control_scheme::AlsControlSchemeConfig>,
                >| {
                    spawn_als_character(
                        &mut commands,
                        &assets,
                        &mut params,
                        &mut configs,
                        AlsCharacterBlueprint {
                            spawn_camera: false,
                            ..AlsCharacterBlueprint::new(Transform::from_xyz(0.0, 1.5, 0.0))
                        },
                    )
                },
            )
            .unwrap();
        let avatar = app
            .world()
            .get::<AlsCharacterAssembly>(actor)
            .unwrap()
            .avatar;
        let scene = app
            .world()
            .get::<AnimatedSceneHandle>(avatar)
            .unwrap()
            .handle
            .clone();
        app.world_mut()
            .get_mut::<AnimatedSceneHandle>(avatar)
            .unwrap()
            .handle = default();
        app.world_mut().get_mut::<LinearVelocity>(actor).unwrap().0 = Vec3::X;
        for _ in 0..2 {
            app.update();
            assert!(app.world().get::<AlsBirthAnimationPending>(actor).is_some());
            assert_eq!(*app.world().get::<Gait>(actor).unwrap(), Gait::Running);
            assert!(
                app.world()
                    .get::<Transform>(actor)
                    .unwrap()
                    .translation
                    .abs_diff_eq(Vec3::new(0.0, 1.5, 0.0), 1e-6)
            );
        }
        app.world_mut().get_mut::<LinearVelocity>(actor).unwrap().0 = Vec3::ZERO;
        app.world_mut()
            .entity_mut(actor)
            .remove::<crate::actual_motion::AlsActualMotion>();
        app.world_mut()
            .get_mut::<AnimatedSceneHandle>(avatar)
            .unwrap()
            .handle = scene;
        crate::roll_drive::tests::ready_roll_player(&mut app, avatar);
        if app.world().resource::<Capture>().ordinary.is_none() {
            app.update();
        }
        let output = app.world().resource::<Capture>();
        let (position, mode, frame, curves, deltas) = output.initial.unwrap();
        assert!((position.y - 1.5).abs() < 1e-6);
        assert_eq!(mode, LocomotionState::Grounded);
        assert_eq!(frame, 1);
        assert_eq!(curves, [1.0; 6]);
        assert_eq!(deltas, [0.0; 3]);
        let (position, mode, frame) = output.ordinary.unwrap();
        assert_eq!(position.y, 1.494_55);
        assert_eq!(mode, LocomotionState::Air);
        assert_eq!(frame, 2);
    }

    #[test]
    fn first_ordinary_animation_update_consumes_jump_without_selecting_jump_pose() {
        use crate::{
            als::{
                character::AlsCollisionLayer,
                control_scheme::{AlsControlScheme, AlsControlSchemeActionDiscriminant},
                input::AlsInputCommand,
                state::LocomotionState,
            },
            animation_visibility::AlsAnimationTickOption,
            character_assembly::{
                AlsCharacterAssembly, AlsCharacterBlueprint, spawn_als_character,
            },
            extensions::{
                interruptible_fsm::InterruptibleFsmState,
                post_physics_animation::AnimationPoseUpdateGate,
            },
        };
        use avian3d::prelude::{Collider, CollisionLayers, LayerMask, RigidBody};
        use bevy::ecs::system::RunSystemOnce;
        use bevy_animation_graph::core::{
            animated_scene::AnimatedSceneInstance, animation_graph::AnimationGraph,
            animation_graph_player::AnimationGraphPlayer, context::node_states::StateKey,
        };
        use bevy_tnua::TnuaController;

        let (mut app, _, _, _) = crate::camera_component::tests::loaded_camera_scene();
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f32(1.0 / 60.0),
        ));
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(100.0, 1.0, 100.0),
            Transform::from_xyz(0.0, -0.5, 0.0),
            CollisionLayers::new(
                [AlsCollisionLayer::Default, AlsCollisionLayer::Visibility],
                LayerMask::ALL,
            ),
        ));
        let fall = "edccb450-4d4e-e5ca-ad05-ae8b1d2b22d3";
        let jump = "3c79c741-4298-752a-f317-838c0ca38811";
        for (x, delay_jump) in [(0.0, false), (5.0, true)] {
            if delay_jump {
                app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                    std::time::Duration::ZERO,
                ));
            }
            let actor = app
                .world_mut()
                .run_system_once_with(
                    |In(x): In<f32>,
                     mut commands: Commands,
                     assets: Res<AssetServer>,
                     mut params: ResMut<Assets<crate::als::params::AlsMovementParams>>,
                     mut configs: ResMut<
                        Assets<crate::als::control_scheme::AlsControlSchemeConfig>,
                    >| {
                        spawn_als_character(
                            &mut commands,
                            &assets,
                            &mut params,
                            &mut configs,
                            AlsCharacterBlueprint::new(Transform::from_xyz(x, 0.924, 0.0)),
                        )
                    },
                    x,
                )
                .unwrap();
            let avatar = app
                .world()
                .get::<AlsCharacterAssembly>(actor)
                .unwrap()
                .avatar;
            app.world_mut()
                .entity_mut(avatar)
                .insert(AlsAnimationTickOption::AlwaysTickPoseAndRefreshBones);
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
            let entity = loop {
                app.update();
                if let Some(scene) = app.world().get::<AnimatedSceneInstance>(avatar)
                    && app
                        .world()
                        .get::<AnimationGraphPlayer>(scene.player_entity())
                        .is_some_and(|player| player.get_default_output_pose().is_some())
                {
                    break scene.player_entity();
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "Birth animation did not become ready"
                );
                std::thread::sleep(std::time::Duration::from_millis(5));
            };
            if delay_jump {
                assert!(
                    app.world()
                        .get::<crate::als::character::AlsBirthAnimationPending>(actor)
                        .is_some()
                );
                app.update();
                assert_eq!(
                    app.world()
                        .get::<AnimationGraphPlayer>(entity)
                        .unwrap()
                        .get_context_arena()
                        .unwrap()
                        .frame_index(),
                    1
                );
                app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                    std::time::Duration::from_secs_f32(1.0 / 60.0),
                ));
                app.update();
            }
            let player = app.world().get::<AnimationGraphPlayer>(entity).unwrap();
            assert_eq!(player.get_context_arena().unwrap().frame_index(), 1);
            assert_eq!(player.elapsed(), 0.0);
            assert!(
                app.world()
                    .get::<crate::als::character::AlsBirthAnimationPending>(actor)
                    .is_none()
            );
            let ground = app
                .world()
                .get::<bevy_tnua::TnuaSensorsEntities<AlsControlScheme>>(actor)
                .unwrap()
                .ground
                .unwrap();
            assert!(
                app.world()
                    .get::<bevy_tnua::TnuaProximitySensor>(ground)
                    .is_some()
            );
            if delay_jump {
                app.update();
            }
            assert_eq!(
                *app.world().get::<LocomotionState>(actor).unwrap(),
                LocomotionState::Grounded
            );
            app.world_mut()
                .get_mut::<AlsInputCommand>(actor)
                .unwrap()
                .jump_pressed = true;
            app.update();

            let flow = app
                .world()
                .get::<TnuaController<AlsControlScheme>>(actor)
                .unwrap()
                .action_flow_status();
            assert_eq!(
                flow.just_starting(),
                Some(AlsControlSchemeActionDiscriminant::Jump)
            );
            assert_eq!(
                *app.world().get::<LocomotionState>(actor).unwrap(),
                LocomotionState::Air
            );
            assert!(
                app.world()
                    .get::<crate::actual_motion::AlsActualMotion>(actor)
                    .unwrap()
                    .world_velocity
                    .y
                    > 0.0
            );
            let player = app.world().get::<AnimationGraphPlayer>(entity).unwrap();
            assert_eq!(
                player
                    .get_global_input_data("animation.pending_update")
                    .unwrap()
                    .as_bool()
                    .unwrap(),
                !delay_jump
            );
            assert!(
                !app.world()
                    .get::<AnimationPoseUpdateGate>(entity)
                    .unwrap()
                    .pending_update
            );
            let arena = player.get_context_arena().unwrap();
            let graphs = app.world().resource::<Assets<AnimationGraph>>();
            let state = arena
                .iter_context_ids()
                .find_map(|id| {
                    let context = arena.get_context(id)?;
                    let node = graphs
                        .get(context.get_graph_id())?
                        .nodes
                        .values()
                        .find(|node| node.name == "Locomotion States\nState Machine")?;
                    if !context.node_caches.is_updated(node.id, StateKey::Default) {
                        return None;
                    }
                    context
                        .node_states
                        .get_all_upcoming_states::<InterruptibleFsmState>(node.id)
                        .ok()?
                        .find(|state| state.last_output_pose().is_some())
                })
                .expect("Locomotion state machine must publish its current state");
            let (expected, other) = if delay_jump {
                (jump, fall)
            } else {
                (fall, jump)
            };
            assert_eq!(
                state.recorded_state_weight(expected),
                1.0,
                "Wrong startup pose (delay_jump={delay_jump})"
            );
            assert_eq!(state.recorded_state_weight(other), 0.0);
            app.world_mut()
                .get_mut::<AlsInputCommand>(actor)
                .unwrap()
                .jump_pressed = false;
        }
    }

    #[test]
    fn animation_phase_follows_rebuilt_scene_on_retained_avatar() {
        use crate::{
            character_assembly::{AlsCharacterAssembly, AlsCharacterAvatar},
            extensions::post_physics_animation::PostPhysicsAnimation,
        };
        use bevy::world_serialization::{WorldAsset, WorldInstanceReady, WorldInstanceSpawner};
        use bevy_animation_graph::core::{
            animated_scene::{AnimatedSceneHandle, AnimatedSceneInstance},
            animation_graph_player::AnimationGraphPlayer,
        };

        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            bevy_animation_graph::AnimationGraphPlugin::default(),
        ));
        app.add_systems(Update, mark_character_animation_phase);
        let character = app.world_mut().spawn(AlsCharacter).id();
        let avatar = app
            .world_mut()
            .spawn((
                AlsCharacterAvatar { character },
                AnimatedSceneHandle::default(),
            ))
            .id();
        app.world_mut()
            .entity_mut(character)
            .insert(AlsCharacterAssembly {
                avatar,
                camera: None,
            });
        let ready = |app: &mut App| {
            app.world_mut().trigger(WorldInstanceReady {
                entity: avatar,
                instance_id: WorldInstanceSpawner::default().spawn(Handle::<WorldAsset>::default()),
            });
            app.world_mut().flush();
            app.world_mut().run_schedule(Update);
        };
        let old_player = app
            .world_mut()
            .spawn((AnimationGraphPlayer::new(default()), ChildOf(avatar)))
            .id();
        ready(&mut app);
        assert!(
            app.world()
                .get::<PostPhysicsAnimation>(old_player)
                .is_some()
        );
        app.world_mut().entity_mut(old_player).remove::<ChildOf>();
        let player = app
            .world_mut()
            .spawn((AnimationGraphPlayer::new(default()), ChildOf(avatar)))
            .id();
        ready(&mut app);
        assert_eq!(
            app.world()
                .get::<AnimatedSceneInstance>(avatar)
                .unwrap()
                .player_entity(),
            player
        );
        assert!(app.world().get::<PostPhysicsAnimation>(player).is_some());
        assert!(
            app.world()
                .get::<PostPhysicsAnimation>(old_player)
                .is_none()
        );
        app.world_mut()
            .entity_mut(avatar)
            .remove::<AlsCharacterAvatar>();
        app.world_mut().run_schedule(Update);
        assert!(app.world().get::<PostPhysicsAnimation>(player).is_none());
        let unrelated = app
            .world_mut()
            .spawn((AnimationGraphPlayer::new(default()), PostPhysicsAnimation))
            .id();
        app.world_mut()
            .entity_mut(avatar)
            .insert(AlsCharacterAvatar { character });
        app.world_mut().run_schedule(Update);
        assert!(app.world().get::<PostPhysicsAnimation>(player).is_some());
        assert!(app.world().get::<PostPhysicsAnimation>(unrelated).is_some());
        app.world_mut()
            .entity_mut(character)
            .remove::<AlsCharacter>();
        app.world_mut().run_schedule(Update);
        assert!(app.world().get::<PostPhysicsAnimation>(player).is_none());
        app.world_mut().entity_mut(character).insert(AlsCharacter);
        app.world_mut().run_schedule(Update);
        assert!(app.world().get::<PostPhysicsAnimation>(player).is_some());
        app.world_mut()
            .entity_mut(player)
            .remove::<PostPhysicsAnimation>();
        app.world_mut().run_schedule(Update);
        assert!(app.world().get::<PostPhysicsAnimation>(player).is_some());
        app.world_mut().entity_mut(avatar).despawn();
        app.world_mut().run_schedule(Update);
        assert!(
            app.world()
                .get::<PostPhysicsAnimation>(old_player)
                .is_none()
        );
        assert!(app.world().get::<PostPhysicsAnimation>(unrelated).is_some());
    }

    #[test]
    fn foot_bones_follow_selected_avatar_when_old_bones_remain_alive() {
        fn spawn_feet(world: &mut World, avatar: Entity) -> [Entity; 6] {
            ["thigh_l", "calf_l", "foot_l", "thigh_r", "calf_r", "foot_r"].map(|name| {
                world
                    .spawn((
                        Name::new(name),
                        EntityPath::from(vec![name.to_owned()])
                            .id()
                            .animation_target_id(),
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
        assert_eq!(
            app.world().get::<AlsFootBones>(character).unwrap().left,
            old_feet[2]
        );
        app.world_mut()
            .entity_mut(character)
            .insert(AlsFootLockState {
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
        assert_eq!(
            app.world().get::<AlsFootBones>(character).unwrap().left,
            new_feet[2]
        );
        assert!(
            !app.world()
                .get::<AlsFootLockState>(character)
                .unwrap()
                .valid
        );

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
        assert_eq!(
            app.world().get::<AlsFootBones>(character).unwrap().left,
            last_feet[2]
        );
    }

    #[test]
    fn source_gameplay_tag_table_accepts_als_gait() {
        assert!(
            GameplayTagsSettings::parse_tag_table(include_str!(
                "../../../assets/ALS/Data/als_gameplay_tags.json"
            ))
            .is_ok()
        );
        let tag = GameplayTag::try_new("Als.Gait.Walking").unwrap();
        assert!(tag.is_valid());
        assert!(tag.matches_tag_exact(&GameplayTag::new("Als.Gait.Walking")));
    }
}
