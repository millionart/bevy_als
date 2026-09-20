use bevy::{
    light::ClusteredDecal,
    pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline},
    prelude::*,
    render::{render_resource::{AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError}, storage::ShaderBuffer},
    mesh::MeshVertexBufferLayoutRef,
    shader::ShaderRef,
};
use std::{collections::{HashMap, HashSet}, sync::{Arc, Mutex}};

pub const PACKED_DECAL_TAG: u32 = 1 << 31;
pub const DECAL_PROBE_SAMPLES: usize = 16;
pub const DECAL_PROBE_WORDS: usize = 4 + DECAL_PROBE_SAMPLES * 28;

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct DecalPipelines {
    pub decal_ready: bool,
    pub receiver_ready: bool,
    pub clustered_supported: bool,
    pub native_decals: usize,
    pub material_response_samples: usize,
    pub material_response_max_error: f32,
    pub errors: Vec<String>,
    pub pipeline_labels: Vec<String>,
}

#[derive(Resource, Clone, Default)]
pub struct DecalRenderStatus(pub Arc<Mutex<DecalPipelines>>);

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PackedDecalPrepareSet;

#[derive(Component)]
pub struct NoDecalReceiver;

#[derive(Component, Clone, Debug)]
pub struct PhysicalSurface(pub String);

#[derive(Asset, TypePath, Clone, Debug)]
pub struct PackedDecalMaterial {
    pub color: Vec4,
    pub roughness_scale: f32,
    pub metallic: f32,
    pub specular: f32,
    pub texture: Handle<Image>,
}

#[derive(Component, Clone, Debug)]
#[require(Transform, Visibility, ClusteredDecal)]
pub struct PackedDecal(pub Handle<PackedDecalMaterial>);

#[derive(Clone, Default, ShaderType)]
pub struct PackedDecalData {
    pub local_from_world: Mat4,
    pub color: Vec4,
    pub material: Vec4,
}

#[derive(Resource, Clone, Default)]
pub struct PackedDecalBuffers {
    pub materials: Handle<ShaderBuffer>,
    pub probe: Handle<ShaderBuffer>,
}

impl PackedDecalBuffers {
    fn new(buffers: &mut Assets<ShaderBuffer>, enabled: bool) -> Self {
        let mut words = vec![0_u32; DECAL_PROBE_WORDS];
        words[0] = u32::from(enabled);
        Self { materials: buffers.add(ShaderBuffer::from(vec![PackedDecalData::default()])), probe: buffers.add(ShaderBuffer::from(words)) }
    }
}

pub type PackedDecalReceiverMaterial = ExtendedMaterial<StandardMaterial, PackedDecalReceiverExtension>;

#[derive(Component, Clone)]
pub struct StandardMaterialSource(pub Handle<StandardMaterial>);

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct PackedDecalReceiverExtension {
    #[storage(110, read_only, visibility(fragment))]
    pub decals: Handle<ShaderBuffer>,
    #[storage(111, visibility(fragment))]
    pub probe: Handle<ShaderBuffer>,
}

impl MaterialExtension for PackedDecalReceiverExtension {
    fn fragment_shader() -> ShaderRef { "shaders/packed_decal_receiver.wgsl".into() }
    fn deferred_fragment_shader() -> ShaderRef { "shaders/packed_decal_receiver.wgsl".into() }
    fn specialize(_: &MaterialExtensionPipeline, descriptor: &mut RenderPipelineDescriptor, _: &MeshVertexBufferLayoutRef, _: MaterialExtensionKey<Self>) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.label = Some(format!("als_packed_receiver_{}", descriptor.label.as_deref().unwrap_or_default()).into());
        Ok(())
    }
}

pub struct PackedDecalPlugin;

