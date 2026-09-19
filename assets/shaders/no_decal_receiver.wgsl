#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_prepass_functions::{prepass_alpha_discard, calculate_motion_vector},
    pbr_fragment::pbr_input_from_standard_material,
}

#ifdef PREPASS_FRAGMENT
@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    prepass_alpha_discard(in);
    var out: FragmentOutput;
#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    out.frag_depth = in.unclipped_depth;
#endif
#ifdef NORMAL_PREPASS
    let pbr_input = pbr_input_from_standard_material(in, is_front);
    out.normal = vec4(pbr_input.N * 0.5 + vec3(0.5), 0.0);
#endif
#ifdef MOTION_VECTOR_PREPASS
    out.motion_vector = calculate_motion_vector(in.world_position, in.previous_world_position);
#endif
    return out;
}
#else
@fragment
fn fragment(in: VertexOutput) {
    prepass_alpha_discard(in);
}
#endif
