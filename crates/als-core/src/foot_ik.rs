use avian3d::prelude::{SpatialQuery, SpatialQueryFilter};
use bevy::prelude::*;

use crate::character::AlsCharacter;
use crate::state::LocomotionState;

pub const FOOT_BONE_NAMES: [&str; 2] = ["foot_l", "foot_r"];
pub const PROBE_UP_OFFSET: f32 = 0.15;
pub const PROBE_DEPTH: f32 = 0.85;
pub const ANKLE_TO_SOLE: f32 = 0.09;
pub const MAX_FOOT_REACH: f32 = 0.45;

const PELVIS_INTERP_SPEED: f32 = 12.0;
const FOOT_INTERP_SPEED: f32 = 18.0;
const WEIGHT_BLEND_SPEED: f32 = 10.0;
const PIN_WEIGHT_SPEED: f32 = 12.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlsGroundHit {
    pub point: Vec3,
    pub normal: Vec3,
}

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct AlsFootProbe {
    pub left: Option<AlsGroundHit>,
    pub right: Option<AlsGroundHit>,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct AlsFootBones {
    pub left: Entity,
    pub right: Entity,
}

#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct AlsFootPinRequest {
    pub left: bool,
    pub right: bool,
}

#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct AlsFootIkTargets {
    pub grounded: bool,
    pub pin_left: bool,
    pub pin_right: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct FootSideInput {
    pub raw_offset: f32,
    pub pin_request: bool,
}

