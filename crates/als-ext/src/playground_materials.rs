use bevy::asset::AssetServer;
use bevy::asset::RenderAssetUsages;
use bevy::image::{
        CompressedImageFormats, Image, ImageSampler, ImageSamplerDescriptor, ImageType,
    };
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;

pub struct PlaygroundMaterialsPlugin;

impl Plugin for PlaygroundMaterialsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlaygroundMaterialLibrary>()
            .add_systems(PreStartup, load_playground_textures);
    }
}

#[derive(Resource, Default, Clone)]
pub struct PlaygroundMaterialLibrary {
    pub tiles_base_color: Option<Handle<Image>>,
    pub tiles_normal: Option<Handle<Image>>,
}

fn repeating_sampler_descriptor() -> ImageSamplerDescriptor {
    ImageSamplerDescriptor {
        address_mode_u: bevy::image::ImageAddressMode::Repeat,
        address_mode_v: bevy::image::ImageAddressMode::Repeat,
        ..Default::default()
    }
}

fn decode_png(server: &AssetServer, bytes: &[u8], is_normal: bool) -> Option<Handle<Image>> {
    let mut img = Image::from_buffer(
        bytes,
        ImageType::Extension("png".into()),
        CompressedImageFormats::NONE,
        !is_normal,
        ImageSampler::Descriptor(repeating_sampler_descriptor()),
        RenderAssetUsages::default(),
    )
    .ok()?;
    if is_normal && img.texture_descriptor.format == TextureFormat::Rgba8UnormSrgb {
        img.texture_descriptor.format = TextureFormat::Rgba8Unorm;
    }
    Some(server.add(img))
}

fn load_playground_textures(mut commands: Commands, server: Res<AssetServer>) {
    let base = std::fs::read("assets/textures/playground/T_Als_Tiles_M.png");
    let normal = std::fs::read("assets/textures/playground/T_Als_Tiles_N.png");

    let library = PlaygroundMaterialLibrary {
        tiles_base_color: base.ok().and_then(|b| decode_png(&server, &b, false)),
        tiles_normal: normal.ok().and_then(|b| decode_png(&server, &b, true)),
    };
    info!(
        "[PGMAT] textures loaded base={} normal={}",
        library.tiles_base_color.is_some(),
        library.tiles_normal.is_some()
    );
    commands.insert_resource(library);
}
