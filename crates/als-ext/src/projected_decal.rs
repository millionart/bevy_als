use bevy::{
    pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline},
    prelude::*,
    render::render_resource::{AsBindGroup, CompareFunction, Face, RenderPipelineDescriptor, SpecializedMeshPipelineError},
    mesh::MeshVertexBufferLayoutRef,
    shader::ShaderRef,
};

pub type PackedDecalMaterial = ExtendedMaterial<StandardMaterial, PackedDecalExtension>;

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
        Ok(())
    }
}