impl Default for FootSideInput {
    fn default() -> Self {
        Self {
            raw_offset: 0.0,
            pin_request: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct SideRuntime {
    offset: f32,
    ik_weight: f32,
    pin_weight: f32,
    locked_offset: Option<f32>,
}

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct AlsFootIkRuntime {
    left: SideRuntime,
    right: SideRuntime,
    pub pelvis_drop: f32,
}

pub fn foot_target_offset(ankle_y: f32, hit: &AlsGroundHit) -> f32 {
    let raw = hit.point.y + ANKLE_TO_SOLE - ankle_y;
    raw.clamp(-MAX_FOOT_REACH, MAX_FOOT_REACH)
}

pub fn foot_ik_target(hit: &AlsGroundHit) -> Vec3 {
    hit.point + Vec3::Y * ANKLE_TO_SOLE
}

pub fn slope_is_walkable(normal: Vec3, max_slope_degrees: f32) -> bool {
    let up_dot = normal.normalize_or_zero().dot(Vec3::Y);
    up_dot >= max_slope_degrees.to_radians().cos()
}

fn move_toward_cap(current: f32, target: f32, max_delta: f32) -> f32 {
    let diff = target - current;
    if diff.abs() <= max_delta {
        target
    } else {
        current + diff.signum() * max_delta
    }
}

pub fn pelvis_drop(offset_left: f32, offset_right: f32, w_left: f32, w_right: f32) -> f32 {
    (offset_left * w_left).min(offset_right * w_right).min(0.0)
}

fn step_side(rt: &mut SideRuntime, input: &FootSideInput, ik_target: f32, dt: f32) -> f32 {
    rt.ik_weight = move_toward_cap(rt.ik_weight, ik_target, WEIGHT_BLEND_SPEED * dt);

    if input.pin_request && rt.locked_offset.is_none() {
        rt.locked_offset = Some(rt.offset);
    } else if !input.pin_request && rt.pin_weight < 1e-3 {
        rt.locked_offset = None;
    }

    let pin_goal = if input.pin_request && rt.locked_offset.is_some() {
        1.0
    } else {
        0.0
    };
    rt.pin_weight = move_toward_cap(rt.pin_weight, pin_goal, PIN_WEIGHT_SPEED * dt);

    let locked = rt.locked_offset.unwrap_or(rt.offset);
    let base = input.raw_offset * rt.ik_weight;
    let target = base + (locked - base) * rt.pin_weight;

    rt.offset = move_toward_cap(rt.offset, target, FOOT_INTERP_SPEED * dt);
    rt.offset
}

impl AlsFootIkRuntime {
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        left: &FootSideInput,
        right: &FootSideInput,
        ik_target: f32,
        dt: f32,
    ) -> (f32, f32, f32) {
        let off_left = step_side(&mut self.left, left, ik_target, dt);
        let off_right = step_side(&mut self.right, right, ik_target, dt);
        self.pelvis_drop = move_toward_cap(
            self.pelvis_drop,
            pelvis_drop(off_left, off_right, self.left.ik_weight, self.right.ik_weight),
            PELVIS_INTERP_SPEED * dt,
        );
        (off_left, off_right, self.pelvis_drop)
    }

    pub fn side(&self, side: usize) -> (f32, f32) {
        let rt = match side {
            0 => &self.left,
            _ => &self.right,
        };
        (rt.offset, rt.pin_weight)
    }

    pub fn ik_weights(&self) -> (f32, f32) {
        (self.left.ik_weight, self.right.ik_weight)
    }
}

pub fn foot_ik_state_system(
    mut commands: Commands,
    q: Query<
        (
            Entity,
            &LocomotionState,
            Option<&AlsFootPinRequest>,
            Option<&AlsFootIkTargets>,
        ),
        With<AlsCharacter>,
    >,
) {
    for (entity, loco, pins, targets) in &q {
        let grounded = *loco == LocomotionState::Grounded;
        let desired = AlsFootIkTargets {
            grounded,
            pin_left: pins.map(|p| p.left).unwrap_or(false) && grounded,
            pin_right: pins.map(|p| p.right).unwrap_or(false) && grounded,
        };
        if targets != Some(&desired) {
            commands.entity(entity).insert(desired);
        }
    }
}

pub const TURN_PIN_HOLD_SECS: f64 = 1.2;

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct AlsFootPinHold {
    pub until_secs: f64,
}

pub fn extend_foot_pin_hold(hold: &mut AlsFootPinHold, now_secs: f64) {
    hold.until_secs = hold.until_secs.max(now_secs + TURN_PIN_HOLD_SECS);
}

pub fn foot_pin_request_system(
    time: Res<Time>,
    mut q: Query<
        (
            Entity,
            &crate::start_stop::StartStopState,
            Option<&mut AlsFootPinHold>,
            Option<&mut AlsFootPinRequest>,
        ),
        With<AlsCharacter>,
    >,
    mut commands: Commands,
) {
    let now = time.elapsed_secs_f64();
    for (entity, start_stop, hold, pins) in &mut q {
        let stopping = matches!(
            start_stop.phase,
            crate::start_stop::StartStopPhase::Stopping(_)
        );
        let turning = hold.map(|h| now < h.until_secs).unwrap_or(false);
        let want = stopping || turning;
        let desired = AlsFootPinRequest {
            left: want,
            right: want,
        };
        match pins {
            Some(mut current) => {
                if *current != desired {
                    if want {
                        *current = desired;
                    } else {
                        commands.entity(entity).remove::<AlsFootPinRequest>();
                    }
                }
            }
            None => {
                if want {
                    commands.entity(entity).insert(desired);
                }
            }
        }
    }
}

fn collect_named_descendants(
    root: Entity,
    children_q: &Query<&Children>,
    name_q: &Query<&Name>,
    wanted: &[&str],
    out: &mut Vec<(String, Entity)>,
) {
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        let Ok(children) = children_q.get(entity) else {
            continue;
        };
        for child in children.iter() {
            if let Ok(name) = name_q.get(child) {
                let n = name.as_str();
                if wanted.contains(&n) && !out.iter().any(|(s, _)| s == n) {
                    out.push((n.to_string(), child));
                }
            }
            stack.push(child);
        }
    }
}

pub fn build_foot_bone_map(
    root: Entity,
    children_q: &Query<&Children>,
    name_q: &Query<&Name>,
) -> Option<AlsFootBones> {
    let mut found = Vec::new();
    collect_named_descendants(root, children_q, name_q, &FOOT_BONE_NAMES, &mut found);
    let left = found.iter().find(|(n, _)| n == "foot_l").map(|(_, e)| *e);
    let right = found.iter().find(|(n, _)| n == "foot_r").map(|(_, e)| *e);
    Some(AlsFootBones {
        left: left?,
        right: right?,
    })
}

pub fn foot_bone_map_system(
    mut commands: Commands,
    characters: Query<Entity, (With<AlsCharacter>, Without<AlsFootBones>)>,
    children_q: Query<&Children>,
    name_q: Query<&Name>,
) {
    for entity in &characters {
        if let Some(map) = build_foot_bone_map(entity, &children_q, &name_q) {
            commands.entity(entity).insert(map);
        }
    }
}

pub fn foot_ground_probe_system(
    mut commands: Commands,
    spatial: SpatialQuery,
    characters: Query<(Entity, &GlobalTransform, &AlsFootBones), With<AlsCharacter>>,
    bones_tf: Query<&GlobalTransform, Without<AlsCharacter>>,
) {
    for (entity, char_tf, feet) in &characters {
        let origin_base = char_tf.translation();
        let mut probe = AlsFootProbe::default();
        for (bone, slot) in [(feet.left, 0usize), (feet.right, 1usize)] {
            let Ok(bone_tf) = bones_tf.get(bone) else {
                continue;
            };
            let ankle = bone_tf.translation();
            if (ankle - origin_base).length() > 3.0 {
                continue;
            }
            let origin = ankle + Vec3::Y * PROBE_UP_OFFSET;
            let filter = SpatialQueryFilter::from_excluded_entities([entity, bone]);
            if let Some(hit) = spatial.cast_ray(origin, Dir3::NEG_Y, PROBE_DEPTH, true, &filter) {
                let value = AlsGroundHit {
                    point: origin - Vec3::Y * hit.distance,
                    normal: hit.normal,
                };
                if slot == 0 {
                    probe.left = Some(value);
                } else {
                    probe.right = Some(value);
                }
            }
        }
        commands.entity(entity).insert(probe);
    }
}

pub struct AlsFootProbePlugin;

impl Plugin for AlsFootProbePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            (
                foot_bone_map_system,
                foot_ground_probe_system,
                foot_pin_request_system,
                foot_ik_state_system,
            )
                .chain(),
        );
    }
}

