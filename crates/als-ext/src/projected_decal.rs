use bevy::{
    pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline},
    prelude::*,
    render::render_resource::{AsBindGroup, CompareFunction, Face, RenderPipelineDescriptor, SpecializedMeshPipelineError},
    mesh::MeshVertexBufferLayoutRef,
    shader::ShaderRef,
};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct DecalPipelines {
    pub decal_ready: bool,
    pub receiver_ready: bool,
    pub errors: Vec<String>,
}

#[derive(Resource, Clone, Default)]
pub struct DecalRenderStatus(pub Arc<Mutex<DecalPipelines>>);

pub struct PackedDecalPlugin;

impl Plugin for PackedDecalPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((bevy::pbr::MaterialPlugin::<PackedDecalMaterial>::default(),
            bevy::pbr::MaterialPlugin::<NoDecalMaterial>::default()));
        let status = DecalRenderStatus::default();
        app.insert_resource(status.clone());
        if let Some(render) = app.get_sub_app_mut(bevy::render::RenderApp) {
            render.insert_resource(status).add_systems(bevy::render::Render, capture_decal_pipelines.in_set(bevy::render::RenderSystems::Cleanup));
        }
        if let Some(directory) = std::env::var_os("ALS_ANIMATION_VISUAL_EVIDENCE_DIR").map(std::path::PathBuf::from) {
            app.add_systems(Last, move |mut exit: MessageReader<AppExit>, status: Res<DecalRenderStatus>| {
                if exit.read().next().is_none() { return; }
                std::fs::write(directory.join("decal_render.json"), serde_json::to_vec(&*status.0.lock().unwrap()).unwrap())
                    .expect("Decal render report must be writable");
            });
        }
    }
}

fn capture_decal_pipelines(cache: Res<bevy::render::render_resource::PipelineCache>, status: Res<DecalRenderStatus>) {
    use bevy::render::render_resource::{CachedPipelineState, PipelineDescriptor};
    let mut current = DecalPipelines::default();
    for pipeline in cache.pipelines() {
        let PipelineDescriptor::RenderPipelineDescriptor(descriptor) = &pipeline.descriptor else { continue; };
        let label = descriptor.label.as_deref().unwrap_or_default();
        if label != "als_packed_decal" && !label.starts_with("als_no_decal_receiver") { continue; }
        match &pipeline.state {
            CachedPipelineState::Ok(_) => {
                current.decal_ready |= label == "als_packed_decal";
                current.receiver_ready |= label == "als_no_decal_receiver_prepass_pipeline";
            }
            CachedPipelineState::Err(error) => current.errors.push(format!("{label}: {error}")),
            _ => {}
        }
    }
    *status.0.lock().unwrap() = current;
}

pub type PackedDecalMaterial = ExtendedMaterial<StandardMaterial, PackedDecalExtension>;

pub type NoDecalMaterial = ExtendedMaterial<StandardMaterial, NoDecalExtension>;

#[derive(Component)]
pub struct NoDecalReceiver;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone, Default)]
pub struct NoDecalExtension {}

impl MaterialExtension for NoDecalExtension {
    fn prepass_fragment_shader() -> ShaderRef {
        "shaders/no_decal_receiver.wgsl".into()
    }

    fn specialize(_pipeline: &MaterialExtensionPipeline, descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef, _key: MaterialExtensionKey<Self>) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.label = Some(format!("als_no_decal_receiver_{}", descriptor.label.as_deref().unwrap_or_default()).into());
        Ok(())
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct PackedDecalExtension {
    #[uniform(100)]
    pub roughness_scale: f32,
    #[texture(101)]
    #[sampler(102)]
    pub texture: Handle<Image>,
}

impl MaterialExtension for PackedDecalExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/packed_decal.wgsl".into()
    }

    fn alpha_mode() -> Option<AlphaMode> {
        Some(AlphaMode::Blend)
    }

    fn enable_prepass() -> bool { false }

    fn enable_shadows() -> bool { false }

    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            depth.depth_compare = Some(CompareFunction::Always);
            depth.depth_write_enabled = Some(false);
        }
        descriptor.primitive.cull_mode = Some(Face::Front);
        descriptor.label = Some("als_packed_decal".into());
        Ok(())
    }
}
