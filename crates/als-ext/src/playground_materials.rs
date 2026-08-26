use bevy::asset::AssetServer;
use bevy::asset::RenderAssetUsages;
use bevy::image::{
    CompressedImageFormats, Image, ImageSampler, ImageSamplerDescriptor, ImageType,
    TextureFormatPixelInfo,
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

fn generate_mip_chain(img: &mut Image) {
    let width = img.width();
    let height = img.height();
    if width == 1 || height == 1 {
        return;
    }
    let format_size = img.texture_descriptor.format.pixel_size().unwrap_or(4);
    let mut levels: Vec<Vec<u8>> = Vec::new();
    let mut cur = img.data.clone().expect("image data");
    let (mut w, mut h) = (width as usize, height as usize);
    loop {
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let mut next = vec![0u8; nw * nh * format_size];
        for y in 0..nh {
            for x in 0..nw {
                for c in 0..format_size {
                    let mut sum = 0u32;
                    for dy in 0..2 {
                        for dx in 0..2 {
                            let sx = (x * 2 + dx).min(w - 1);
                            let sy = (y * 2 + dy).min(h - 1);
                            let idx = (sy * w + sx) * format_size + c;
                            sum += cur[idx] as u32;
                        }
                    }
                    next[(y * nw + x) * format_size + c] = (sum / 4) as u8;
                }
            }
        }
        cur = next.clone();
        levels.push(next);
        w = nw;
        h = nh;
        if w == 1 && h == 1 {
            break;
        }
    }

    let mut total = img.data.as_ref().map(|d| d.len()).unwrap_or(0);
    for l in &levels {
        total += l.len();
    }
    let mut all = Vec::with_capacity(total);
    all.extend_from_slice(img.data.as_deref().unwrap_or(&[]));
    for l in &levels {
        all.extend_from_slice(l);
    }
    img.data = Some(all);
    img.texture_descriptor.mip_level_count = levels.len() as u32 + 1;
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
    generate_mip_chain(&mut img);
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
