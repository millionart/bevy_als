use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::gpu_readback::{Readback, ReadbackComplete};
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::{BufferUsages, ShaderType, encase};
use bevy::render::renderer::{RenderContext, ViewQuery};
use bevy::render::storage::{GpuShaderBuffer, ShaderBuffer};
use bevy::render::view::{ViewUniform, ViewUniformOffset, ViewUniforms};
use bevy::render::{RenderApp, RenderStartup, init_gpu_resource};
use bevy::post_process::auto_exposure::AutoExposure;
use bevy::core_pipeline::{schedule::Core3d, tonemapping::tonemapping};

pub struct LightingProbePlugin(pub Option<std::path::PathBuf>);

#[derive(Resource, Clone)]
struct ViewReadback(Handle<ShaderBuffer>);

#[derive(Resource)]
struct LightingSamples {
    path: std::path::PathBuf,
    samples: usize,
    minimum_ev: f32,
    maximum_ev: f32,
    last_ev: f32,
    physical_exposure: f32,
    invalid_samples: usize,
    environment_maximums: [Option<f32>; 2],
}

fn exposure_passed(samples: usize, minimum: f32, maximum: f32, physical: f32, invalid: usize) -> bool {
    samples >= 2 && invalid == 0 && minimum.is_finite() && maximum.is_finite()
        && maximum.abs().max(minimum.abs()) > 1e-5 && (physical - 1.0).abs() < 1e-5
}

impl Plugin for LightingProbePlugin {
    fn build(&self, app: &mut App) {
        let Some(path) = self.0.clone() else { return; };
        let handle = app.world_mut().resource_mut::<Assets<ShaderBuffer>>()
            .add(ShaderBuffer::from(vec![0u32; ViewUniform::min_size().get() as usize / 4]));
        let readback = ViewReadback(handle.clone());
        app.insert_resource(LightingSamples { path, samples: 0, minimum_ev: f32::INFINITY,
            maximum_ev: f32::NEG_INFINITY, last_ev: 0.0, physical_exposure: 0.0, invalid_samples: 0, environment_maximums: [None; 2] })
            .add_systems(Last, save_lighting_evidence)
            .add_systems(PostUpdate, prepare_environment_readback);
        app.world_mut().spawn(Readback::buffer(handle)).observe(
            |event: On<ReadbackComplete>, mut samples: ResMut<LightingSamples>| {
                if event.data.len() < ViewUniform::min_size().get() as usize { return; }
                let view = encase::UniformBuffer::new(event.data.as_slice()).create::<ViewUniform>()
                    .expect("Lighting readback must match the native view layout");
                if !view.color_grading.exposure.is_finite() || !view.exposure.is_finite() {
                    samples.invalid_samples += 1;
                    return;
                }
                samples.samples += 1;
                samples.minimum_ev = samples.minimum_ev.min(view.color_grading.exposure);
                samples.maximum_ev = samples.maximum_ev.max(view.color_grading.exposure);
                samples.last_ev = view.color_grading.exposure;
                samples.physical_exposure = view.exposure;
            });
        let render = app.sub_app_mut(RenderApp);
        render.insert_resource(readback)
            .add_systems(RenderStartup, configure_view_copy.after(init_gpu_resource::<ViewUniforms>))
            .add_systems(Core3d, copy_view_buffer.after(tonemapping));
    }
}

#[derive(Component)]
struct EnvironmentReadbackInstalled;

fn filtered_environment_maximum(bytes: &[u8], template: &Image) -> Option<f32> {
    let size = template.texture_descriptor.size;
    let required = size.width as usize * size.height as usize * size.depth_or_array_layers as usize * 8;
    if bytes.len() < required { return None; }
    let mut image = template.clone();
    image.data = Some(bytes[..required].to_vec());
    let mut maximum = 0.0_f32;
    for layer in 0..size.depth_or_array_layers {
        for y in 0..size.height {
            for x in 0..size.width {
                let pixel = image.get_color_at_3d(x, y, layer).ok()?.to_linear().to_f32_array();
                if pixel.iter().any(|channel| !channel.is_finite()) || pixel[3] != 1.0 { return None; }
                maximum = maximum.max(pixel[0]).max(pixel[1]).max(pixel[2]);
            }
        }
    }
    (maximum > 0.0).then_some(maximum)
}

