#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    mesh_functions::get_local_from_world,
    mesh_view_bindings::view,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    prepass_utils::{prepass_depth, prepass_normal},
    view_transformations::position_ndc_to_world,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100)
var<uniform> roughness_scale: f32;
@group(#{MATERIAL_BIND_GROUP}) @binding(101)
var packed_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(102)
var packed_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    let depth = prepass_depth(in.position, 0u);
    let screen_uv = (in.position.xy - view.viewport.xy) / view.viewport.zw;
    let world = position_ndc_to_world(vec3(screen_uv * vec2(2.0, -2.0) + vec2(-1.0, 1.0), depth));
    let local = (get_local_from_world(in.instance_index) * vec4(world, 1.0)).xyz;
    let uv = local.yz + vec2(0.5);
    let packed = textureSampleGrad(packed_texture, packed_sampler, uv, dpdx(uv), dpdy(uv));
    if (depth == 0.0 || any(abs(local) > vec3(0.5))) {
        discard;
    }
    var surface = in;
    surface.world_position = vec4(world, 1.0);
    surface.world_normal = prepass_normal(in.position, 0u);
    var pbr_input = pbr_input_from_standard_material(surface, true);
    pbr_input.material.base_color.a *= packed.a * clamp(4.0 - 8.0 * abs(local.x), 0.0, 1.0);
    pbr_input.material.perceptual_roughness = packed.g * roughness_scale;
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
