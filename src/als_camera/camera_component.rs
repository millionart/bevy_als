use crate::ab_als_root::{
    gait_tag, locomotion_action_tag, locomotion_mode_tag, rotation_mode_tag, stance_tag,
};
#[cfg(test)]
use crate::als::character::AlsCollisionLayer;
use crate::als::{
    aim::AlsLastFreeRotationMode,
    state::{Gait, LocomotionState, RotationMode, Stance},
};
use crate::als_camera::{
    AlsCameraAnimationFrame, AlsCameraCurveValues, AlsCameraDefinition, AlsCameraDefinitionHandle,
    AlsCharacterCamera, CAMERA_CURVE_NAMES,
};
use crate::extensions::post_physics_animation::ManualAnimationPlayer;
use bevy::{
    prelude::*,
    time::{Real, Virtual},
};
use bevy_animation_graph::core::{
    animated_scene::AnimatedSceneInstance, animation_graph_player::AnimationGraphPlayer,
    context::system_resources::SystemResources, plugin::AnimationGraphSet,
    systems::run_animation_player,
};

#[derive(Component)]
#[require(
    crate::als_camera::AlsCameraRig,
    AlsCameraCurveValues,
    AlsCameraAnimationFrame
)]
pub struct AlsSourceCameraComponent {
    bound_player: Option<Entity>,
    active: bool,
    refresh_view: bool,
}

impl Default for AlsSourceCameraComponent {
    fn default() -> Self {
        Self {
            bound_player: None,
            active: false,
            refresh_view: false,
        }
    }
}

impl AlsSourceCameraComponent {
    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn activate(&mut self, reset: bool) {
        self.refresh_view |= reset || !self.active;
        self.active = true;
    }

    pub fn deactivate(&mut self) {
        self.refresh_view |= self.active;
        self.active = false;
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsCameraAnimationSet;

pub struct AlsSourceCameraPlugin;

impl Plugin for AlsSourceCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            initialize_components.before(AnimationGraphSet::PrePhysics),
        )
        .add_systems(
            PostUpdate,
            update_camera_animation
                .run_if(bevy::ecs::schedule::common_conditions::not(
                    bevy::time::common_conditions::paused,
                ))
                .in_set(AlsCameraAnimationSet)
                .after(crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate)
                .before(AnimationGraphSet::PostPhysics),
        )
        .add_systems(
            Update,
            (remove_orphaned_views, publish_player_view_yaw)
                .chain()
                .before(crate::als::view::AlsViewInputSet),
        );
    }
}

