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
pub struct AlsSourceCameraComponent {
    pub initialized: bool,
    pub active: bool,
    previous_global_time_dilation: f32,
}

impl Default for AlsSourceCameraComponent {
    fn default() -> Self {
        Self {
            initialized: false,
            active: false,
            previous_global_time_dilation: 1.0,
        }
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
    mut components: Query<(
        &AlsCharacterCamera,
        &mut AlsSourceCameraComponent,
        &mut Transform,
        &mut Visibility,
        Option<&AnimatedSceneInstance>,
    )>,
) {
    for (owner, mut component, mut transform, mut visibility, scene) in &mut components {
        if component.initialized {
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
        let Some(scene) = scene else {
            continue;
        };
        commands
            .entity(scene.player_entity())
            .insert(als_ext::post_physics_animation::ManualAnimationPlayer);
        component.active = definition.auto_activate;
        component.initialized = true;
    }
}

fn update_camera_animation(
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
        frame.active = component.initialized && component.active;
        if !frame.active {
            continue;
        }
        let Some(definition) = definitions.get(&handle.0) else {
            continue;
        };
        let Ok(mut player) = players.get_mut(scene.player_entity()) else {
            continue;
        };
        let rotation =
            als_camera::camera_profile_rotation_mode(*rotation, first_person, desired.copied());
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
            assert!(std::time::Instant::now() < deadline,
                "Camera scene load made no progress: asset={:?}, world_root={}, instance={}", server.load_state(scene.id()),
                app.world().get::<bevy::world_serialization::WorldAssetRoot>(component).is_some(),
                app.world().get::<AnimatedSceneInstance>(component).is_some());
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        app.world_mut()
            .resource_mut::<Time<Virtual>>()
            .set_relative_speed(0.2);
        for _ in 0..14 {
            app.update();
        }
        let instance = app.world().get::<AnimatedSceneInstance>(component).unwrap();
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
    }
}

fn remove_orphaned_views(
    mut commands: Commands,
    characters: Query<(), With<als_core::character::AlsCharacter>>,
    views: Query<(Entity, &AlsCharacterCamera), With<Camera3d>>,
) {
    for (entity, owner) in &views {
        if characters.get(owner.character).is_err() {
            commands.entity(entity).despawn();
        }
    }
}
