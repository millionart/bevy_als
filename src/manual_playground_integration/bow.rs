use super::*;
use als_core::input::AlsInputCommand;
use bevy_animation_graph::core::state_machine::low_level::LowLevelStateId;

const GRAPH: &str = "generated/ALS/Character/AnimationInstances/Overlays/AB_Als_Bow/Overlay.animgraph.ron";
const SYNC: &str = "sync_groups.ab_als_bow.secondary_motion.phase";
pub(super) const LABELS: [&str; 6] = ["bow_relaxed", "bow_standing_aim", "bow_release", "bow_walking_aim", "bow_crouching_aim", "bow_standing_aim_return"];

#[derive(Default)]
pub(super) struct Probe {
    pub(super) index: usize,
    pub(super) covered: [bool; 6],
    seconds: [f32; 6],
    canonical_seen: [bool; 2],
    pub(super) waited_seconds: f32,
    pub(super) sample: Sample,
}

#[derive(Default, Serialize)]
pub(super) struct Sample {
    state: Option<String>,
    transition: Option<String>,
    elapsed: Option<f32>,
    weights: Vec<(String, f32)>,
    phase: Option<f32>,
    aiming: [Option<f32>; 2],
    fsm_arms: Vec<(String, [f32; 4])>,
    root_arms: Vec<(String, [f32; 4])>,
}

pub(super) fn command(index: usize) -> AlsInputCommand {
    AlsInputCommand {
        aim_held: matches!(index, 1 | 3..=5),
        move_dir: if index == 3 { Vec2::Y } else { Vec2::ZERO },
        ..default()
    }
}

fn expected_state(index: usize) -> &'static str {
    if matches!(index, 0 | 2) { "Relaxed" } else { "Aiming" }
}

fn settled(index: usize, stance: Stance, mode: RotationMode, speed: f32, sample: &Sample) -> bool {
    sample.state.as_deref() == Some(expected_state(index)) && sample.transition.is_none()
        && stance == if index == 4 { Stance::Crouching } else { Stance::Standing }
        && (mode == RotationMode::Aiming) == command(index).aim_held
        && if index == 3 { speed > 0.5 } else { speed < 0.1 }
        && sample.aiming.iter().all(|value| value.is_some_and(|value| (value - command(index).aim_held as u8 as f32).abs() < 1e-4))
        && sample.fsm_arms.len() == 4 && sample.root_arms.len() == 4
}

fn arms(pose: &Pose, skeletons: &Assets<Skeleton>) -> Vec<(String, [f32; 4])> {
    let Some(skeleton) = skeletons.get(&pose.skeleton) else { return Vec::new(); };
    skeleton.iter_bones().filter_map(|id| {
        let path = skeleton.id_to_path(id)?;
        let name = path.parts.last()?.as_str();
        ["upperarm_l", "lowerarm_l", "upperarm_r", "lowerarm_r"].contains(&name)
            .then(|| Some((name.to_owned(), pose.get_bone(id)?.rotation?.to_array()))).flatten()
    }).collect()
}

fn sample(player: &AnimationGraphPlayer, probe: &MotionProbe, root_context: Option<&AlsRootAnimationContext>) -> Sample {
    let mut sample = Sample::default();
    sample.aiming[1] = root_context.map(|context| context.f32("PoseAiming").unwrap_or(0.0));
    if let Some(DataValue::Pose(pose)) = player.get_outputs().get("pose") {
        sample.root_arms = arms(pose, &probe.skeletons);
    }
    let Some(arena) = player.get_context_arena() else { return sample; };
    sample.phase = als_ext::sync_group_phase::source_sync_phase(arena, &probe.graphs, SYNC);
    for id in arena.iter_context_ids() {
        let Some(context) = arena.get_context(id) else { continue; };
        if probe.asset_server.get_path(context.get_graph_id()).is_none_or(|path| path.path() != std::path::Path::new(GRAPH)) { continue; }
        let Some(graph) = probe.graphs.get(context.get_graph_id()) else { continue; };
        let Some(node) = graph.nodes.values().find(|node| node.name == "Bow States\nState Machine") else { continue; };
        if !context.node_caches.is_updated(node.id, StateKey::Default) { continue; }
        let Some(fsm) = node.try_inner_downcast_ref::<als_ext::InterruptibleFsmNode>() else { continue; };
        let Some(machine) = probe.machines.get(&fsm.fsm) else { continue; };
        let Ok(runtime) = context.node_states.get::<als_ext::InterruptibleFsmState>(node.id, StateKey::Default) else { continue; };
        sample.state = match &runtime.state {
            LowLevelStateId::HlState(id) => machine.states.get(id).map(|state| state.label.clone()),
            LowLevelStateId::HlTransition(_) => None,
        };
        sample.transition = runtime.active_direct_transition_id();
        sample.elapsed = Some(runtime.current_state_elapsed_time());
        sample.aiming[0] = Some(runtime.context.curves.get("PoseAiming").copied().unwrap_or(0.0));
        sample.weights = machine.states.values().map(|state| (state.label.clone(), runtime.recorded_state_weight(&state.id.uuid().to_string()))).collect();
        if let Some(pose) = runtime.last_output_pose() { sample.fsm_arms = arms(pose, &probe.skeletons); }
    }
    sample
}