impl Plugin for PackedDecalPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bevy::pbr::MaterialPlugin::<PackedDecalReceiverMaterial>::default())
            .init_asset::<PackedDecalMaterial>();
        if !app.world().contains_resource::<Assets<ShaderBuffer>>() { app.init_asset::<ShaderBuffer>(); }
        let probe_enabled = std::env::var("ALS_MANUAL_INTEGRATION_CASE").is_ok_and(|case| case == "footprint");
        let buffers = PackedDecalBuffers::new(&mut app.world_mut().resource_mut::<Assets<ShaderBuffer>>(), probe_enabled);
        app.insert_resource(buffers)
            .add_systems(Update, prepare_receivers.in_set(PackedDecalPrepareSet))
            .add_systems(PostUpdate, prepare_decals.after(bevy::transform::TransformSystems::Propagate).in_set(PackedDecalPrepareSet));
        let status = DecalRenderStatus::default();
        app.insert_resource(status.clone());
        if probe_enabled {
            app.add_systems(Startup, |mut commands: Commands, buffers: Res<PackedDecalBuffers>| {
                commands.spawn(bevy::render::gpu_readback::Readback::buffer(buffers.probe.clone())).observe(read_material_probe);
            });
        }
        if let Some(render) = app.get_sub_app_mut(bevy::render::RenderApp) {
            render.insert_resource(status).add_systems(bevy::render::Render, capture_decal_pipelines.in_set(bevy::render::RenderSystems::Cleanup));
        }
        if let Some(directory) = std::env::var_os("ALS_ANIMATION_VISUAL_EVIDENCE_DIR").map(std::path::PathBuf::from) {
            app.add_systems(Last, move |mut exit: MessageReader<AppExit>, status: Res<DecalRenderStatus>,
                receivers: Query<Entity, (With<MeshMaterial3d<StandardMaterial>>, With<NoDecalReceiver>)>, markers: Query<Entity, With<NoDecalReceiver>>| {
                if exit.read().next().is_none() { return; }
                let mut report = serde_json::to_value(&*status.0.lock().unwrap()).unwrap();
                report["receiver_entities"] = receivers.iter().count().into();
                report["receiver_markers"] = markers.iter().count().into();
                std::fs::write(directory.join("decal_render.json"), serde_json::to_vec(&report).unwrap()).expect("Decal render report must be writable");
            });
        }
    }
}

fn material_probe_error(words: &[u32]) -> Option<(usize, f32)> {
    if words.len() != DECAL_PROBE_WORDS || words[0] != 1 || words[1] == 0 { return None; }
    let count = (words[1] as usize).min(DECAL_PROBE_SAMPLES);
    let mut error = 0.0_f32;
    for sample in words[4..].chunks_exact(28).take(count) {
        let value: Vec<_> = sample.iter().map(|word| f32::from_bits(*word)).collect();
        if value.iter().any(|value| !value.is_finite()) { return Some((count, f32::MAX)); }
        let alpha = (value[11] * value[15]).clamp(0.0, 1.0);
        let blend = |before: f32, after: f32| before * (1.0 - alpha) + after * alpha;
        let mut expected = [0.0; 8];
        for channel in 0..3 { expected[channel] = blend(value[channel], value[12 + channel]); }
        expected[3] = blend(value[3], value[9] * value[16]);
        expected[4] = blend(value[4], value[17]);
        for channel in 0..3 { expected[5 + channel] = blend(value[5 + channel].powi(2), value[18] * 0.5).sqrt(); }
        for (actual, expected) in value[20..].iter().zip(expected) { error = error.max((actual - expected).abs()); }
    }
    Some((count, error))
}

fn read_material_probe(event: On<bevy::render::gpu_readback::ReadbackComplete>, mut commands: Commands, status: Res<DecalRenderStatus>) {
    let words: Vec<u32> = event.to_shader_type();
    let Some((count, error)) = material_probe_error(&words) else { return; };
    let mut status = status.0.lock().unwrap();
    status.material_response_samples = count;
    status.material_response_max_error = error;
    if count == DECAL_PROBE_SAMPLES { commands.entity(event.entity).despawn(); }
}

fn prepare_decals(
    mut decals: Query<(Entity, &PackedDecal, &GlobalTransform, &mut ClusteredDecal)>,
    materials: Res<Assets<PackedDecalMaterial>>, handles: Res<PackedDecalBuffers>, mut buffers: ResMut<Assets<ShaderBuffer>>,
) {
    let mut entities: Vec<_> = decals.iter().map(|(entity, _, _, _)| entity).collect();
    entities.sort_unstable();
    let mut values = Vec::with_capacity(entities.len().max(1));
    for entity in entities {
        let (_, handle, transform, mut decal) = decals.get_mut(entity).unwrap();
        let Some(material) = materials.get(&handle.0) else { decal.metallic_roughness_texture = None; continue; };
        let index = values.len() as u32;
        decal.tag = PACKED_DECAL_TAG | index;
        decal.base_color_texture = None;
        decal.normal_map_texture = None;
        decal.emissive_texture = None;
        decal.metallic_roughness_texture = Some(material.texture.clone());
        values.push(PackedDecalData { local_from_world: transform.affine().inverse().into(), color: material.color,
            material: Vec4::new(material.roughness_scale, material.metallic, material.specular, 0.0) });
    }
    if values.is_empty() { values.push(default()); }
    buffers.get_mut(&handles.materials).unwrap().set_data(values);
}

