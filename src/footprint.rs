use bevy::prelude::*;
use serde::Deserialize;
use std::{collections::HashMap, sync::LazyLock};

#[derive(Deserialize)]
pub struct FootprintDefinition {
    pub source_settings: String,
    pub trace_channel: String,
    pub trace_distance: f32,
    pub spawn_angle_cos: f32,
    pub foot_y_axes: [Vec3; 2],
    pub foot_up_axes: [Vec3; 2],
    pub notify_defaults: HashMap<String, serde_json::Value>,
    pub effects: Vec<FootprintEffect>,
    pub material: FootprintMaterial,
}

#[derive(Deserialize)]
pub struct FootprintEffect {
    pub surface: String,
    pub attached: bool,
    pub size: Vec3,
    pub location_offset: Vec3,
    pub rotation_offsets: [Quat; 2],
    pub duration: f32,
    pub fade_out_duration: f32,
}

#[derive(Deserialize)]
pub struct FootprintMaterial {
    pub base_color: f32,
    pub opacity: f32,
    pub roughness_scale: f32,
    pub metallic: f32,
    pub specular: f32,
    pub texture: String,
}

pub static FOOTPRINT: LazyLock<FootprintDefinition> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../assets/effects/footprint.runtime.json"))
        .expect("Generated footprint definition must be valid")
});

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{asset::RenderAssetUsages, image::{CompressedImageFormats, ImageSampler, ImageType}};

    #[test]
    fn footprint_generated_settings_and_images_preserve_source_channels() {
        assert_eq!(FOOTPRINT.trace_distance, 0.5);
        assert_eq!(FOOTPRINT.effects[0].location_offset, Vec3::new(0.0, -0.0175, -0.1));
        assert_eq!(FOOTPRINT.notify_defaults["bSpawnDecal"].as_bool(), Some(true));
        let decode = |bytes| Image::from_buffer(bytes, ImageType::Extension("tga"),
            CompressedImageFormats::NONE, false, ImageSampler::default(), RenderAssetUsages::all()).unwrap();
        let source = decode(include_bytes!("../assets/textures/footprint/T_Als_Footprint_M.tga"));
        assert_eq!(source.size(), UVec2::splat(512));
        let data = source.data.unwrap();
        assert_eq!(&data[..4], [0, 0, 255, 0]);
        assert_eq!(&data[(256 * 512 + 256) * 4..][..4], [0, 0, 255, 255]);
        assert!(data.chunks_exact(4).any(|pixel| pixel[1] == 255 && pixel[3] == 255));
        for rotation in FOOTPRINT.effects[0].rotation_offsets {
            assert!((rotation.length_squared() - 1.0).abs() < 1e-6);
        }
    }
}