#[cfg(test)]
mod pin_request_tests {
    use super::*;
    use crate::start_stop::{StartStopPhase, StartStopState};
    use bevy::MinimalPlugins;

    fn app_with_character(phase: StartStopPhase) -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(Update, foot_pin_request_system);
        let e = app
            .world_mut()
            .spawn((
                AlsCharacter,
                StartStopState {
                    phase,
                    last_direction: None,
                },
            ))
            .id();
        (app, e)
    }

    #[test]
    fn stopping_phase_pins_both_feet() {
        let (mut app, e) = app_with_character(StartStopPhase::Stopping(
            crate::start_stop::MoveDirection::Forward,
        ));
        app.update();
        let pins = *app.world().get::<AlsFootPinRequest>(e).unwrap();
        assert!(pins.left && pins.right);
    }

    #[test]
    fn idle_phase_never_pins() {
        let (mut app, e) = app_with_character(StartStopPhase::Idle);
        app.update();
        assert!(app.world().get::<AlsFootPinRequest>(e).is_none());
    }

    #[test]
    fn hold_component_pins_until_expiry() {
        let (mut app, e) = app_with_character(StartStopPhase::Idle);
        let now = app.world().resource::<Time>().elapsed_secs_f64();
        app.world_mut().entity_mut(e).insert(AlsFootPinHold {
            until_secs: now + TURN_PIN_HOLD_SECS,
        });
        app.update();
        let pins = *app.world().get::<AlsFootPinRequest>(e).unwrap();
        assert!(pins.left && pins.right, "hold window pins feet");

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f64(TURN_PIN_HOLD_SECS * 2.0));
        app.world_mut()
            .get_mut::<AlsFootPinHold>(e)
            .unwrap()
            .until_secs = 0.0;
        app.update();
        assert!(
            app.world().get::<AlsFootPinRequest>(e).is_none(),
            "expired hold without stop phase clears pins"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    #[test]
    fn target_offset_positive_when_ground_above_ankle() {
        let hit = AlsGroundHit {
            point: Vec3::new(1.0, 0.20, 2.0),
            normal: Vec3::Y,
        };
        let off = foot_target_offset(0.10, &hit);
        assert!((off - (0.20 + ANKLE_TO_SOLE - 0.10)).abs() < 1e-5);
    }

    #[test]
    fn offset_clamps_to_max_reach() {
        let deep = AlsGroundHit {
            point: Vec3::new(0.0, -5.0, 0.0),
            normal: Vec3::Y,
        };
        assert!((foot_target_offset(0.0, &deep) + MAX_FOOT_REACH).abs() < 1e-5);
    }

    #[test]
    fn slope_gate_matches_als_max_slope() {
        assert!(slope_is_walkable(Vec3::Y, 50.0));
        let steep = Vec3::new(1.0, 0.5, 0.0);
        assert!(!slope_is_walkable(steep, 50.0));
        assert!(!slope_is_walkable(Vec3::ZERO, 50.0));
    }

    #[test]
    fn pelvis_takes_lower_weighted_side_and_never_rises() {
        assert!((pelvis_drop(-0.10, -0.02, 1.0, 1.0) + 0.10).abs() < 1e-6);
        assert!((pelvis_drop(-0.10, -0.02, 0.0, 1.0) + 0.02).abs() < 1e-6);
        assert!(pelvis_drop(0.05, 0.03, 1.0, 1.0) <= 0.0);
        assert_eq!(pelvis_drop(0.05, 0.03, 0.0, 0.0), 0.0);
    }

    #[test]
    fn weights_blend_in_and_out_with_speed_cap() {
        let mut rt = AlsFootIkRuntime::default();
        let inp_l = FootSideInput::default();
        let inp_r = FootSideInput::default();

        let (off_l, _, pelvis) = rt.update(&inp_l, &inp_r, 0.0, DT);
        assert_eq!(off_l, 0.0);
        assert_eq!(pelvis, 0.0);

        for _ in 0..60 {
            rt.update(&inp_l, &inp_r, 1.0, DT);
        }
        let (wl, wr) = rt.ik_weights();
        assert!((wl - 1.0).abs() < 1e-4 && (wr - 1.0).abs() < 1e-4);

        for _ in 0..60 {
            rt.update(&inp_l, &inp_r, 0.0, DT);
        }
        let (wl2, _) = rt.ik_weights();
        assert!(wl2.abs() < 1e-4);
    }

    #[test]
    fn offset_tracks_raw_input_after_blend_in() {
        let mut rt = AlsFootIkRuntime::default();
        let raw = FootSideInput {
            raw_offset: 0.20,
            pin_request: false,
        };
        for _ in 0..120 {
            rt.update(&raw, &raw, 1.0, DT);
        }
        let (off_l, _) = rt.side(0);
        assert!((off_l - 0.20).abs() < 1e-3, "got {off_l}");
    }

    #[test]
    fn pin_freezes_offset_at_lock_value_and_releases() {
        let mut rt = AlsFootIkRuntime::default();
        let raised = FootSideInput {
            raw_offset: 0.25,
            pin_request: false,
        };
        for _ in 0..90 {
            rt.update(&raised, &raised, 1.0, DT);
        }
        let (settled, _) = rt.side(0);

        let pinned = FootSideInput {
            raw_offset: -0.30,
            pin_request: true,
        };
        for _ in 0..60 {
            rt.update(&pinned, &pinned, 1.0, DT);
        }
        let (held, pw) = rt.side(0);
        assert!(pw > 0.99);
        assert!(
            (held - settled).abs() < 1e-2,
            "pinned foot keeps its lock height, held={held} settled={settled}"
        );

        let released = FootSideInput {
            raw_offset: -0.30,
            pin_request: false,
        };
        for _ in 0..120 {
            rt.update(&released, &released, 1.0, DT);
        }
        let (after, pw_after) = rt.side(0);
        assert!(pw_after < 1e-3);
        assert!((after - (-0.30)).abs() < 1e-3, "returns to raw after release");
    }

    #[test]
    fn air_grounded_transition_zeroes_effective_offsets() {
        let mut rt = AlsFootIkRuntime::default();
        let raised = FootSideInput {
            raw_offset: 0.40,
            pin_request: false,
        };
        for _ in 0..120 {
            rt.update(&raised, &raised, 1.0, DT);
        }
        for _ in 0..120 {
            rt.update(&raised, &raised, 0.0, DT);
        }
        let (off_l, _) = rt.side(0);
        assert!(off_l.abs() < 1e-2, "ik weight 0 pulls offset to rest, got {off_l}");
    }

    #[test]
    fn pelvis_drop_interpolates_without_overshoot() {
        let mut rt = AlsFootIkRuntime::default();
        let low = FootSideInput {
            raw_offset: -0.20,
            pin_request: false,
        };
        let flat = FootSideInput::default();
        let mut prev = 0.0;
        for _ in 0..120 {
            let (_, _, drop) = rt.update(&low, &flat, 1.0, DT);
            assert!(drop <= prev + 1e-5, "no overshoot toward negative drop");
            prev = drop;
        }
        let (_, _, drop) = rt.update(&low, &flat, 1.0, DT);
        assert!((drop + 0.20).abs() < 1e-2, "settles at min side, got {drop}");
    }
}
