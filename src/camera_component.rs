use crate::ab_als_root::{
    gait_tag, locomotion_action_tag, locomotion_mode_tag, rotation_mode_tag, stance_tag,
};
use als_camera::{
    AlsCameraAnimationFrame, AlsCameraCurveValues, AlsCameraDefinition, AlsCameraDefinitionHandle,
    AlsCharacterCamera, CAMERA_CURVE_NAMES,
};
use als_core::{
    aim::AlsLastFreeRotationMode,
    state::{Gait, LocomotionState, RotationMode, Stance},
};
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
#[require(als_camera::AlsCameraRig)]
pub struct AlsSourceCameraComponent {
    bound_player: Option<Entity>,
    active: bool,
    refresh_view: bool,
    previous_global_time_dilation: f32,
}

impl Default for AlsSourceCameraComponent {
    fn default() -> Self {
        Self {
            bound_player: None,
            active: false,
            refresh_view: false,
            previous_global_time_dilation: 1.0,
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
                .in_set(AlsCameraAnimationSet)
                .after(als_ext::post_physics_animation::PostPhysicsAnimationSet::Evaluate)
                .before(AnimationGraphSet::PostPhysics),
        )
        .add_systems(Update, remove_orphaned_views);
    }
}

fn initialize_components(
    mut commands: Commands,
    definitions: Res<Assets<AlsCameraDefinition>>,
    characters: Query<&AlsCameraDefinitionHandle>,
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
            if let Ok(mut frame) = frames.get_mut(owner.character) {
                frame.active = false;
            }
            continue;
        };
        if component.bound_player == Some(scene.player_entity()) {
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
        commands
            .entity(scene.player_entity())
            .insert(als_ext::post_physics_animation::ManualAnimationPlayer);
        if component.bound_player.is_none() {
            if definition.auto_activate {
                component.activate(false);
            }
            if !definition.right_shoulder {
                commands
                    .entity(owner.character)
                    .insert(als_core::input_request::AlsCameraLeftShoulder);
            }
            if definition.override_field_of_view {
                commands
                    .entity(entity)
                    .insert(als_camera::AlsCameraFovOverride(
                        definition.field_of_view_override_deg,
                    ));
            }
        }
        component.bound_player = Some(scene.player_entity());
    }
}