pub(super) fn observe(probe: &mut Probe, player: &AnimationGraphPlayer, motion: &MotionProbe,
    context: Option<&AlsRootAnimationContext>, stance: Stance, mode: RotationMode, speed: f32,
    window: &mut ManualAnimationWindowEvidence) {
    probe.sample = sample(player, motion, context);
    let index = probe.index;
    if !settled(index, stance, mode, speed, &probe.sample) { return; }
    let state = if expected_state(index) == "Aiming" { 1 } else { 0 };
    *window = ManualAnimationWindowEvidence {
        label: Some(LABELS[index]), index: Some(index), settled: true,
        capture_source_window: true, capture_every_frame: true,
        requires_non_reference_root_pose: true, requires_source_pose_chain: true,
        source_node: Some("Bow States\nState Machine"), source_state: Some(expected_state(index)),
        expected_runtime_state: Some(state), observed_runtime_state: Some(state),
        minimum_observation_seconds: 0.3,
        source_sync_group: Some("Secondary Motion"), sync_phase_global: Some(SYNC),
        canonical_sync_phase: (index < 2).then_some(0.35), maximum_sync_phase_distance: 0.04,
        direction_signature_group: (index < 2).then_some("bow_standing_pose"),
        required_direction_signature_members: if index < 2 { 2 } else { 0 },
        minimum_direction_signature_rotation_radians: if index < 2 { 0.01 } else { 0.0 },
        expected_direction: Some(LABELS[index]), observed_direction: Some(LABELS[index]),
        source_pose_chain: ["Bow States\nState Machine", "Output Pose\nOverlay"].map(|node| ManualAnimationPoseBoundary {
            label: node, graph: GRAPH, node, output: "Pose", minimum_translation_span: 0.0, minimum_rotation_span_radians: 0.0,
        }).to_vec(),
        ..default()
    };
    probe.seconds[index] += graph_global_f32(player, "animation.delta_seconds").unwrap_or(0.0);
    if index < 2 { probe.canonical_seen[index] |= probe.sample.phase.is_some_and(|phase| (phase - 0.35).abs() <= 0.04); }
    probe.covered[index] |= probe.seconds[index] >= 0.5 && (index >= 2 || probe.canonical_seen[index]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bow_fixture_requires_actual_state_curve_and_normal_held_input() {
        assert!(!command(0).aim_held && command(1).aim_held && !command(2).aim_held);
        assert_eq!(command(3).move_dir, Vec2::Y);
        assert_eq!(command(4).move_dir, Vec2::ZERO);
        let mut sample = Sample { state: Some("Aiming".into()), aiming: [Some(1.0); 2],
            fsm_arms: vec![(String::new(), [0.0; 4]); 4], root_arms: vec![(String::new(), [0.0; 4]); 4], ..default() };
        assert!(settled(1, Stance::Standing, RotationMode::Aiming, 0.0, &sample));
        sample.state = Some("Ready".into());
        assert!(!settled(1, Stance::Standing, RotationMode::Aiming, 0.0, &sample));
        sample.state = Some("Aiming".into());
        sample.aiming[1] = Some(0.0);
        assert!(!settled(1, Stance::Standing, RotationMode::Aiming, 0.0, &sample));
        assert!(!settled(4, Stance::Standing, RotationMode::Aiming, 0.0, &sample));
        assert!(!default_overlay_should_timeout(false, 11.9));
        assert!(default_overlay_should_timeout(false, 12.0));
        assert!(!default_overlay_should_timeout(true, 12.0));
    }
}
