use super::*;
use crate::footprint::{Footprint, FootprintDiagnostics, FOOTPRINT, footprint_opacity};
use als_ext::projected_decal::{NoDecalMaterial, PackedDecalMaterial};

pub(super) const LABELS: [&str; 8] = ["footprint_prepare", "footprint_walk", "footprint_walk_stop", "footprint_run", "footprint_run_stop", "footprint_crouch", "footprint_crouch_stop", "footprint_cleanup"];

#[derive(Default)]
pub(super) struct Probe {
    pub(super) stage: usize,
    pub(super) covered: [bool; 8],
    pub(super) previous_spawned: [u64; 2],
    pub(super) visible: bool,
    pub(super) faded: bool,
    pub(super) expired: bool,
    pub(super) moving: bool,
    pub(super) receiver: bool,
    pub(super) pipelines: bool,
    pub(super) errors: Vec<String>,
    pub(super) waited: f32,
    anchors: std::collections::HashMap<Entity, Vec3>,
    last_stage: usize,
}

pub(super) fn install(app: &mut App) {
    app.add_systems(PostUpdate, observe
        .run_if(|state: Res<ManualIntegrationState>| state.scenario == IntegrationScenario::Footprint)
        .after(crate::footprint::FootprintSet).after(AlsCameraSet)
        .before(observe_manual_integration).in_set(ManualIntegrationObservationSet));
}

fn observe(
    mut state: ResMut<ManualIntegrationState>,
    mut report: ResMut<ManualIntegrationGateReport>,
    mut window: ResMut<ManualAnimationWindowEvidence>,
    time: Res<Time>,
    characters: Query<(&FootprintDiagnostics, &LocomotionState, &Stance, &AlsActualMotion)>,
    footprints: Query<(Entity, &Footprint, &Transform, &GlobalTransform, Option<&ChildOf>, &MeshMaterial3d<PackedDecalMaterial>, &ViewVisibility)>,
    globals: Query<&GlobalTransform>,
    owners: Query<&avian3d::prelude::ColliderOf>,
    bodies: Query<&RigidBody>,
    materials: Res<Assets<PackedDecalMaterial>>,
    no_receivers: Query<&MeshMaterial3d<NoDecalMaterial>>,
    images: Res<Assets<Image>>,
    render: Res<als_ext::projected_decal::DecalRenderStatus>,
) {
    let Some(character) = state.character else { return; };
    let Ok((diagnostics, mode, stance, motion)) = characters.get(character) else { return; };
    let elapsed = state.phase_elapsed();
    let phase = state.phase;
    let probe = &mut state.footprint_probe;
    let pipelines = render.0.lock().unwrap().clone();
    probe.pipelines = pipelines.decal_ready && pipelines.receiver_ready && pipelines.errors.is_empty();
    if diagnostics.invalid > 0 && !probe.errors.iter().any(|error| error == "Footstep inputs or final bones were invalid") {
        probe.errors.push("Footstep inputs or final bones were invalid".into());
    }
    probe.receiver |= no_receivers.iter().count() >= 15;
    probe.expired |= diagnostics.expired > 0;
    let mut alive = 0;
    for (entity, mark, local, world, parent, material, visible) in &footprints {
        if mark.character != character { continue; }
        alive += 1;
        let age = time.elapsed_secs_f64() - mark.created_at;
        let alpha = FOOTPRINT.material.opacity * footprint_opacity(age, mark.lifetime);
        let material = materials.get(&material.0);
        if material.is_none_or(|material| (material.base.base_color.alpha() - alpha).abs() > 1e-5) {
            if !probe.errors.iter().any(|error| error == "Footprint opacity differs from source lifetime") { probe.errors.push("Footprint opacity differs from source lifetime".into()); }
        }
        probe.faded |= alpha > 0.0 && alpha < FOOTPRINT.material.opacity - 0.01;
        probe.visible |= visible.get() && material.is_some_and(|material| images.get(&material.extension.texture).is_some_and(|image| image.size() == UVec2::splat(512)));
        let valid_parent = parent.is_some_and(|parent| parent.parent() == mark.surface);
        if !valid_parent { probe.errors.push("Footprint is not attached to its hit component".into()); }
        if let Ok(parent) = globals.get(mark.surface) {
            let expected = parent.mul_transform(*local);
            if expected.translation().distance(world.translation()) > 1e-4 { probe.errors.push("Footprint world location did not follow its hit component".into()); }
        }
        let body = owners.get(mark.surface).map_or(mark.surface, |owner| owner.body);
        if bodies.get(body).is_ok_and(|body| *body == RigidBody::Kinematic) {
            let start = *probe.anchors.entry(entity).or_insert(world.translation());
            probe.moving |= start.distance(world.translation()) > 0.05;
        }
    }
    probe.anchors.retain(|entity, _| footprints.contains(*entity));
    if phase == IntegrationPhase::Footprints {
        let stage = probe.stage;
        if probe.last_stage != stage {
            probe.previous_spawned = diagnostics.spawned;
            probe.last_stage = stage;
        }
        let moving = matches!(stage, 1 | 3 | 5);
        let stable = *mode == LocomotionState::Grounded && *stance == if stage == 5 { Stance::Crouching } else { Stance::Standing }
            && if moving { motion.horizontal_speed > 0.5 } else { motion.horizontal_speed < 0.1 };
        let footprint_pair = (0..2).all(|side| diagnostics.spawned[side] > probe.previous_spawned[side]);
        probe.covered[stage] |= stable && if stage == 7 { elapsed > 8.5 && alive == 0 } else if moving { elapsed > 2.0 && footprint_pair } else { elapsed > 0.5 };
        *window = ManualAnimationWindowEvidence { label: Some(LABELS[stage]), index: Some(stage), settled: stable,
            capture_source_window: stable, capture_every_frame: true, requires_non_reference_root_pose: true,
            requires_source_pose_chain: true, minimum_observation_seconds: 0.25,
            source_pose_chain: vec![ManualAnimationPoseBoundary { label: "locomotion", graph: LOCOMOTION_ROOT,
                node: "Output Pose\nAnimGraph", output: "Pose", minimum_translation_span: 0.0, minimum_rotation_span_radians: 0.0 }], ..default() };
    }
    report.direction_trace.push(serde_json::json!({"footprint_stage": probe.stage, "time": time.elapsed_secs_f64(), "alive": alive,
        "notifications":diagnostics.notifications,"spawned":diagnostics.spawned,"expired":diagnostics.expired,
        "missed":diagnostics.missed,"angle":diagnostics.rejected_angle,"invalid":diagnostics.invalid,
        "visible":probe.visible,"moving":probe.moving,"receiver":probe.receiver,"pipelines":probe.pipelines,
        "pipeline_errors":pipelines.errors,"errors":probe.errors}));
}
