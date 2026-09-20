#import bevy_pbr::{
    pbr_types::PbrInput,
    clustered_forward,
    mesh_view_bindings,
    decal::clustered::{clustered_decal_iterator_new, clustered_decal_iterator_next, get_view_z, view_is_orthographic},
}

struct PackedDecalData {
    local_from_world: mat4x4<f32>,
    color: vec4<f32>,
    material: vec4<f32>,
}

struct DecalMaterialProbe {
    enabled: u32,
    count: atomic<u32>,
    padding: vec2<u32>,
    values: array<vec4<f32>, 112>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(110)
var<storage, read> packed_decals: array<PackedDecalData>;
@group(#{MATERIAL_BIND_GROUP}) @binding(111)
var<storage, read_write> decal_probe: DecalMaterialProbe;

fn record_material_response(before_color_roughness: vec4<f32>, before_material: vec4<f32>, packed: vec4<f32>, color: vec4<f32>, material: vec4<f32>, after_color_roughness: vec4<f32>, after_material: vec4<f32>) {
    if (decal_probe.enabled == 0u || atomicLoad(&decal_probe.count) >= 16u || packed.a * color.a <= 0.0001) { return; }
    let index = atomicAdd(&decal_probe.count, 1u);
    if (index >= 16u) { return; }
    let offset = index * 7u;
    decal_probe.values[offset] = before_color_roughness;
    decal_probe.values[offset + 1u] = before_material;
    decal_probe.values[offset + 2u] = packed;
    decal_probe.values[offset + 3u] = color;
    decal_probe.values[offset + 4u] = material;
    decal_probe.values[offset + 5u] = after_color_roughness;
    decal_probe.values[offset + 6u] = after_material;
}

fn apply_packed_decals(pbr_input: ptr<function, PbrInput>) {
#ifdef CLUSTERED_DECALS_ARE_USABLE
    let world = (*pbr_input).world_position;
    let world_dx = dpdx(world.xyz);
    let world_dy = dpdy(world.xyz);
    let cluster = clustered_forward::view_fragment_cluster_index((*pbr_input).frag_coord.xy, get_view_z(world.xyz), view_is_orthographic());
    var ranges = clustered_forward::unpack_clusterable_object_index_ranges(cluster);
    var cursor = 0u;
    loop {
        var selected = 0xffffffffu;
        var texture_index = -1i;
        var uv = vec2(0.0);
        var iterator = clustered_decal_iterator_new(world.xyz, &ranges);
        while (clustered_decal_iterator_next(&iterator)) {
            let index = iterator.tag & 0x7fffffffu;
            if ((iterator.tag & 0x80000000u) != 0u && index >= cursor && index < selected && index < arrayLength(&packed_decals) && iterator.metallic_roughness_texture_index >= 0) {
                selected = index;
                texture_index = iterator.metallic_roughness_texture_index;
                uv = iterator.uv;
            }
        }
        if (selected == 0xffffffffu) { break; }
        cursor = selected + 1u;
        let decal = packed_decals[selected];
        let local = (decal.local_from_world * world).xyz;
        let uv_dx = (decal.local_from_world * vec4(world_dx, 0.0)).xy * vec2(1.0, -1.0);
        let uv_dy = (decal.local_from_world * vec4(world_dy, 0.0)).xy * vec2(1.0, -1.0);
        let packed = textureSampleGrad(mesh_view_bindings::clustered_decal_textures[texture_index], mesh_view_bindings::clustered_decal_sampler, uv, uv_dx, uv_dy);
        let color = vec4(decal.color.rgb, decal.color.a * clamp(4.0 - 8.0 * abs(local.z), 0.0, 1.0));
        let alpha = clamp(packed.a * color.a, 0.0, 1.0);
        let before_color_roughness = vec4((*pbr_input).material.base_color.rgb, (*pbr_input).material.perceptual_roughness);
        let before_material = vec4((*pbr_input).material.metallic, (*pbr_input).material.reflectance);
        (*pbr_input).material.base_color = vec4(mix(before_color_roughness.rgb, color.rgb, alpha), (*pbr_input).material.base_color.a);
        (*pbr_input).material.perceptual_roughness = mix(before_color_roughness.a, packed.g * decal.material.x, alpha);
        (*pbr_input).material.metallic = mix(before_material.x, decal.material.y, alpha);
        (*pbr_input).material.reflectance = sqrt(mix(before_material.yzw * before_material.yzw, vec3(decal.material.z * 0.5), alpha));
        record_material_response(before_color_roughness, before_material, packed, color, decal.material,
            vec4((*pbr_input).material.base_color.rgb, (*pbr_input).material.perceptual_roughness),
            vec4((*pbr_input).material.metallic, (*pbr_input).material.reflectance));
    }
#endif
}