fn prepare_environment_readback(mut commands: Commands, mut images: ResMut<Assets<Image>>,
    environments: Query<(Entity, &bevy::light::EnvironmentMapLight), (With<Camera3d>, Without<EnvironmentReadbackInstalled>)>) {
    for (entity, environment) in &environments {
        for (kind, handle) in [environment.diffuse_map.clone(), environment.specular_map.clone()].into_iter().enumerate() {
            let mut image = images.get_mut(&handle).expect("Native environment output must exist before extraction");
            image.texture_descriptor.usage |= bevy::render::render_resource::TextureUsages::COPY_SRC;
            let template = image.clone();
            commands.spawn(Readback::texture(handle)).observe(move |event: On<ReadbackComplete>,
                mut samples: ResMut<LightingSamples>, mut commands: Commands| {
                if let Some(maximum) = filtered_environment_maximum(&event.data, &template) {
                    samples.environment_maximums[kind] = Some(maximum);
                    commands.entity(event.entity).try_despawn();
                }
            });
        }
        commands.entity(entity).insert(EnvironmentReadbackInstalled);
    }
}

fn configure_view_copy(mut uniforms: ResMut<ViewUniforms>) {
    uniforms.uniforms.add_usages(BufferUsages::COPY_SRC);
}

fn copy_view_buffer(view: ViewQuery<(&ViewUniformOffset, &AutoExposure)>, uniforms: Res<ViewUniforms>,
    handle: Res<ViewReadback>, buffers: Res<RenderAssets<GpuShaderBuffer>>, mut context: RenderContext) {
    let Some(source) = uniforms.uniforms.buffer() else { return; };
    let Some(destination) = buffers.get(handle.0.id()) else { return; };
    let (offset, _) = view.into_inner();
    context.command_encoder().copy_buffer_to_buffer(source, u64::from(offset.offset),
        &destination.buffer, 0, ViewUniform::min_size().get());
}