fn update_camera_animation(
    assets: Res<AssetServer>,
    virtual_time: Res<Time<Virtual>>,
    real_time: Res<Time<Real>>,
    definitions: Res<Assets<AlsCameraDefinition>>,
    mut components: Query<(
        &AlsCharacterCamera,
        &mut AlsSourceCameraComponent,
        &AnimatedSceneInstance,
    )>,
    mut characters: Query<(
        &AlsCameraDefinitionHandle,
        &Gait,
        &Stance,
        &LocomotionState,
        &RotationMode,
        Option<&AlsLastFreeRotationMode>,
        Has<als_core::input_request::AlsFirstPersonView>,
        Has<als_core::input_request::AlsCameraLeftShoulder>,
        Option<&als_camera::AlsCameraCustomTimeDilation>,
        Option<&crate::animation_events::AlsNotifyLocomotionAction>,
        Option<&als_core::mantle::AlsMantleState>,
        Option<&als_core::roll::AlsRollState>,
        &mut AlsCameraCurveValues,
        &mut AlsCameraAnimationFrame,
    )>,
    mut players: Query<&mut AnimationGraphPlayer>,
    resources: SystemResources,
) {
    for (owner, mut component, scene) in &mut components {
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
            mut curves,
            mut frame,
        )) = characters.get_mut(owner.character)
        else {
            continue;
        };
        frame.active = component.bound_player == Some(scene.player_entity()) && component.active;
        frame.force_no_lag = false;
        if !frame.active {
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
        let rotation =
            als_camera::camera_animation_rotation_mode(*rotation, first_person, desired.copied());
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
            als_ext::set_gameplay_tag_input(scene.player_entity(), key, tag);
        }
        player.set_global_input_data("b_right_shoulder", !left_shoulder);
        let delta = als_camera::camera_delta_seconds_from_times(
            &mut component.previous_global_time_dilation,
            &definition.settings,
            virtual_time.delta_secs() * dilation.map_or(1.0, |value| value.0),
            real_time.delta_secs(),
            virtual_time.relative_speed(),
            dilation.map_or(1.0, |value| value.0),
        );
        frame.delta_seconds = delta;
        player.set_global_input_data("animation.delta_seconds", delta);
        let mut time = Time::default();
        time.advance_by(std::time::Duration::from_secs_f32(delta));
        run_animation_player(scene.player_entity(), player, &time, &resources);
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
            als_ext::animation_context::node_context_state(
                &arena.get_toplevel().node_states,
                bevy_animation_graph::core::context::node_states::StateKey::Default,
                *node,
            )
        }) {
            for (index, name) in CAMERA_CURVE_NAMES.iter().enumerate() {
                curves.values[index] = context.curves.get(*name).copied().unwrap_or(0.0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::world_serialization::WorldSerializationPlugin;
    use bevy_animation_graph::core::animated_scene::AnimatedSceneHandle;

    #[test]
    fn actual_camera_scene_loads_binds_and_evaluates_once_with_source_inputs() {
        let mut app = App::new();
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
            .load("params/als_camera.camera_component.ron");
        let scene = app
            .world()
            .resource::<AssetServer>()
            .load("animated_scenes/als_camera.static.animscn.ron");
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
                AlsCameraCurveValues::default(),
                AlsCameraAnimationFrame::default(),
                als_camera::AlsCameraCustomTimeDilation(0.5),
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
            als_camera::AlsCameraTarget,
            als_core::motion::AlsViewYaw(0.0),
            als_core::view::AlsViewPitch(0.0),
            als_camera::AlsCameraSocketLocations {
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
        app.world_mut()
            .resource_mut::<Time<Virtual>>()
            .set_relative_speed(0.2);
        for _ in 0..14 {
            app.update();
        }
        let instance = app.world().get::<AnimatedSceneInstance>(component).unwrap();
        assert!(
            app.world()
                .get::<als_camera::AlsCameraRig>(component)
                .is_some(),
            "Source camera must compute a view without a rendered Camera3d"
        );
        assert!(
            app.world()
                .get::<als_camera::AlsCameraViewInfo>(component)
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
            .get::<AlsCameraCurveValues>(character)
            .unwrap()
            .values[0];
        assert!(right > 0.1 && right < 1.0, "right offset {right}");
        assert!(
            (app.world()
                .get::<AlsCameraAnimationFrame>(character)
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
            .insert(als_core::input_request::AlsCameraLeftShoulder);
        for _ in 0..8 {
            app.update();
        }
        let left = app
            .world()
            .get::<AlsCameraCurveValues>(character)
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
            als_core::input_request::AlsFirstPersonView,
            AlsLastFreeRotationMode(RotationMode::VelocityDirection),
        ));
        for _ in 0..10 {
            app.update();
        }
        let values = app.world().get::<AlsCameraCurveValues>(character).unwrap();
        assert!((values.get(als_camera::AlsCameraCurve::FirstPersonOverride) - 1.0).abs() < 1e-6);
        assert!(values.get(als_camera::AlsCameraCurve::CameraOffsetX).abs() < 1e-5);
        assert!((values.get(als_camera::AlsCameraCurve::CameraOffsetZ) - 3.25).abs() < 1e-5);
        assert_eq!(
            *app.world().get::<RotationMode>(character).unwrap(),
            RotationMode::ViewDirection
        );
        assert!(
            (app.world()
                .get::<AlsCameraAnimationFrame>(character)
                .unwrap()
                .delta_seconds
                - 0.1)
                .abs()
                < 1e-6
        );
        app.world_mut()
            .entity_mut(character)
            .remove::<als_core::input_request::AlsFirstPersonView>();
        for _ in 0..10 {
            app.update();
        }
        let values = app.world().get::<AlsCameraCurveValues>(character).unwrap();
        assert!((values.get(als_camera::AlsCameraCurve::CameraOffsetX) - left).abs() < 1e-5);
        let view = app
            .world_mut()
            .spawn((
                Camera3d::default(),
                Transform::IDENTITY,
                als_camera::AlsCameraFieldOfView::default(),
                als_camera::AlsCameraViewSource(component),
                AlsCharacterCamera { character },
            ))
            .id();
        app.world_mut()
            .entity_mut(component)
            .insert(als_camera::AlsCameraFovOverride(107.0));
        app.update();
        let cached = *app
            .world()
            .get::<als_camera::AlsCameraViewInfo>(component)
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
                .get_mut::<als_camera::AlsCameraSocketLocations>(character)
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
                .get::<als_camera::AlsCameraViewInfo>(component)
                .unwrap()
                .location,
            cached.location
        );
        assert_eq!(
            app.world()
                .get::<als_camera::AlsCameraFieldOfView>(view)
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
                .get::<als_camera::AlsCameraViewInfo>(component)
                .unwrap()
                .location
                .distance(cached.location + shift)
                < 1e-5
        );
        assert_eq!(
            app.world()
                .get::<als_camera::AlsCameraFieldOfView>(view)
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
    views: Query<(Entity, &AlsCharacterCamera), With<Camera3d>>,
) {
    for (entity, owner) in &views {
        if characters.get(owner.character).is_err() {
            commands.entity(entity).despawn();
        }
    }
}