fn prepare_receivers(
    mut commands: Commands,
    source: Res<Assets<StandardMaterial>>, mut targets: ResMut<Assets<PackedDecalReceiverMaterial>>,
    handles: Res<PackedDecalBuffers>, mut changes: MessageReader<AssetEvent<StandardMaterial>>,
    meshes: Query<(Entity, &MeshMaterial3d<StandardMaterial>), With<Mesh3d>>,
    converted: Query<(Entity, &StandardMaterialSource), With<MeshMaterial3d<PackedDecalReceiverMaterial>>>,
    excluded: Query<(), With<NoDecalReceiver>>,
    mut cache: Local<HashMap<AssetId<StandardMaterial>, Handle<PackedDecalReceiverMaterial>>>,
) {
    let ignored = |entity| excluded.contains(entity);
    let changed: HashSet<_> = changes.read().filter_map(|event| match event {
        AssetEvent::Modified { id } => Some(*id), _ => None,
    }).collect();
    let mut active = HashSet::new();
    for (entity, material) in &converted {
        if ignored(entity) {
            commands.entity(entity).remove::<(MeshMaterial3d<PackedDecalReceiverMaterial>, StandardMaterialSource)>().insert(MeshMaterial3d(material.0.clone()));
        } else { active.insert(material.0.id()); }
    }
    for (entity, material) in &meshes {
        if ignored(entity) { continue; }
        let Some(base) = source.get(&material.0) else { continue; };
        if !matches!(base.alpha_mode, AlphaMode::Opaque | AlphaMode::Mask(_) | AlphaMode::AlphaToCoverage) { continue; }
        active.insert(material.0.id());
        let target = cache.entry(material.0.id()).or_insert_with(|| targets.add(PackedDecalReceiverMaterial {
            base: base.clone(), extension: PackedDecalReceiverExtension { decals: handles.materials.clone(), probe: handles.probe.clone() },
        }));
        commands.entity(entity).remove::<MeshMaterial3d<StandardMaterial>>()
            .insert((MeshMaterial3d(target.clone()), StandardMaterialSource(material.0.clone())));
    }
    cache.retain(|id, _| active.contains(id));
    for id in changed {
        if let Some(handle) = cache.get(&id) && let Some(base) = source.get(id) && let Some(mut target) = targets.get_mut(handle) { target.base = base.clone(); }
    }
}