fn save_lighting_evidence(mut exits: MessageReader<AppExit>, samples: Res<LightingSamples>, diagnostics: Option<Res<bevy::diagnostic::DiagnosticsStore>>,
    environments: Query<(&bevy::light::GeneratedEnvironmentMapLight, Option<&bevy::light::EnvironmentMapLight>), With<Camera3d>>,
    ambient: Option<Res<GlobalAmbientLight>>) {
    if exits.read().next().is_none() { return; }
    let exposure_passed = exposure_passed(samples.samples, samples.minimum_ev, samples.maximum_ev, samples.physical_exposure, samples.invalid_samples);
    let rendered_passes: Vec<_> = diagnostics.iter().flat_map(|store| store.iter())
        .filter(|diagnostic| diagnostic.path().as_str().ends_with("/elapsed_gpu")
            && diagnostic.value().is_some_and(|value| value.is_finite() && value > 0.0))
        .map(|diagnostic| diagnostic.path().as_str()).collect();
    let environment_ready_count = environments.iter().filter(|(source, filtered)|
        filtered.is_some_and(|filtered| filtered.intensity == source.intensity && filtered.rotation == source.rotation)).count();
    let bloom_rendered = rendered_passes.iter().any(|path| path.ends_with("/bloom/elapsed_gpu"));
    let environment_rendered = samples.environment_maximums.iter().all(Option::is_some);
    let ambient_brightness = ambient.map(|ambient| ambient.brightness);
    let passed = exposure_passed && bloom_rendered && environment_rendered
        && environment_ready_count == 1 && ambient_brightness == Some(0.0);
    let report = serde_json::json!({"format":"playground_lighting_evidence_v1", "pass":passed,
        "exposure_passed":exposure_passed,"bloom_rendered":bloom_rendered,"environment_rendered":environment_rendered,
        "environment_maximums":samples.environment_maximums,"environment_ready_count":environment_ready_count,"ambient_brightness":ambient_brightness,
        "samples":samples.samples,"minimum_ev":samples.minimum_ev,"maximum_ev":samples.maximum_ev,
        "rendered_passes":rendered_passes,"last_ev":samples.last_ev,"physical_exposure":samples.physical_exposure,"invalid_samples":samples.invalid_samples,"images":[]});
    std::fs::write(&samples.path, serde_json::to_vec_pretty(&report).unwrap()).expect("Lighting evidence must be saved");
    info!("[LIGHTING][VERDICT] samples={} min={} max={} last={} physical={} pass={passed}",
        samples.samples, samples.minimum_ev, samples.maximum_ev, samples.last_ev, samples.physical_exposure);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposure_readback_rejects_missing_fixed_and_non_finite_results() {
        assert!(exposure_passed(20, 0.0, 2.0, 1.0, 0));
        assert!(!exposure_passed(0, 0.0, 2.0, 1.0, 0));
        assert!(!exposure_passed(20, 0.0, 0.0, 1.0, 0));
        assert!(!exposure_passed(20, f32::NAN, 2.0, 1.0, 0));
        assert!(!exposure_passed(20, 0.0, 2.0, 0.20833333, 0));
        assert!(!exposure_passed(20, 0.0, 2.0, 1.0, 1));
    }

    #[test]
    fn environment_readback_rejects_pending_and_non_finite_pixels() {
        use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
        let size = Extent3d { width: 32, height: 32, depth_or_array_layers: 6 };
        let image = Image::new_fill(size, TextureDimension::D2, &[0, 60, 0, 0, 0, 0, 0, 60],
            TextureFormat::Rgba16Float, bevy::asset::RenderAssetUsages::all());
        assert_eq!(filtered_environment_maximum(image.data.as_ref().unwrap(), &image), Some(1.0));
        assert_eq!(filtered_environment_maximum(&[], &image), None);
        assert_eq!(filtered_environment_maximum(&vec![0; 32 * 32 * 6 * 8], &image), None);
        let mut invalid = image.data.clone().unwrap();
        invalid[..2].copy_from_slice(&[0, 126]);
        assert_eq!(filtered_environment_maximum(&invalid, &image), None);
    }

    #[test]
    fn environment_readback_is_installed_before_native_gpu_assets_once() {
        use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).init_resource::<Assets<ShaderBuffer>>().init_resource::<Assets<Image>>();
        app.insert_sub_app(RenderApp, bevy::app::SubApp::new());
        app.add_plugins(LightingProbePlugin(Some(std::env::temp_dir().join("als_environment_probe_unused.json"))));
        app.add_systems(Update, bevy::pbr::generate::generate_environment_map_light);
        let image = Image::new_fill(Extent3d { width: 128, height: 128, depth_or_array_layers: 6 },
            TextureDimension::D2, &[0; 8], TextureFormat::Rgba16Float, bevy::asset::RenderAssetUsages::all());
        let source = app.world_mut().resource_mut::<Assets<Image>>().add(image);
        let camera = app.world_mut().spawn((Camera3d::default(), bevy::light::GeneratedEnvironmentMapLight {
            environment_map: source, intensity: 2.0, ..default()
        })).id();
        app.update();
        let maps = app.world().get::<bevy::light::EnvironmentMapLight>(camera).unwrap();
        let images = app.world().resource::<Assets<Image>>();
        for handle in [&maps.diffuse_map, &maps.specular_map] {
            assert!(images.get(handle).unwrap().texture_descriptor.usage.contains(TextureUsages::COPY_SRC),
                "Readback must be configured before the native GPU output is prepared");
        }
        assert_eq!(app.world_mut().query::<&Readback>().iter(app.world()).count(), 3);
        app.update();
        assert_eq!(app.world_mut().query::<&Readback>().iter(app.world()).count(), 3,
            "Static native outputs require only one diagnostic request each");
    }

    #[test]
    fn lighting_saves_the_final_sample_on_post_update_exit() {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("als_lighting_{}_{stamp}.json", std::process::id()));
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).init_resource::<Assets<ShaderBuffer>>().init_resource::<Assets<Image>>();
        app.insert_sub_app(RenderApp, bevy::app::SubApp::new());
        app.add_plugins(LightingProbePlugin(Some(path.clone())));
        {
            let mut samples = app.world_mut().resource_mut::<LightingSamples>();
            samples.samples = 20;
            samples.minimum_ev = 0.0;
            samples.maximum_ev = 2.0;
            samples.last_ev = 2.0;
            samples.physical_exposure = 1.0;
        }
        app.add_systems(PostUpdate, |mut exits: MessageWriter<AppExit>| { exits.write(AppExit::Success); });
        app.update();
        let bytes = std::fs::read(&path).expect("Lighting evidence must include the exit frame");
        let report: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(report["pass"], false);
        assert_eq!(report["exposure_passed"], true);
        assert_eq!(report["environment_ready_count"], 0);
        assert_eq!(report["last_ev"], 2.0);
        std::fs::remove_file(path).unwrap();
    }
}
