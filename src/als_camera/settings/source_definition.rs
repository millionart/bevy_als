use bevy::{
    asset::{AssetLoader, io::Reader},
    prelude::*,
};
use serde::{Deserialize, Serialize};

use crate::als_camera::{AlsCameraParams, CameraLoaderError, load_ron};

#[derive(Asset, Reflect, Serialize, Deserialize, Clone, Debug)]
pub struct AlsCameraDefinition {
    pub view_aspect_ratio: f32,
    pub settings: AlsCameraParams,
    pub first_person_socket: String,
    pub pivot_sockets: [String; 2],
    pub shoulder_sockets: [String; 2],
    pub sockets: Vec<CameraSocket>,
    pub standing_eye_height: f32,
    pub crouching_eye_height: f32,
    pub graph: String,
    pub skeleton: String,
    pub mesh_scene: String,
    pub source_material: String,
    pub attachment_translation: Vec3,
    pub attachment_rotation: Quat,
    pub attachment_scale: Vec3,
    pub auto_activate: bool,
    pub hidden_in_game: bool,
    pub right_shoulder: bool,
    pub override_field_of_view: bool,
    pub field_of_view_override_deg: f32,
    pub post_process_weight: f32,
    pub collision_enabled: bool,
    pub simulate_physics: bool,
    pub physics_body: CameraPhysicsBody,
    pub sprint_shake: CameraShakeDefinition,
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug)]
pub struct CameraSocket {
    pub name: String,
    pub bone: String,
    pub location: Vec3,
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug)]
pub struct CameraPhysicsBody {
    pub bone: String,
    pub center: Vec3,
    pub rotation: Quat,
    pub half_extents: Vec3,
    pub mass_kg: f32,
    pub default_collision: String,
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug)]
pub struct CameraShakeDefinition {
    pub single_instance: bool,
    pub duration: f32,
    pub blend_in: f32,
    pub blend_out: f32,
    pub location: [CameraWave; 3],
    pub rotation_yxz: [CameraWave; 3],
    pub field_of_view: CameraWave,
}

#[derive(Reflect, Serialize, Deserialize, Clone, Copy, Debug)]
pub struct CameraWave {
    pub amplitude: f32,
    pub frequency_hz: f32,
    pub random_start: bool,
}

#[derive(Default, TypePath)]
pub struct AlsCameraDefinitionLoader;

impl AssetLoader for AlsCameraDefinitionLoader {
    type Asset = AlsCameraDefinition;
    type Settings = ();
    type Error = CameraLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _context: &mut bevy::asset::LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        load_ron(reader).await
    }

    fn extensions(&self) -> &[&str] {
        &["camera_component.ron"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_camera_definition_preserves_asset_and_instance_boundaries() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin {
                file_path: path.to_string_lossy().into_owned(),
                ..default()
            },
        ))
        .init_asset::<AlsCameraDefinition>()
        .init_asset_loader::<AlsCameraDefinitionLoader>();
        app.finish();
        app.cleanup();
        let handle = app
            .world()
            .resource::<AssetServer>()
            .load::<AlsCameraDefinition>("ALSCamera/Data/als_camera.camera_component.ron");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        loop {
            app.update();
            match app
                .world()
                .resource::<AssetServer>()
                .load_state(handle.id())
            {
                bevy::asset::LoadState::Loaded => break,
                bevy::asset::LoadState::Failed(error) => {
                    panic!("Camera definition failed to load: {error}")
                }
                state => assert!(
                    std::time::Instant::now() < deadline,
                    "Camera definition made no progress: {state:?}"
                ),
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let definitions = app.world().resource::<Assets<AlsCameraDefinition>>();
        let definition = definitions.get(&handle).unwrap();
        assert!(
            definition.settings.ignore_time_dilation
                && definition.auto_activate
                && definition.hidden_in_game
        );
        assert!(!definition.collision_enabled && !definition.simulate_physics);
        assert_eq!(definition.physics_body.default_collision, "QueryAndPhysics");
        assert_eq!(definition.physics_body.bone, "cameraboom_root");
        assert_eq!(
            definition.physics_body.half_extents,
            Vec3::new(0.3, 0.15, 0.175)
        );
        assert!((definition.attachment_rotation * Vec3::X - Vec3::Z).length() < 1e-6);
        assert_eq!(definition.sprint_shake.location[2].amplitude, -0.01);
        assert_eq!(definition.sprint_shake.location[0].frequency_hz, 1.0);
        assert!(!definition.sprint_shake.single_instance);
        assert!(
            (definition.sprint_shake.rotation_yxz[0].amplitude + 0.1_f32.to_radians()).abs() < 1e-8
        );
        assert_eq!(definition.sprint_shake.field_of_view.amplitude, 0.0);
        let socket = definition
            .sockets
            .iter()
            .find(|socket| socket.name == definition.first_person_socket)
            .unwrap();
        assert_eq!(socket.bone, "head");
        assert_eq!(socket.location, Vec3::new(0.04, 0.0, 0.14));
        assert_eq!(definition.post_process_weight, 0.0);
    }
}
