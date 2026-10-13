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
            maximum_ev: f32::NEG_INFINITY, last_ev: 0.0, physical_exposure: 0.0, invalid_samples: 0 })
            .add_systems(Last, save_lighting_evidence);
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

fn save_lighting_evidence(mut exits: MessageReader<AppExit>, samples: Res<LightingSamples>, diagnostics: Option<Res<bevy::diagnostic::DiagnosticsStore>>) {
    if exits.read().next().is_none() { return; }
    let passed = exposure_passed(samples.samples, samples.minimum_ev, samples.maximum_ev, samples.physical_exposure, samples.invalid_samples);
    let rendered_passes: Vec<_> = diagnostics.iter().flat_map(|store| store.iter())
        .filter(|diagnostic| diagnostic.path().as_str().ends_with("/elapsed_gpu")
            && diagnostic.value().is_some_and(|value| value.is_finite() && value > 0.0))
        .map(|diagnostic| diagnostic.path().as_str()).collect();
    let report = serde_json::json!({"format":"playground_lighting_evidence_v1", "pass":passed,
        "samples":samples.samples,"minimum_ev":samples.minimum_ev,"maximum_ev":samples.maximum_ev,
        "rendered_passes":rendered_passes,"last_ev":samples.last_ev,"physical_exposure":samples.physical_exposure,"invalid_samples":samples.invalid_samples,"images":[]});
    std::fs::write(&samples.path, serde_json::to_vec_pretty(&report).unwrap()).expect("Lighting evidence must be saved");
    info!("[LIGHTING][GPU-EXPOSURE] samples={} min={} max={} last={} physical={} pass={passed}",
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
    fn lighting_saves_the_final_sample_on_post_update_exit() {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("als_lighting_{}_{stamp}.json", std::process::id()));
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).init_resource::<Assets<ShaderBuffer>>();
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
        assert_eq!(report["pass"], true);
        assert_eq!(report["last_ev"], 2.0);
        std::fs::remove_file(path).unwrap();
    }
}