fn initialize_components(
    mut commands: Commands,
    definitions: Res<Assets<AlsCameraDefinition>>,
    characters: Query<&AlsCameraDefinitionHandle>,
    unclaimed_players: Query<(), (With<AnimationGraphPlayer>, Without<ManualAnimationPlayer>)>,
    mut frames: Query<&mut AlsCameraAnimationFrame>,
    mut components: Query<(
        Entity,
        &AlsCharacterCamera,
        &mut AlsSourceCameraComponent,
        &mut Transform,
        &mut Visibility,
        Option<&AnimatedSceneInstance>,
    )>,
) {
    for (entity, owner, mut component, mut transform, mut visibility, scene) in &mut components {
        let Some(scene) = scene else {
            if let Ok(mut frame) = frames.get_mut(entity) {
                frame.active = false;
            }
            continue;
        };
        let player = scene.player_entity();
        if unclaimed_players.contains(player) {
            commands.entity(player).insert(ManualAnimationPlayer);
        }
        if component.bound_player == Some(player) {
            continue;
        }
        let Ok(handle) = characters.get(owner.character) else {
            continue;
        };
        let Some(definition) = definitions.get(&handle.0) else {
            continue;
        };
        *transform = Transform {
            translation: definition.attachment_translation,
            rotation: definition.attachment_rotation,
            scale: definition.attachment_scale,
        };
        *visibility = if definition.hidden_in_game {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if component.bound_player.is_none() {
            if definition.auto_activate {
                component.activate(false);
            }
            if !definition.right_shoulder {
                commands
                    .entity(owner.character)
                    .insert(crate::als::input_request::AlsCameraLeftShoulder);
            }
            if definition.override_field_of_view {
                commands
                    .entity(entity)
                    .insert(crate::als_camera::AlsCameraFovOverride(
                        definition.field_of_view_override_deg,
                    ));
            }
        }
        component.bound_player = Some(player);
    }
}

fn update_camera_animation(
    mut source_inputs: Query<&mut crate::extensions::SourceAnimationInputs>,
    assets: Res<AssetServer>,
    virtual_time: Res<Time<Virtual>>,
    real_time: Res<Time<Real>>,
    definitions: Res<Assets<AlsCameraDefinition>>,
    mut components: Query<(
        &AlsCharacterCamera,
        &mut AlsSourceCameraComponent,
        &AnimatedSceneInstance,
        &mut AlsCameraCurveValues,
        &mut AlsCameraAnimationFrame,
    )>,
    characters: Query<(
        &AlsCameraDefinitionHandle,
        &Gait,
        &Stance,
        &LocomotionState,
        &RotationMode,
        Option<&AlsLastFreeRotationMode>,
        Has<crate::als::input_request::AlsFirstPersonView>,
        Has<crate::als::input_request::AlsCameraLeftShoulder>,
        Option<&crate::als_camera::AlsCameraCustomTimeDilation>,
        Option<&crate::animation_events::AlsNotifyLocomotionAction>,
        Option<&crate::als::mantle::AlsMantleState>,
        Option<&crate::als::roll::AlsRollState>,
    )>,
    mut players: Query<&mut AnimationGraphPlayer>,
    resources: SystemResources,
) {
    for (owner, mut component, scene, mut curves, mut frame) in &mut components {
        let Ok((
            handle,
            gait,
            stance,
            locomotion,
            rotation,
            desired,
            first_person,
            left_shoulder,
            dilation,
            action,
            mantle,
            roll,
        )) = characters.get(owner.character)
        else {
            continue;
        };
        frame.active = component.bound_player == Some(scene.player_entity()) && component.active;
        frame.force_no_lag = false;
        if !frame.active {
            frame.force_no_lag = std::mem::take(&mut component.refresh_view);
            continue;
        }
        let Some(definition) = definitions.get(&handle.0) else {
            continue;
        };
        let Ok(mut player) = players.get_mut(scene.player_entity()) else {
            frame.active = false;
            continue;
        };
        if let bevy_animation_graph::core::animation_graph_player::AnimationSource::Graph(graph) =
            player.get_animation_source()
            && !assets.is_loaded_with_dependencies(graph)
        {
            frame.active = false;
            continue;
        }
        let rotation = crate::als_camera::camera_animation_rotation_mode(
            *rotation,
            first_person,
            desired.copied(),
        );
        for (key, tag) in [
            ("gait", gait_tag(*gait)),
            ("stance", stance_tag(*stance)),
            ("locomotion_mode", locomotion_mode_tag(*locomotion)),
            ("rotation_mode", rotation_mode_tag(rotation)),
            (
                "locomotion_action",
                locomotion_action_tag(action, mantle, roll),
            ),
            (
                "view_mode",
                bevy_gameplay_tag::GameplayTag::new(if first_person {
                    "Als.ViewMode.FirstPerson"
                } else {
                    "Als.ViewMode.ThirdPerson"
                }),
            ),
        ] {
            source_inputs
                .get_mut(scene.player_entity())
                .unwrap()
                .set_gameplay_tag_input(key, tag);
        }
        player.set_global_input_data("b_right_shoulder", !left_shoulder);
        let delta = if definition.settings.ignore_time_dilation {
            real_time.delta_secs()
        } else {
            virtual_time.delta_secs() * dilation.map_or(1.0, |value| value.0)
        };
        frame.delta_seconds = delta;
        player.set_global_input_data("animation.delta_seconds", delta);
        let mut time = Time::default();
        time.advance_by(std::time::Duration::from_secs_f32(delta));
        source_inputs
            .get(scene.player_entity())
            .unwrap()
            .evaluate(scene.player_entity(), || {
                run_animation_player(scene.player_entity(), player, &time, &resources)
            });
        let player = players.get(scene.player_entity()).unwrap();
        frame.active = player.get_error().is_none() && player.get_default_output_pose().is_some();
        if frame.active {
            frame.force_no_lag = std::mem::take(&mut component.refresh_view);
        }
        if let Some(context) = player.get_context_arena().and_then(|arena| {
            let graph = resources
                .animation_graph_assets
                .get(arena.get_toplevel().get_graph_id())?;
            let source = graph.edges_inverted.get(
                &bevy_animation_graph::core::animation_graph::TargetPin::OutputData("pose".into()),
            )?;
            let bevy_animation_graph::core::animation_graph::SourcePin::NodeData(node, _) = source
            else {
                return None;
            };
            crate::extensions::animation_context::node_context_state(
                &arena.get_toplevel().node_states,
                bevy_animation_graph::core::context::node_states::StateKey::Default,
                *node,
            )
        }) {
            for (index, name) in CAMERA_CURVE_NAMES.iter().enumerate() {
                curves.values[index] = context
                    .curves
                    .get(&crate::extensions::SourceCurveName::from(*name))
                    .copied()
                    .unwrap_or(0.0);
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use bevy::world_serialization::WorldSerializationPlugin;
    use bevy_animation_graph::core::animated_scene::{AnimatedScene, AnimatedSceneHandle};

    #[test]
    fn movement_view_uses_the_completed_render_transform_and_clears_invalid_views() {
        use crate::als::{
            character::AlsCharacter,
            input::AlsInputCommand,
            motion::AlsViewYaw,
            view::{AlsPlayerViewYaw, AlsViewInputPlugin, AlsViewInputSet, AlsViewPitch},
        };
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, AlsViewInputPlugin))
            .add_systems(
                Update,
                (remove_orphaned_views, publish_player_view_yaw)
                    .chain()
                    .before(AlsViewInputSet),
            );
        let character = app
            .world_mut()
            .spawn((
                AlsCharacter,
                AlsViewYaw(1.2),
                AlsViewPitch::default(),
                AlsInputCommand {
                    look_delta: Vec2::new(20.0, 0.0),
                    ..default()
                },
            ))
            .id();
        let source = app
            .world_mut()
            .spawn((
                AlsSourceCameraComponent::default(),
                AlsCharacterCamera { character },
            ))
            .id();
        let view = app
            .world_mut()
            .spawn((
                Camera3d::default(),
                crate::als_camera::AlsCameraViewSource(source),
                AlsCharacterCamera { character },
                Transform::from_rotation(Quat::from_rotation_y(0.4)),
            ))
            .id();
        app.update();
        assert!(app.world().get::<AlsPlayerViewYaw>(character).is_none());
        app.update();
        assert!((app.world().get::<AlsPlayerViewYaw>(character).unwrap().0 - 0.4).abs() < 1e-6);
        assert!(
            (app.world().get::<AlsViewYaw>(character).unwrap().0
                - (1.2 - 2.0 * 20.0_f32.to_radians()))
            .abs()
                < 1e-6
        );
        app.world_mut().get_mut::<Transform>(view).unwrap().rotation = Quat::from_rotation_y(0.8);
        app.update();
        assert!((app.world().get::<AlsPlayerViewYaw>(character).unwrap().0 - 0.4).abs() < 1e-6);
        app.update();
        assert!((app.world().get::<AlsPlayerViewYaw>(character).unwrap().0 - 0.8).abs() < 1e-6);
        app.world_mut().get_mut::<Camera>(view).unwrap().is_active = false;
        app.update();
        assert!(app.world().get::<AlsPlayerViewYaw>(character).is_none());
        app.world_mut().get_mut::<Camera>(view).unwrap().is_active = true;
        app.update();
        assert!(app.world().get::<AlsPlayerViewYaw>(character).is_some());
        app.world_mut().despawn(source);
        app.update();
        assert!(app.world().get_entity(view).is_err());
        assert!(app.world().get::<AlsPlayerViewYaw>(character).is_none());
    }

    #[test]
    fn player_view_readiness_tracks_publication_and_camera_replacement() {
        use crate::als::character::AlsCharacter;
        use crate::als::view::{AlsPlayerViewReadiness, AlsPlayerViewYaw};
        use crate::character_assembly::{AlsCharacterAssembly, AlsPlayerCharacter};
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin))
            .add_systems(
                Update,
                (remove_orphaned_views, publish_player_view_yaw).chain(),
            );
        let character = app
            .world_mut()
            .spawn((AlsCharacter, AlsPlayerCharacter, Transform::IDENTITY))
            .id();
        let source = app
            .world_mut()
            .spawn((
                AlsSourceCameraComponent::default(),
                AlsCharacterCamera { character },
            ))
            .id();
        let view = app
            .world_mut()
            .spawn((
                Camera3d::default(),
                crate::als_camera::AlsCameraViewSource(source),
                AlsCharacterCamera { character },
                Transform::from_rotation(Quat::from_rotation_y(0.4)),
            ))
            .id();
        app.world_mut()
            .entity_mut(character)
            .insert(AlsCharacterAssembly {
                avatar: Entity::PLACEHOLDER,
                camera: Some(view),
            });
        app.update();
        let readiness = app
            .world()
            .get::<AlsPlayerViewReadiness>(character)
            .unwrap();
        assert_eq!(readiness.source, source);
        assert!(!readiness.has_published);
        assert!(app.world().get::<AlsPlayerViewYaw>(character).is_none());
        app.update();
        assert!(
            app.world()
                .get::<AlsPlayerViewReadiness>(character)
                .unwrap()
                .has_published
        );
        assert!(app.world().get::<AlsPlayerViewYaw>(character).is_some());
        app.world_mut().get_mut::<Camera>(view).unwrap().is_active = false;
        app.update();
        assert!(app.world().get::<AlsPlayerViewYaw>(character).is_none());
        assert!(
            app.world()
                .get::<AlsPlayerViewReadiness>(character)
                .unwrap()
                .has_published
        );
        app.world_mut().despawn(source);
        app.update();
        assert!(
            app.world()
                .get::<AlsPlayerViewReadiness>(character)
                .is_none()
        );
        assert_eq!(
            app.world()
                .get::<AlsCharacterAssembly>(character)
                .unwrap()
                .camera,
            None
        );
        let replacement = app
            .world_mut()
            .spawn((
                AlsSourceCameraComponent::default(),
                AlsCharacterCamera { character },
            ))
            .id();
        let view = app
            .world_mut()
            .spawn((
                Camera3d::default(),
                crate::als_camera::AlsCameraViewSource(replacement),
                AlsCharacterCamera { character },
                Transform::IDENTITY,
            ))
            .id();
        app.world_mut()
            .get_mut::<AlsCharacterAssembly>(character)
            .unwrap()
            .camera = Some(view);
        app.update();
        let readiness = app
            .world()
            .get::<AlsPlayerViewReadiness>(character)
            .unwrap();
        assert_eq!(readiness.source, replacement);
        assert!(!readiness.has_published);
        app.world_mut().get_mut::<Camera>(view).unwrap().is_active = false;
        app.update();
        assert!(
            app.world()
                .get::<AlsPlayerViewReadiness>(character)
                .is_none()
        );
        app.world_mut().get_mut::<Camera>(view).unwrap().is_active = true;
        app.update();
        assert!(
            app.world()
                .get::<AlsPlayerViewReadiness>(character)
                .unwrap()
                .has_published
        );
        app.world_mut()
            .entity_mut(character)
            .remove::<AlsPlayerCharacter>();
        app.update();
        assert!(
            app.world()
                .get::<AlsPlayerViewReadiness>(character)
                .is_none()
        );
    }

    #[test]
    fn cold_camera_drives_the_player_from_actor_rotation() {
        use crate::als::{
            control_scheme::AlsControlScheme,
            input::AlsInputCommand,
            motion::{AlsViewYaw, locomotion_drive_system},
            view::AlsPlayerViewYaw,
        };
        use crate::character_assembly::{AlsCharacterAssembly, AlsPlayerCharacter};
        use bevy::ecs::system::RunSystemOnce;
        use bevy_tnua::TnuaController;
        let (mut app, character, _) = crate::roll_drive::tests::loaded_roll_character();
        let mut transform = *app.world().get::<Transform>(character).unwrap();
        transform.rotation = Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2);
        app.world_mut()
            .entity_mut(character)
            .insert((
                AlsPlayerCharacter,
                transform,
                GlobalTransform::from(transform),
                AlsViewYaw(-150.0_f32.to_radians()),
            ))
            .remove::<AlsPlayerViewYaw>();
        app.world_mut()
            .get_mut::<AlsInputCommand>(character)
            .unwrap()
            .move_dir = Vec2::Y;
        let source = app
            .world_mut()
            .spawn((
                AlsSourceCameraComponent::default(),
                AlsCharacterCamera { character },
            ))
            .id();
        let view = app
            .world_mut()
            .spawn((
                Camera3d::default(),
                crate::als_camera::AlsCameraViewSource(source),
                AlsCharacterCamera { character },
                Transform::IDENTITY,
            ))
            .id();
        app.world_mut()
            .get_mut::<AlsCharacterAssembly>(character)
            .unwrap()
            .camera = Some(view);
        app.world_mut()
            .run_system_once(publish_player_view_yaw)
            .unwrap();
        app.world_mut()
            .run_system_once(locomotion_drive_system)
            .unwrap();
        let desired = app
            .world()
            .get::<TnuaController<AlsControlScheme>>(character)
            .unwrap()
            .basis
            .desired_motion;
        assert!(
            desired.abs_diff_eq(Vec3::X, 1e-6),
            "Cold player view selected the control target: {desired:?}"
        );
    }

    fn camera_scene() -> (App, Entity, Entity, Handle<AnimatedScene>) {
        let mut app = App::new();
        app.register_required_components::<crate::character_assembly::AlsCharacterAvatar, crate::animation_visibility::AlsAnimationTickOption>();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin {
                file_path: std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("assets")
                    .to_string_lossy()
                    .into_owned(),
                ..default()
            },
            bevy::gltf::GltfPlugin::default(),
            WorldSerializationPlugin,
            TransformPlugin,
            bevy::input::InputPlugin,
            bevy::gizmos::GizmoPlugin,
            bevy::animation::AnimationPlugin,
        ))
        .init_asset::<bevy::world_serialization::WorldAsset>()
        .init_asset::<Mesh>()
        .init_asset::<Image>()
        .init_asset::<StandardMaterial>()
        .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>()
        .add_plugins(crate::playable_character::AlsPlayableCharacterPlugin)
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_millis(100),
        ));
        let definition = app
            .world()
            .resource::<AssetServer>()
            .load("ALSCamera/Data/als_camera.camera_component.ron");
        let scene = app
            .world()
            .resource::<AssetServer>()
            .load("ALSCamera/AB_Als_Camera.static.animscn.ron");
        let character = app
            .world_mut()
            .spawn((
                Name::new("Owner"),
                Transform::IDENTITY,
                AlsCameraDefinitionHandle(definition),
                Gait::Walking,
                Stance::Standing,
                LocomotionState::Grounded,
                RotationMode::ViewDirection,
                crate::als_camera::AlsCameraCustomTimeDilation(0.5),
            ))
            .id();
        let component = app
            .world_mut()
            .spawn((
                Name::new("Camera"),
                AlsCharacterCamera { character },
                AlsSourceCameraComponent::default(),
                AnimatedSceneHandle::new(scene.clone()),
                ChildOf(character),
            ))
            .id();
        app.world_mut().entity_mut(character).insert((
            crate::als_camera::AlsCameraTarget,
            crate::als::motion::AlsViewYaw(0.0),
            crate::als::view::AlsViewPitch(0.0),
            crate::als_camera::AlsCameraSocketLocations {
                root: Vec3::ZERO,
                head: Vec3::Y * 1.6,
                first_person: Vec3::Y * 1.6,
                trace_shoulder_right: Vec3::new(0.2, 1.4, 0.0),
                trace_shoulder_left: Vec3::new(-0.2, 1.4, 0.0),
                mesh_rotation: Quat::IDENTITY,
                mesh_scale: 1.0,
                valid: true,
            },
        ));
        app.finish();
        app.cleanup();
        (app, character, component, scene)
    }

    pub(crate) fn loaded_camera_scene() -> (App, Entity, Entity, Handle<AnimatedScene>) {
        let (mut app, character, component, scene) = camera_scene();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(40);
        loop {
            app.update();
            let server = app.world().resource::<AssetServer>();
            if let bevy::asset::LoadState::Failed(error) = server.load_state(scene.id()) {
                panic!("Camera asset failed: {error}");
            }
            if let Some(instance) = app.world().get::<AnimatedSceneInstance>(component) {
                let player = app
                    .world()
                    .get::<AnimationGraphPlayer>(instance.player_entity())
                    .unwrap();
                assert!(player.get_error().is_none(), "{:?}", player.get_error());
                if player.get_default_output_pose().is_some() {
                    break;
                }
            }
            assert!(
                std::time::Instant::now() < deadline,
                "Camera scene load made no progress: asset={:?}, world_root={}, instance={}",
                server.load_state(scene.id()),
                app.world()
                    .get::<bevy::world_serialization::WorldAssetRoot>(component)
                    .is_some(),
                app.world()
                    .get::<AnimatedSceneInstance>(component)
                    .is_some()
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        (app, character, component, scene)
    }

    #[test]
    fn camera_player_waits_for_its_definition_before_first_evaluation() {
        use crate::extensions::SourceAnimationInputs;
        use bevy_animation_graph::core::animation_graph_player::AnimationSource;

        let (mut app, character, camera, _) = camera_scene();
        let definition = app
            .world()
            .get::<AlsCameraDefinitionHandle>(character)
            .unwrap()
            .0
            .clone();
        let pending = app
            .world()
            .resource::<Assets<AlsCameraDefinition>>()
            .reserve_handle();
        app.world_mut()
            .entity_mut(character)
            .insert(AlsCameraDefinitionHandle(pending));

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(40);
        let player_entity = loop {
            app.update();
            let world = app.world();
            if let Some(scene) = world.get::<AnimatedSceneInstance>(camera) {
                let entity = scene.player_entity();
                let player = world.get::<AnimationGraphPlayer>(entity).unwrap();
                let AnimationSource::Graph(graph) = player.get_animation_source() else {
                    panic!("Expected the source camera graph");
                };
                if world
                    .resource::<AssetServer>()
                    .is_loaded_with_dependencies(graph)
                    && world
                        .resource::<Assets<AlsCameraDefinition>>()
                        .get(&definition)
                        .is_some()
                {
                    assert!(player.get_error().is_none(), "{:?}", player.get_error());
                    assert!(world.get::<ManualAnimationPlayer>(entity).is_some());
                    assert!(player.get_default_output_pose().is_none());
                    assert!(
                        player
                            .get_global_input_data("animation.evaluate_pose")
                            .is_none()
                    );
                    assert!(!world.get::<AlsCameraAnimationFrame>(camera).unwrap().active);
                    break entity;
                }
            }
            assert!(
                std::time::Instant::now() < deadline,
                "Camera graph and definition load timed out"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        let ownership_tick = app
            .world()
            .entity(player_entity)
            .get_ref::<ManualAnimationPlayer>()
            .unwrap()
            .last_changed();
        app.update();
        assert_eq!(
            app.world()
                .entity(player_entity)
                .get_ref::<ManualAnimationPlayer>()
                .unwrap()
                .last_changed(),
            ownership_tick,
        );

        app.world_mut()
            .entity_mut(character)
            .insert(AlsCameraDefinitionHandle(definition));
        app.update();
        let world = app.world();
        let player = world.get::<AnimationGraphPlayer>(player_entity).unwrap();
        assert!(player.get_error().is_none(), "{:?}", player.get_error());
        assert!(player.get_default_output_pose().is_some());
        assert!(world.get::<AlsCameraAnimationFrame>(camera).unwrap().active);
        assert!(
            world
                .get::<SourceAnimationInputs>(player_entity)
                .unwrap()
                .gameplay_tag_input("locomotion_action")
                .is_some()
        );
    }

    #[test]
    fn camera_view_cleanup_tracks_source_lifetime_and_owner() {
        use bevy::ecs::system::RunSystemOnce;
        for case in ["live", "deleted", "removed", "foreign", "rebound"] {
            let mut world = World::new();
            let character = world.spawn_empty().id();
            let source = world
                .spawn((
                    AlsSourceCameraComponent::default(),
                    AlsCharacterCamera { character },
                ))
                .id();
            let view = world
                .spawn((
                    Camera3d::default(),
                    crate::als_camera::AlsCameraViewSource(source),
                    AlsCharacterCamera { character },
                ))
                .id();
            world
                .entity_mut(character)
                .insert(crate::character_assembly::AlsCharacterAssembly {
                    avatar: source,
                    camera: Some(view),
                });
            match case {
                "deleted" => {
                    world.entity_mut(source).despawn();
                }
                "removed" => {
                    world
                        .entity_mut(source)
                        .remove::<AlsSourceCameraComponent>();
                }
                "foreign" => {
                    let other = world.spawn_empty().id();
                    world
                        .entity_mut(source)
                        .insert(AlsCharacterCamera { character: other });
                }
                "rebound" => {
                    let replacement = world
                        .spawn((
                            AlsSourceCameraComponent::default(),
                            AlsCharacterCamera { character },
                        ))
                        .id();
                    world
                        .entity_mut(view)
                        .insert(crate::als_camera::AlsCameraViewSource(replacement));
                    world.entity_mut(source).despawn();
                }
                _ => {}
            }
            world.run_system_once(remove_orphaned_views).unwrap();
            assert_eq!(
                world.get_entity(view).is_ok(),
                matches!(case, "live" | "rebound"),
                "{case}"
            );
            assert_eq!(
                world
                    .get::<crate::character_assembly::AlsCharacterAssembly>(character)
                    .unwrap()
                    .camera
                    .is_some(),
                matches!(case, "live" | "rebound")
            );
            assert!(world.get_entity(character).is_ok());
        }
    }

    #[test]
    fn camera_view_history_resets_on_cuts_and_preserves_stable_frames() {
        use bevy::anti_alias::taa::TemporalAntiAliasing;
        let (mut app, character, component, _) = loaded_camera_scene();
        let view = app
            .world_mut()
            .spawn((
                Camera3d::default(),
                crate::als_camera::AlsCameraViewSource(component),
                AlsCharacterCamera { character },
                TemporalAntiAliasing { reset: false },
            ))
            .id();
        for event in [
            "bind",
            "steady",
            "teleport",
            "steady",
            "deactivate",
            "steady",
            "activate",
            "steady",
            "reset",
            "steady",
        ] {
            app.world_mut()
                .get_mut::<TemporalAntiAliasing>(view)
                .unwrap()
                .reset = false;
            match event {
                "teleport" => {
                    let mut sockets = app
                        .world_mut()
                        .get_mut::<crate::als_camera::AlsCameraSocketLocations>(character)
                        .unwrap();
                    sockets.root += Vec3::X * 3.0;
                    sockets.head += Vec3::X * 3.0;
                    sockets.first_person += Vec3::X * 3.0;
                    sockets.trace_shoulder_left += Vec3::X * 3.0;
                    sockets.trace_shoulder_right += Vec3::X * 3.0;
                }
                "deactivate" => app
                    .world_mut()
                    .get_mut::<AlsSourceCameraComponent>(component)
                    .unwrap()
                    .deactivate(),
                "activate" => app
                    .world_mut()
                    .get_mut::<AlsSourceCameraComponent>(component)
                    .unwrap()
                    .activate(false),
                "reset" => app
                    .world_mut()
                    .get_mut::<AlsSourceCameraComponent>(component)
                    .unwrap()
                    .activate(true),
                _ => {}
            }
            app.update();
            assert_eq!(
                app.world().get::<TemporalAntiAliasing>(view).unwrap().reset,
                event != "steady",
                "{event}"
            );
        }
    }

    #[test]
    fn inactive_source_camera_uses_character_eye_view_without_resetting_owner() {
        use crate::als::{motion::AlsViewYaw, view::AlsViewPitch};
        let (mut app, character, component, _) = loaded_camera_scene();
        let view = app
            .world_mut()
            .spawn((
                Camera3d::default(),
                Transform::IDENTITY,
                crate::als_camera::AlsCameraViewSource(component),
                AlsCharacterCamera { character },
            ))
            .id();
        app.world_mut()
            .get_mut::<AlsSourceCameraComponent>(component)
            .unwrap()
            .deactivate();
        for stance in [Stance::Standing, Stance::Crouching] {
            app.world_mut().entity_mut(character).insert((
                stance,
                AlsViewYaw(0.7),
                AlsViewPitch(0.2),
            ));
            app.update();
            let transform = app.world().get::<Transform>(view).unwrap();
            let owner = app.world().get::<Transform>(character).unwrap();
            let height = if stance == Stance::Standing {
                0.64
            } else {
                0.44799999
            };
            assert!(
                transform
                    .translation
                    .abs_diff_eq(owner.translation + Vec3::Y * height, 1e-5)
            );
            assert!(
                transform
                    .rotation
                    .abs_diff_eq(Quat::from_euler(EulerRot::YXZ, 0.7, 0.2, 0.0), 1e-5)
            );
            assert!(
                (app.world()
                    .get::<crate::als_camera::AlsCameraFieldOfView>(view)
                    .unwrap()
                    .0
                    - 90.0)
                    .abs()
                    < 1e-5
            );
            assert!(
                !app.world()
                    .get::<AlsSourceCameraComponent>(component)
                    .unwrap()
                    .is_active()
            );
        }
        app.world_mut()
            .get_mut::<AlsSourceCameraComponent>(component)
            .unwrap()
            .activate(false);
        app.update();
        assert!(
            app.world()
                .get::<crate::als_camera::AlsCameraViewInfo>(component)
                .unwrap()
                .valid
        );
    }

    #[test]
    fn actual_ragdoll_root_retains_camera_actor_ownership_after_detachment() {
        use crate::character_assembly::{
            AlsCharacterAssembly, AlsCharacterBlueprint, spawn_als_character,
        };
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, _, _, _) = loaded_camera_scene();
        let character = app
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
                        AlsCharacterBlueprint::new(Transform::from_xyz(0.0, 3.0, 0.0)),
                    )
                },
            )
            .unwrap();
        app.world_mut().entity_mut(character).insert((
            avian3d::prelude::RigidBody::Kinematic,
            bevy_tnua::TnuaToggle::Disabled,
        ));
        let avatar = app
            .world()
            .get::<AlsCharacterAssembly>(character)
            .unwrap()
            .avatar;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(40);
        let player = loop {
            app.update();
            if let Some(scene) = app.world().get::<AnimatedSceneInstance>(avatar) {
                let player = app
                    .world()
                    .get::<AnimationGraphPlayer>(scene.player_entity())
                    .unwrap();
                let ready = match player.get_animation_source() {
                    bevy_animation_graph::core::animation_graph_player::AnimationSource::Graph(
                        graph,
                    ) => app
                        .world()
                        .resource::<AssetServer>()
                        .is_loaded_with_dependencies(graph),
                    _ => false,
                };
                if ready {
                    assert!(player.get_error().is_none(), "{:?}", player.get_error());
                    if player.get_default_output_pose().is_some() {
                        break scene.player_entity();
                    }
                }
            }
            assert!(
                std::time::Instant::now() < deadline,
                "Character scene did not become ready"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        app.world_mut()
            .entity_mut(character)
            .insert(crate::als::motion::AlsInputOverride(
                crate::als::input::AlsInputCommand {
                    ragdoll_pressed: true,
                    ..default()
                },
            ));
        app.update();
        app.world_mut()
            .get_mut::<crate::als::motion::AlsInputOverride>(character)
            .unwrap()
            .0
            .ragdoll_pressed = false;
        for _ in 0..4 {
            app.update();
        }
        assert!(
            app.world().get::<ChildOf>(player).is_none(),
            "Ragdoll player must use its source world-space handoff"
        );
        let root = app
            .world()
            .get::<AnimationGraphPlayer>(player)
            .unwrap()
            .get_spawned_ragdoll()
            .expect("Actual character must spawn its ragdoll")
            .root;
        assert!(app.world().get::<ChildOf>(root).is_none());
        assert_eq!(
            app.world()
                .get::<crate::als_camera::AlsCameraTraceOwner>(root),
            Some(&crate::als_camera::AlsCameraTraceOwner(character))
        );
    }

    #[test]
    fn ragdoll_collision_ownership_does_not_collapse_third_person_camera() {
        use avian3d::prelude::{Collider, CollisionLayers, RigidBody};
        let (mut app, character, component, _) = loaded_camera_scene();
        app.world_mut().entity_mut(character).insert((
            crate::animation_events::AlsNotifyLocomotionAction::Ragdolling,
            LocomotionState::None,
        ));
        for _ in 0..12 {
            app.update();
        }
        let before = *app
            .world()
            .get::<crate::als_camera::AlsCameraDebugInfo>(component)
            .unwrap();
        assert!(before.valid && !before.trace_blocking_hit);
        assert_eq!(before.first_person_override, 0.0);
        let root = app
            .world_mut()
            .spawn((
                Transform::IDENTITY,
                crate::als_camera::AlsCameraTraceOwner(character),
            ))
            .id();
        let collider = app
            .world_mut()
            .spawn((
                ChildOf(root),
                RigidBody::Static,
                Collider::sphere(0.35),
                Transform::from_translation(before.trace_start.lerp(before.trace_end, 0.25)),
                CollisionLayers::NONE,
            ))
            .id();
        app.update();
        app.world_mut()
            .entity_mut(collider)
            .insert(CollisionLayers::new(
                [AlsCollisionLayer::Default, AlsCollisionLayer::Visibility],
                avian3d::prelude::LayerMask::ALL,
            ));
        for along_boom in [0.25, 0.0] {
            app.world_mut()
                .get_mut::<Transform>(collider)
                .unwrap()
                .translation = before.trace_start.lerp(before.trace_end, along_boom);
            for _ in 0..4 {
                app.update();
                let debug = app
                    .world()
                    .get::<crate::als_camera::AlsCameraDebugInfo>(component)
                    .unwrap();
                assert_eq!(debug.first_person_override, 0.0);
                assert!(
                    !debug.trace_blocking_hit
                        && !debug.trace_adjusted_start
                        && debug.trace_distance_ratio > 0.99,
                    "Owned ragdoll collider changed third-person camera: {debug:?}"
                );
            }
        }
        let other_actor = app.world_mut().spawn_empty().id();
        app.world_mut()
            .get_mut::<Transform>(collider)
            .unwrap()
            .translation = before.trace_start.lerp(before.trace_end, 0.25);
        app.world_mut()
            .entity_mut(root)
            .insert(crate::als_camera::AlsCameraTraceOwner(other_actor));
        app.update();
        let debug = app
            .world()
            .get::<crate::als_camera::AlsCameraDebugInfo>(component)
            .unwrap();
        assert!(
            debug.trace_blocking_hit,
            "Other actor colliders must still block the camera"
        );
    }

    #[test]
    fn source_camera_pause_preserves_animation_lag_and_view_until_resume() {
        use crate::als_camera::{AlsCameraCurve, AlsCameraRuntime, AlsCameraViewInfo};
        for ignore in [true, false] {
            for owner_dilation in [0.0_f32, 0.5, 1.0] {
                let (mut app, character, component, _) = loaded_camera_scene();
                let handle = app
                    .world()
                    .get::<AlsCameraDefinitionHandle>(character)
                    .unwrap()
                    .0
                    .clone();
                app.world_mut()
                    .resource_mut::<Assets<AlsCameraDefinition>>()
                    .get_mut(&handle)
                    .unwrap()
                    .settings
                    .ignore_time_dilation = ignore;
                app.world_mut().entity_mut(character).insert(
                    crate::als_camera::AlsCameraCustomTimeDilation(owner_dilation),
                );
                app.world_mut()
                    .resource_mut::<Time<Virtual>>()
                    .set_relative_speed(0.2);
                app.update();
                let entity = app
                    .world()
                    .get::<AnimatedSceneInstance>(component)
                    .unwrap()
                    .player_entity();
                let before_running = app
                    .world()
                    .get::<AnimationGraphPlayer>(entity)
                    .unwrap()
                    .elapsed();
                let running_frame = app
                    .world()
                    .get::<AnimationGraphPlayer>(entity)
                    .unwrap()
                    .get_context_arena()
                    .unwrap()
                    .frame_index();
                app.world_mut()
                    .entity_mut(character)
                    .insert(crate::als::input_request::AlsFirstPersonView);
                app.world_mut()
                    .resource_mut::<Time<Virtual>>()
                    .set_relative_speed(0.5);
                app.update();
                let delta = if ignore {
                    app.world().resource::<Time<Real>>().delta_secs()
                } else {
                    app.world().resource::<Time<Virtual>>().delta_secs() * owner_dilation
                };
                let player = app.world().get::<AnimationGraphPlayer>(entity).unwrap();
                assert!(
                    (player.elapsed() - before_running - delta).abs() < 1e-6,
                    "Running source camera clock: ignore={ignore} owner={owner_dilation}"
                );
                assert_eq!(
                    player.get_context_arena().unwrap().frame_index(),
                    running_frame + 1
                );
                let elapsed = player.elapsed();
                let frame = player.get_context_arena().unwrap().frame_index();
                let pose = player
                    .get_default_output_pose()
                    .unwrap()
                    .bones
                    .iter()
                    .map(|bone| bone.to_transform())
                    .collect::<Vec<_>>();
                let curves = app
                    .world()
                    .get::<AlsCameraCurveValues>(component)
                    .unwrap()
                    .values;
                let view = *app.world().get::<AlsCameraViewInfo>(component).unwrap();
                let runtime = *app.world().get::<AlsCameraRuntime>(component).unwrap();
                assert!(view.valid);
                app.world_mut().resource_mut::<Time<Virtual>>().pause();
                app.world_mut()
                    .get_mut::<crate::als::motion::AlsViewYaw>(character)
                    .unwrap()
                    .0 += 0.8;
                {
                    let mut sockets = app
                        .world_mut()
                        .get_mut::<crate::als_camera::AlsCameraSocketLocations>(character)
                        .unwrap();
                    sockets.root += Vec3::X * 0.3;
                    sockets.head += Vec3::X * 0.3;
                    sockets.first_person += Vec3::X * 0.3;
                }
                for _ in 0..3 {
                    app.update();
                    let player = app.world().get::<AnimationGraphPlayer>(entity).unwrap();
                    assert_eq!(
                        player.get_context_arena().unwrap().frame_index(),
                        frame,
                        "Paused source camera graph tick: ignore={ignore} owner={owner_dilation}"
                    );
                    assert_eq!(player.elapsed(), elapsed);
                    assert_eq!(
                        player
                            .get_default_output_pose()
                            .unwrap()
                            .bones
                            .iter()
                            .map(|bone| bone.to_transform())
                            .collect::<Vec<_>>(),
                        pose
                    );
                    assert_eq!(
                        app.world()
                            .get::<AlsCameraCurveValues>(component)
                            .unwrap()
                            .values,
                        curves
                    );
                    let current = app.world().get::<AlsCameraViewInfo>(component).unwrap();
                    assert!(
                        current.valid
                            && current.location == view.location
                            && current.rotation == view.rotation
                            && current.field_of_view_deg == view.field_of_view_deg,
                        "Paused source camera view tick: ignore={ignore} owner={owner_dilation} before={view:?} after={current:?}"
                    );
                    let current = app.world().get::<AlsCameraRuntime>(component).unwrap();
                    assert_eq!(
                        (
                            current.previous_pivot_target,
                            current.pivot_lag_location,
                            current.camera_rotation,
                            current.trace_distance_ratio
                        ),
                        (
                            runtime.previous_pivot_target,
                            runtime.pivot_lag_location,
                            runtime.camera_rotation,
                            runtime.trace_distance_ratio
                        )
                    );
                }
                app.world_mut().resource_mut::<Time<Virtual>>().unpause();
                app.update();
                let player = app.world().get::<AnimationGraphPlayer>(entity).unwrap();
                assert_eq!(player.get_context_arena().unwrap().frame_index(), frame + 1);
                assert!(
                    (player.elapsed() - elapsed - delta).abs() < 1e-6,
                    "Resumed source camera clock: ignore={ignore} owner={owner_dilation}"
                );
                assert!(
                    app.world()
                        .get::<AlsCameraViewInfo>(component)
                        .unwrap()
                        .valid
                );
                if delta > 0.0 {
                    assert!(
                        app.world()
                            .get::<AlsCameraViewInfo>(component)
                            .unwrap()
                            .location
                            .distance(view.location)
                            > 1e-5
                    );
                    assert!(
                        app.world()
                            .get::<AlsCameraCurveValues>(component)
                            .unwrap()
                            .get(AlsCameraCurve::FirstPersonOverride)
                            >= curves[AlsCameraCurve::FirstPersonOverride.index()]
                    );
                }
            }
        }
    }

    #[test]
    fn actual_camera_scene_loads_binds_and_evaluates_once_with_source_inputs() {
        let (mut app, character, component, scene) = loaded_camera_scene();
        app.world_mut()
            .resource_mut::<Time<Virtual>>()
            .set_relative_speed(0.2);
        for _ in 0..14 {
            app.update();
        }
        let instance = app.world().get::<AnimatedSceneInstance>(component).unwrap();
        let definition_handle = app
            .world()
            .get::<AlsCameraDefinitionHandle>(character)
            .unwrap();
        let definitions = app.world().resource::<Assets<AlsCameraDefinition>>();
        let definition = definitions.get(&definition_handle.0).unwrap();
        let scenes = app
            .world()
            .resource::<Assets<bevy_animation_graph::core::animated_scene::AnimatedScene>>();
        let scene_asset = scenes.get(&scene).unwrap();
        assert_eq!(
            scene_asset.source.path().unwrap().to_string(),
            definition.mesh_scene
        );
        assert_eq!(
            scene_asset.skeleton.path().unwrap().to_string(),
            definition.skeleton
        );
        let graphs = app
            .world()
            .resource::<Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>>();
        let wrapper = graphs.get(&scene_asset.animation_graph).unwrap();
        let generated = wrapper
            .nodes
            .values()
            .find_map(|node| node.try_inner_downcast_ref::<crate::extensions::LayeredGraphNode>())
            .unwrap();
        assert_eq!(
            generated.graph.path().unwrap().to_string(),
            definition.graph
        );
        assert!(
            app.world()
                .get::<crate::als_camera::AlsCameraRig>(component)
                .is_some(),
            "Source camera must compute a view without a rendered Camera3d"
        );
        assert!(
            app.world()
                .get::<crate::als_camera::AlsCameraViewInfo>(component)
                .unwrap()
                .valid
        );
        let player_entity = instance.player_entity();
        let player = app
            .world()
            .get::<AnimationGraphPlayer>(player_entity)
            .unwrap();
        assert_eq!(player.get_default_output_pose().unwrap().bones.len(), 2);
        let frame = player.get_context_arena().unwrap().frame_index();
        let right = app
            .world()
            .get::<AlsCameraCurveValues>(component)
            .unwrap()
            .values[0];
        assert!(right > 0.1 && right < 1.0, "right offset {right}");
        assert!(
            (app.world()
                .get::<AlsCameraAnimationFrame>(component)
                .unwrap()
                .delta_seconds
                - 0.1)
                .abs()
                < 1e-6
        );

        app.update();
        assert_eq!(
            app.world()
                .get::<AnimationGraphPlayer>(player_entity)
                .unwrap()
                .get_context_arena()
                .unwrap()
                .frame_index(),
            frame + 1
        );
        app.world_mut()
            .entity_mut(character)
            .insert(crate::als::input_request::AlsCameraLeftShoulder);
        for _ in 0..8 {
            app.update();
        }
        let left = app
            .world()
            .get::<AlsCameraCurveValues>(component)
            .unwrap()
            .values[0];
        assert!(
            (right + left).abs() < 1e-5,
            "shoulder offsets {right} / {left}"
        );
        assert!(
            (app.world().get::<Transform>(component).unwrap().rotation * Vec3::X - Vec3::Z)
                .length()
                < 1e-6
        );
        app.world_mut().entity_mut(character).insert((
            crate::als::input_request::AlsFirstPersonView,
            AlsLastFreeRotationMode(RotationMode::VelocityDirection),
        ));
        for _ in 0..10 {
            app.update();
        }
        let values = app.world().get::<AlsCameraCurveValues>(component).unwrap();
        assert!(
            (values.get(crate::als_camera::AlsCameraCurve::FirstPersonOverride) - 1.0).abs() < 1e-6
        );
        assert!(
            values
                .get(crate::als_camera::AlsCameraCurve::CameraOffsetX)
                .abs()
                < 1e-5,
            "first-person curves: {:?}",
            values.values
        );
        assert!((values.get(crate::als_camera::AlsCameraCurve::CameraOffsetZ) - 3.25).abs() < 1e-5);
        assert_eq!(
            *app.world().get::<RotationMode>(character).unwrap(),
            RotationMode::ViewDirection
        );
        assert!(
            (app.world()
                .get::<AlsCameraAnimationFrame>(component)
                .unwrap()
                .delta_seconds
                - 0.1)
                .abs()
                < 1e-6
        );
        app.world_mut()
            .entity_mut(character)
            .remove::<crate::als::input_request::AlsFirstPersonView>();
        for _ in 0..10 {
            app.update();
        }
        let values = app.world().get::<AlsCameraCurveValues>(component).unwrap();
        assert!((values.get(crate::als_camera::AlsCameraCurve::CameraOffsetX) - left).abs() < 1e-5);
        let view = app
            .world_mut()
            .spawn((
                Camera3d::default(),
                Transform::IDENTITY,
                crate::als_camera::AlsCameraFieldOfView::default(),
                crate::als_camera::AlsCameraViewSource(component),
                AlsCharacterCamera { character },
            ))
            .id();
        app.world_mut()
            .entity_mut(component)
            .insert(crate::als_camera::AlsCameraFovOverride(107.0));
        app.update();
        let cached = *app
            .world()
            .get::<crate::als_camera::AlsCameraViewInfo>(component)
            .unwrap();
        assert_eq!(cached.field_of_view_deg, 107.0);
        assert!(
            app.world()
                .get::<Transform>(view)
                .unwrap()
                .translation
                .distance(cached.location)
                < 1e-6
        );
        let stopped_frame = app
            .world()
            .get::<AnimationGraphPlayer>(player_entity)
            .unwrap()
            .get_context_arena()
            .unwrap()
            .frame_index();
        app.world_mut()
            .get_mut::<AlsSourceCameraComponent>(component)
            .unwrap()
            .deactivate();
        let shift = Vec3::new(0.2, 0.1, 0.3);
        app.world_mut()
            .get_mut::<Transform>(character)
            .unwrap()
            .translation += shift;
        {
            let mut sockets = app
                .world_mut()
                .get_mut::<crate::als_camera::AlsCameraSocketLocations>(character)
                .unwrap();
            sockets.root += shift;
            sockets.head += shift;
            sockets.first_person += shift;
            sockets.trace_shoulder_left += shift;
            sockets.trace_shoulder_right += shift;
        }
        for _ in 0..2 {
            app.update();
        }
        assert_eq!(
            app.world()
                .get::<AnimationGraphPlayer>(player_entity)
                .unwrap()
                .get_context_arena()
                .unwrap()
                .frame_index(),
            stopped_frame
        );
        assert_eq!(
            app.world()
                .get::<crate::als_camera::AlsCameraViewInfo>(component)
                .unwrap()
                .location,
            cached.location
        );
        assert_eq!(
            app.world()
                .get::<crate::als_camera::AlsCameraFieldOfView>(view)
                .unwrap()
                .0,
            90.0
        );
        assert!(
            app.world()
                .get::<Transform>(view)
                .unwrap()
                .translation
                .distance(shift + Vec3::Y * 0.64)
                < 1e-6
        );
        app.world_mut()
            .entity_mut(character)
            .insert(Stance::Crouching);
        app.update();
        assert!(
            app.world()
                .get::<Transform>(view)
                .unwrap()
                .translation
                .distance(shift + Vec3::Y * 0.44799999)
                < 1e-6
        );
        app.world_mut()
            .entity_mut(character)
            .insert(Stance::Standing);
        app.world_mut()
            .get_mut::<AlsSourceCameraComponent>(component)
            .unwrap()
            .activate(false);
        app.update();
        assert!(
            app.world()
                .get::<crate::als_camera::AlsCameraViewInfo>(component)
                .unwrap()
                .location
                .distance(cached.location + shift)
                < 1e-5
        );
        assert_eq!(
            app.world()
                .get::<crate::als_camera::AlsCameraFieldOfView>(view)
                .unwrap()
                .0,
            107.0
        );
        let graph = match app
            .world()
            .get::<AnimationGraphPlayer>(player_entity)
            .unwrap()
            .get_animation_source()
        {
            bevy_animation_graph::core::animation_graph_player::AnimationSource::Graph(graph) => {
                graph.clone()
            }
            _ => panic!("Camera must use its generated graph"),
        };
        app.world_mut()
            .get_mut::<AnimationGraphPlayer>(player_entity)
            .unwrap()
            .start(graph);
        app.world_mut()
            .get_mut::<AlsSourceCameraComponent>(component)
            .unwrap()
            .activate(true);
        app.update();
        assert_eq!(
            app.world()
                .get::<AnimationGraphPlayer>(player_entity)
                .unwrap()
                .get_context_arena()
                .unwrap()
                .frame_index(),
            1
        );
        assert!(
            app.world()
                .get::<AlsSourceCameraComponent>(component)
                .unwrap()
                .is_active()
        );
        assert!(
            app.world()
                .get::<Transform>(component)
                .unwrap()
                .translation
                .length()
                < 1e-6
        );
        app.world_mut().entity_mut(character).despawn();
        app.update();
        assert!(app.world().get_entity(component).is_err());
        assert!(app.world().get_entity(player_entity).is_err());
        assert!(app.world().get_entity(view).is_err());
    }
}

fn remove_orphaned_views(
    mut commands: Commands,
    characters: Query<()>,
    sources: Query<&AlsCharacterCamera, With<AlsSourceCameraComponent>>,
    mut assemblies: Query<&mut crate::character_assembly::AlsCharacterAssembly>,
    views: Query<
        (
            Entity,
            &AlsCharacterCamera,
            &crate::als_camera::AlsCameraViewSource,
        ),
        With<Camera3d>,
    >,
) {
    for (entity, owner, source) in &views {
        if characters.get(owner.character).is_err()
            || !sources
                .get(source.0)
                .is_ok_and(|source_owner| source_owner.character == owner.character)
        {
            if let Ok(mut assembly) = assemblies.get_mut(owner.character)
                && assembly.camera == Some(entity)
            {
                assembly.camera = None;
            }
            commands.entity(entity).despawn();
        }
    }
}

fn publish_player_view_yaw(
    mut commands: Commands,
    mut characters: Query<
        (
            Entity,
            Option<&mut crate::als::view::AlsPlayerViewYaw>,
            Option<&crate::character_assembly::AlsCharacterAssembly>,
            Has<crate::character_assembly::AlsPlayerCharacter>,
            Option<&mut crate::als::view::AlsPlayerViewReadiness>,
        ),
        With<crate::als::character::AlsCharacter>,
    >,
    views: Query<
        (
            &Camera,
            Ref<GlobalTransform>,
            &AlsCharacterCamera,
            &crate::als_camera::AlsCameraViewSource,
        ),
        With<Camera3d>,
    >,
    sources: Query<&AlsCharacterCamera, With<AlsSourceCameraComponent>>,
) {
    for (character, current, assembly, player, readiness) in &mut characters {
        let view = views.iter().find_map(|(camera, transform, owner, source)| {
            (owner.character == character && camera.is_active && !transform.is_added())
                .then(|| transform.rotation().to_euler(EulerRot::YXZ).0)
                .filter(|yaw| yaw.is_finite())
                .map(|yaw| (yaw, source.0))
        });
        let manager = assembly
            .filter(|_| player)
            .and_then(|assembly| assembly.camera)
            .and_then(|camera| views.get(camera).ok())
            .and_then(|(camera, _, owner, source)| {
                (owner.character == character
                    && sources
                        .get(source.0)
                        .is_ok_and(|owner| owner.character == character))
                .then_some((source.0, camera.is_active))
            });
        match (readiness, manager) {
            (Some(mut readiness), Some((source, active))) => {
                let published = view.is_some_and(|(_, view_source)| view_source == source);
                if readiness.source != source {
                    if active {
                        *readiness = crate::als::view::AlsPlayerViewReadiness {
                            source,
                            has_published: published,
                        };
                    } else {
                        commands
                            .entity(character)
                            .remove::<crate::als::view::AlsPlayerViewReadiness>();
                    }
                } else if !active && !readiness.has_published {
                    commands
                        .entity(character)
                        .remove::<crate::als::view::AlsPlayerViewReadiness>();
                } else if published && !readiness.has_published {
                    readiness.has_published = true;
                }
            }
            (None, Some((source, true))) => {
                commands
                    .entity(character)
                    .insert(crate::als::view::AlsPlayerViewReadiness {
                        source,
                        has_published: view.is_some_and(|(_, view_source)| view_source == source),
                    });
            }
            (None, Some((_, false))) => {}
            (Some(_), None) => {
                commands
                    .entity(character)
                    .remove::<crate::als::view::AlsPlayerViewReadiness>();
            }
            (None, None) => {}
        }
        match (current, view.map(|(yaw, _)| yaw)) {
            (Some(mut current), Some(yaw)) => current.0 = yaw,
            (None, Some(yaw)) => {
                commands
                    .entity(character)
                    .insert(crate::als::view::AlsPlayerViewYaw(yaw));
            }
            (Some(_), None) => {
                commands
                    .entity(character)
                    .remove::<crate::als::view::AlsPlayerViewYaw>();
            }
            (None, None) => {}
        }
    }
}