fn capture_decal_pipelines(
    cache: Res<bevy::render::render_resource::PipelineCache>, status: Res<DecalRenderStatus>,
    device: Res<bevy::render::renderer::RenderDevice>, adapter: Res<bevy::render::renderer::RenderAdapter>,
    decals: Res<bevy::pbr::decal::clustered::RenderClusteredDecals>,
) {
    use bevy::render::render_resource::{CachedPipelineState, PipelineDescriptor};
    let mut current = status.0.lock().unwrap();
    current.clustered_supported = bevy::pbr::decal::clustered::clustered_decals_are_usable(&device, &adapter);
    current.native_decals = decals.len();
    current.receiver_ready = false;
    current.errors.clear();
    current.pipeline_labels.clear();
    for pipeline in cache.pipelines() {
        let PipelineDescriptor::RenderPipelineDescriptor(descriptor) = &pipeline.descriptor else { continue; };
        let label = descriptor.label.as_deref().unwrap_or_default();
        if !label.starts_with("als_packed_receiver") && !label.starts_with("als_playground_receiver") { continue; }
        current.pipeline_labels.push(label.to_owned());
        match &pipeline.state {
            CachedPipelineState::Ok(_) => current.receiver_ready |= label.ends_with("opaque_mesh_pipeline") || label.ends_with("alpha_to_coverage_mesh_pipeline"),
            CachedPipelineState::Err(error) => current.errors.push(format!("{label}: {error}")),
            _ => {}
        }
    }
    current.decal_ready = current.clustered_supported && current.receiver_ready && current.material_response_samples == DECAL_PROBE_SAMPLES && current.material_response_max_error <= 1e-5;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receivers_preserve_updates_exclusions_and_release_unused_materials() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<StandardMaterial>().init_asset::<PackedDecalReceiverMaterial>()
            .init_resource::<PackedDecalBuffers>().add_systems(Update, prepare_receivers);
        let material = app.world_mut().resource_mut::<Assets<StandardMaterial>>().add(StandardMaterial {
            base_color: Color::linear_rgb(0.2, 0.3, 0.4), metallic: 0.7, perceptual_roughness: 0.8, reflectance: 0.9, ..default()
        });
        let first = app.world_mut().spawn((Mesh3d::default(), MeshMaterial3d(material.clone()))).id();
        let second = app.world_mut().spawn((Mesh3d::default(), MeshMaterial3d(material.clone()))).id();
        let excluded = app.world_mut().spawn((Mesh3d::default(), MeshMaterial3d(material.clone()), NoDecalReceiver)).id();
        app.update();
        let handle = app.world().get::<MeshMaterial3d<PackedDecalReceiverMaterial>>(first).unwrap().0.clone();
        assert_eq!(app.world().get::<MeshMaterial3d<PackedDecalReceiverMaterial>>(second).unwrap().0, handle);
        assert_eq!(app.world().get::<MeshMaterial3d<StandardMaterial>>(excluded).unwrap().0, material);
        assert!(app.world().get::<MeshMaterial3d<PackedDecalReceiverMaterial>>(excluded).is_none());
        assert_eq!(app.world().get::<StandardMaterialSource>(first).unwrap().0, material);
        let read = |app: &App| app.world().resource::<Assets<PackedDecalReceiverMaterial>>().get(&handle).unwrap().base.clone();
        assert_eq!(read(&app).metallic, 0.7);
        assert_eq!(read(&app).reflectance, 0.9);
        app.world_mut().resource_mut::<Assets<StandardMaterial>>().get_mut(&material).unwrap().metallic = 0.4;
        app.update();
        app.update();
        assert_eq!(read(&app).metallic, 0.4);
        app.world_mut().entity_mut(first).insert(NoDecalReceiver);
        app.update();
        assert_eq!(app.world().get::<MeshMaterial3d<StandardMaterial>>(first).unwrap().0, material);
        assert!(app.world().get::<StandardMaterialSource>(first).is_none());
        assert!(app.world().get::<MeshMaterial3d<PackedDecalReceiverMaterial>>(first).is_none());
        app.world_mut().despawn(second);
        drop(handle);
        for _ in 0..4 { app.update(); }
        assert!(app.world().resource::<Assets<PackedDecalReceiverMaterial>>().is_empty());
    }

    #[test]
    fn gpu_material_probe_rejects_wrong_blend_values() {
        let mut words = vec![0_u32; DECAL_PROBE_WORDS];
        words[0] = 1;
        assert!(material_probe_error(&words).is_none());
        words[1] = 1;
        let values: [f32; 28] = [0.8, 0.4, 0.2, 0.9, 0.6, 0.9, 0.7, 0.5,
            0.1, 0.8, 0.3, 0.5, 0.0, 0.0, 0.0, 0.6, 0.2, 0.0, 0.5, 0.0,
            0.56, 0.28, 0.14, 0.678, 0.42, 0.642_f32.sqrt(), 0.418_f32.sqrt(), 0.5];
        for (word, value) in words[4..].iter_mut().zip(values) { *word = value.to_bits(); }
        assert!(material_probe_error(&words).unwrap().1 < 1e-6);
        for index in 20..28 {
            let mut invalid = words.clone();
            invalid[4 + index] = 0.0_f32.to_bits();
            assert!(material_probe_error(&invalid).unwrap().1 > 0.1);
        }
    }

    #[test]
    fn packed_decals_modify_surface_properties_before_lighting() {
        let decal = include_str!("../../../assets/shaders/packed_decal.wgsl");
        assert!(!decal.contains("apply_pbr_lighting("), "Decal still computes independent lighting before alpha blending");
        let receiver = include_str!("../../../assets/shaders/playground_tiles.wgsl");
        let blend = receiver.find("apply_packed_decals(&pbr_input)").expect("Receiving material does not consume packed decals");
        let lighting = receiver.find("apply_pbr_lighting(pbr_input)").unwrap();
        assert!(blend < lighting, "Decal material properties were blended after lighting");
    }
}
