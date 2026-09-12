use avian3d::prelude::{LinearVelocity, ShapeCastConfig, SpatialQuery, SpatialQueryFilter};
use bevy::prelude::*;
use bevy_tnua::TnuaPipelineSystems;

use crate::character::{AlsCharacter, als_character_capsule};
use crate::state::LocomotionState;

pub const LAND_HEAVY_IMPACT_VY: f32 = 8.0;
pub const FALL_FAST_ENTER_VY: f32 = 8.0;
pub const FALL_FAST_SPAN_VY: f32 = 6.0;
pub const AIR_BLEND_SPEED: f32 = 10.0;
pub const LAND_BLEND_SPEED: f32 = 14.0;
pub const LAND_RECOVERY_LIGHT_SECS: f32 = 0.9;
pub const LAND_RECOVERY_HEAVY_SECS: f32 = 1.3;
pub const IMPACT_VY_SMOOTH_ALPHA: f32 = 0.3;
const LAND_TAIL_SECS: f32 = 0.25;

pub const GROUND_PREDICTION_VERTICAL_VY_THRESHOLD: f32 = -2.0;
pub const GROUND_PREDICTION_MIN_VY: f32 = -40.0;
pub const GROUND_PREDICTION_MAX_VY: f32 = -2.0;
pub const GROUND_PREDICTION_MIN_SWEEP_M: f32 = 1.5;
pub const GROUND_PREDICTION_MAX_SWEEP_M: f32 = 20.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AirPhase {
    #[default]
    Idle,
    Airborne,
    Landing,
}

fn move_toward(current: f32, target: f32, max_delta: f32) -> f32 {
    let diff = target - current;
    if diff.abs() <= max_delta {
        target
    } else {
        current + diff.signum() * max_delta
    }
}

fn clamp01(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AlsAirState {
    pub phase: AirPhase,
    pub ground_prediction: Option<GroundPrediction>,
    pub landing_heavy: bool,
    pub landing_time: f32,
    pub air_weight: f32,
    pub fall_fast_weight: f32,
    pub land_weight: f32,
    pub land_heavy_weight: f32,
    pub down_vy_smooth: f32,
    pub last_air_vertical_velocity: f32,
    pub last_impact_vy: f32,
}

#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct AlsPreImpactVelocity {
    pub last_air_vertical_velocity: f32,
}

impl Default for AlsAirState {
    fn default() -> Self {
        Self {
            phase: AirPhase::Idle,
            ground_prediction: None,
            landing_heavy: false,
            landing_time: 0.0,
            air_weight: 0.0,
            fall_fast_weight: 0.0,
            land_weight: 0.0,
            land_heavy_weight: 0.0,
            down_vy_smooth: 0.0,
            last_air_vertical_velocity: 0.0,
            last_impact_vy: 0.0,
        }
    }
}

impl AlsAirState {
    pub fn update(&mut self, loco: &LocomotionState, vy: f32, dt: f32) {
        self.update_with_pre_impact_velocity(loco, vy, None, dt);
    }

    pub fn update_with_pre_impact_velocity(
        &mut self,
        loco: &LocomotionState,
        vy: f32,
        pre_impact_velocity: Option<f32>,
        dt: f32,
    ) {
        let dt = dt.max(0.0);
        if *loco == LocomotionState::Custom {
            self.phase = AirPhase::Idle;
            self.last_impact_vy = 0.0;
        }
        match self.phase {
            AirPhase::Idle => {
                if *loco == LocomotionState::Air {
                    self.phase = AirPhase::Airborne;
                    self.down_vy_smooth = (-vy).max(0.0);
                    self.last_air_vertical_velocity = vy;
                }
            }
            AirPhase::Airborne => {
                if *loco == LocomotionState::Grounded {
                    let pre_impact_velocity =
                        pre_impact_velocity.unwrap_or(self.last_air_vertical_velocity);
                    self.last_impact_vy = (-pre_impact_velocity).max(0.0);
                    self.landing_heavy = self.last_impact_vy >= LAND_HEAVY_IMPACT_VY;
                    self.landing_time = 0.0;
                    self.phase = AirPhase::Landing;
                } else {
                    self.last_air_vertical_velocity = vy;
                    let target = (-vy).max(0.0);
                    self.down_vy_smooth += (target - self.down_vy_smooth) * IMPACT_VY_SMOOTH_ALPHA;
                }
            }
            AirPhase::Landing => {
                if *loco == LocomotionState::Air {
                    self.landing_time = 0.0;
                    self.phase = AirPhase::Airborne;
                    self.down_vy_smooth = (-vy).max(0.0);
                    self.last_air_vertical_velocity = vy;
                }
            }
        }

        match self.phase {
            AirPhase::Idle => {
                self.air_weight = move_toward(self.air_weight, 0.0, AIR_BLEND_SPEED * dt);
                self.fall_fast_weight =
                    move_toward(self.fall_fast_weight, 0.0, AIR_BLEND_SPEED * dt);
                self.land_weight = move_toward(self.land_weight, 0.0, LAND_BLEND_SPEED * dt);
                self.land_heavy_weight =
                    move_toward(self.land_heavy_weight, 0.0, LAND_BLEND_SPEED * dt);
            }
            AirPhase::Airborne => {
                self.air_weight = move_toward(self.air_weight, 1.0, AIR_BLEND_SPEED * dt);
                let target = clamp01((-vy - FALL_FAST_ENTER_VY) / FALL_FAST_SPAN_VY);
                self.fall_fast_weight =
                    move_toward(self.fall_fast_weight, target, AIR_BLEND_SPEED * dt);
                self.land_weight = move_toward(self.land_weight, 0.0, LAND_BLEND_SPEED * dt);
                self.land_heavy_weight =
                    move_toward(self.land_heavy_weight, 0.0, LAND_BLEND_SPEED * dt);
            }
            AirPhase::Landing => {
                let recovery = if self.landing_heavy {
                    LAND_RECOVERY_HEAVY_SECS
                } else {
                    LAND_RECOVERY_LIGHT_SECS
                };
                self.landing_time += dt;
                let remaining = (recovery - self.landing_time).max(0.0);
                self.air_weight = move_toward(self.air_weight, 0.0, AIR_BLEND_SPEED * dt);
                self.fall_fast_weight =
                    move_toward(self.fall_fast_weight, 0.0, AIR_BLEND_SPEED * dt);
                let land_target = if remaining > LAND_TAIL_SECS { 1.0 } else { 0.0 };
                self.land_weight =
                    move_toward(self.land_weight, land_target, LAND_BLEND_SPEED * dt);
                let heavy_target = if self.landing_heavy { 1.0 } else { 0.0 };
                self.land_heavy_weight =
                    move_toward(self.land_heavy_weight, heavy_target, LAND_BLEND_SPEED * dt);
                if remaining <= 0.0 && *loco == LocomotionState::Grounded {
                    self.phase = AirPhase::Idle;
                }
            }
        }
    }

    pub fn landing_vertical_velocity_world_space(&self) -> Option<f32> {
        (self.phase == AirPhase::Landing).then_some(-self.last_impact_vy)
    }
}

pub fn land_recovery_secs(heavy: bool) -> f32 {
    if heavy {
        LAND_RECOVERY_HEAVY_SECS
    } else {
        LAND_RECOVERY_LIGHT_SECS
    }
}

type AirQueryItem<'w, 's> = (
    Entity,
    &'w LocomotionState,
    &'w LinearVelocity,
    &'w GlobalTransform,
    Option<&'s mut AlsAirState>,
    Option<&'w AlsPreImpactVelocity>,
    Option<&'w crate::character::AlsMovementParamsHandle>,
);
#[allow(clippy::type_complexity)]
fn air_state_system(
    time: Res<Time>,
    spatial: SpatialQuery,
    mut q: Query<AirQueryItem, With<AlsCharacter>>,
    params_assets: Res<Assets<crate::params::AlsMovementParams>>,
    crouched: Query<Has<crate::motion::AlsCapsuleCrouched>>,
) {
    let dt = time.delta_secs();
    for (entity, loco, vel, transform, state, pre_impact, handle) in &mut q {
        if let Some(mut s) = state {
            s.update_with_pre_impact_velocity(
                loco,
                vel.y,
                pre_impact.map(|record| record.last_air_vertical_velocity),
                dt,
            );
            let half_height = handle
                .and_then(|h| params_assets.get(h.id()))
                .map(|p| {
                    let standing = p.crouch.standing_capsule_half_height;
                    let crouched_h = p.crouch.crouched_capsule_half_height;
                    crouched
                        .get(entity)
                        .unwrap_or(false)
                        .then_some(crouched_h)
                        .unwrap_or(standing)
                })
                .unwrap_or(0.9);
            s.ground_prediction = Some(ground_prediction_sweep(
                &spatial,
                entity,
                transform.translation(),
                **vel,
                half_height,
                0.3,
            ));
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroundPrediction {
    pub amount: f32,
    pub hit_fraction: Option<f32>,
    pub sweep_start: Option<Vec3>,
    pub sweep_end: Option<Vec3>,
    pub capsule_half_height: f32,
    pub capsule_radius: f32,
    pub ground_valid: bool,
}

pub const WALKABLE_FLOOR_COS: f32 = 0.71;

pub fn ground_prediction_amount(hit_fraction: f32) -> f32 {
    let time = hit_fraction.clamp(0.0, 1.0);
    let time_squared = time * time;
    let time_cubed = time_squared * time;
    let h00 = 2.0 * time_cubed - 3.0 * time_squared + 1.0;
    let h10 = time_cubed - 2.0 * time_squared + time;
    (h00 - 2.066_088_4 * h10).clamp(0.0, 1.0)
}

pub fn ground_prediction_sweep(
    spatial: &SpatialQuery,
    excluded: Entity,
    origin: Vec3,
    velocity: Vec3,
    capsule_half_height: f32,
    capsule_radius: f32,
) -> GroundPrediction {
    let mut result = GroundPrediction {
        amount: 0.0,
        hit_fraction: None,
        sweep_start: None,
        sweep_end: None,
        capsule_half_height,
        capsule_radius,
        ground_valid: false,
    };
    if velocity.y > GROUND_PREDICTION_VERTICAL_VY_THRESHOLD {
        return result;
    }
    let Ok(dir) = Dir3::new(
        Vec3::new(
            velocity.x,
            velocity
                .y
                .clamp(GROUND_PREDICTION_MIN_VY, GROUND_PREDICTION_MAX_VY),
            velocity.z,
        )
        .normalize_or_zero(),
    ) else {
        return result;
    };
    let vertical_velocity = velocity
        .y
        .clamp(GROUND_PREDICTION_MIN_VY, GROUND_PREDICTION_MAX_VY);
    let distance = GROUND_PREDICTION_MIN_SWEEP_M
        + (vertical_velocity - GROUND_PREDICTION_MAX_VY)
            / (GROUND_PREDICTION_MIN_VY - GROUND_PREDICTION_MAX_VY)
            * (GROUND_PREDICTION_MAX_SWEEP_M - GROUND_PREDICTION_MIN_SWEEP_M);
    let filter = SpatialQueryFilter::from_excluded_entities([excluded]);
    let shape = als_character_capsule(capsule_half_height, capsule_radius);
    let config = ShapeCastConfig::from_max_distance(distance);
    let hit = spatial.cast_shape(&shape, origin, Quat::IDENTITY, dir, &config, &filter);
    result.sweep_start = Some(origin);
    match hit {
        Some(h) => {
            result.sweep_end = Some(origin + dir * h.distance);
            result.ground_valid = h.normal1.y >= WALKABLE_FLOOR_COS;
            if result.ground_valid {
                let time_frac = (h.distance / distance).clamp(0.0, 1.0);
                result.hit_fraction = Some(time_frac);
                result.amount = ground_prediction_amount(time_frac);
            }
        }
        None => {
            result.sweep_end = Some(origin + dir * distance);
        }
    }
    result
}

pub struct AlsAirPlugin;

impl Plugin for AlsAirPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (ensure_air_state, air_state_system.after(ensure_air_state).after(crate::motion::locomotion_state_sync_system)),
        )
        .add_systems(
            Update,
            record_pre_impact_velocity_system.before(TnuaPipelineSystems::Sensors),
        );
    }
}

fn record_pre_impact_velocity_system(
    mut q: Query<
        (&LocomotionState, &LinearVelocity, &mut AlsPreImpactVelocity),
        With<AlsCharacter>,
    >,
) {
    for (locomotion, velocity, mut record) in &mut q {
        if *locomotion == LocomotionState::Air {
            record.last_air_vertical_velocity = velocity.y;
        }
    }
}

fn ensure_air_state(
    mut commands: Commands,
    q: Query<Entity, (With<AlsCharacter>, Without<AlsAirState>)>,
) {
    for entity in &q {
        commands
            .entity(entity)
            .insert((AlsAirState::default(), AlsPreImpactVelocity::default()));
    }
}

#[cfg(test)]
mod tests {
    use avian3d::prelude::{Collider, PhysicsPlugins, PhysicsSystems, Position, RigidBody};
    use bevy::asset::AssetEvent;
    use bevy::ecs::system::RunSystemOnce;

    use super::*;
    use crate::state::Stance;

    const DT: f32 = 1.0 / 60.0;

    #[test]
    fn custom_mode_does_not_turn_an_interrupted_fall_into_a_landing() {
        for landed in [false, true] {
            let mut state = AlsAirState::default();
            state.update(&LocomotionState::Air, -10.0, DT);
            if landed {
                state.update(&LocomotionState::Grounded, 0.0, DT);
            }
            state.update(&LocomotionState::Custom, -10.0, DT);
            state.update(&LocomotionState::Grounded, 0.0, DT);
            assert_eq!(state.phase, AirPhase::Idle);
            assert_eq!(state.last_impact_vy, 0.0);
            state.update(&LocomotionState::Air, -10.0, DT);
            state.update(&LocomotionState::Grounded, 0.0, DT);
            assert_eq!(state.phase, AirPhase::Landing);
            assert_eq!(state.last_impact_vy, 10.0);
        }
    }

    #[derive(Component)]
    struct CapsuleSweepProbeOrigin;

    #[derive(Resource, Default)]
    struct CapsuleSweepProbe {
        ray_hit: bool,
        prediction: Option<GroundPrediction>,
    }

    fn record_capsule_sweep_probe(
        spatial: SpatialQuery,
        origins: Query<(Entity, &GlobalTransform), With<CapsuleSweepProbeOrigin>>,
        mut probe: ResMut<CapsuleSweepProbe>,
    ) {
        let Ok((entity, transform)) = origins.single() else {
            return;
        };
        let filter = SpatialQueryFilter::from_excluded_entities([entity]);
        probe.ray_hit = spatial
            .cast_ray(transform.translation(), Dir3::NEG_Y, 20.0, true, &filter)
            .is_some();
        probe.prediction = Some(ground_prediction_sweep(
            &spatial,
            entity,
            transform.translation(),
            Vec3::new(0.0, -10.0, 0.0),
            0.9,
            0.3,
        ));
    }

    #[test]
    fn ground_prediction_capsule_sweep_hits_an_edge_missed_by_center_ray() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, PhysicsPlugins::new(Update)))
            .init_resource::<Assets<Mesh>>()
            .add_message::<AssetEvent<Mesh>>()
            .init_resource::<CapsuleSweepProbe>()
            .add_systems(
                Update,
                record_capsule_sweep_probe.after(PhysicsSystems::Last),
            );
        app.finish();
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(0.05, 0.1, 0.5),
            Position(Vec3::new(0.1, 0.0, 0.0)),
            Transform::from_xyz(0.1, 0.0, 0.0),
        ));
        app.world_mut()
            .spawn((CapsuleSweepProbeOrigin, Transform::from_xyz(0.0, 2.0, 0.0)));
        for _ in 0..3 {
            app.update();
        }
        let probe = app.world().resource::<CapsuleSweepProbe>();
        let prediction = probe.prediction.unwrap();
        let passed = !probe.ray_hit
            && prediction.ground_valid
            && prediction.hit_fraction.is_some()
            && prediction.capsule_half_height == 0.9
            && prediction.capsule_radius == 0.3;
        let verdict = if passed { "PASS" } else { "FAIL" };
        println!(
            "[BF102.32][VERDICT] ray_hit={} capsule_hit={} hit_fraction={:?} half={:.3} radius={:.3} -> {verdict}",
            probe.ray_hit,
            prediction.ground_valid,
            prediction.hit_fraction,
            prediction.capsule_half_height,
            prediction.capsule_radius,
        );
        assert!(passed);
    }

    #[test]
    fn fixed_pre_impact_record_keeps_landing_roll_deterministic() {
        let mut world = World::new();
        let entity = world
            .spawn((
                AlsCharacter,
                LocomotionState::Air,
                LinearVelocity(Vec3::new(0.0, -7.2, 0.0)),
                AlsPreImpactVelocity::default(),
            ))
            .id();
        world
            .run_system_once(record_pre_impact_velocity_system)
            .unwrap();
        let fixed_record = world
            .get::<AlsPreImpactVelocity>(entity)
            .unwrap()
            .last_air_vertical_velocity;
        let mut impacts = Vec::new();
        for render_updates in [1, 4] {
            let mut air = AlsAirState::default();
            for _ in 0..render_updates {
                air.update_with_pre_impact_velocity(&LocomotionState::Air, -1.0, None, 1.0 / 120.0);
            }
            air.update_with_pre_impact_velocity(
                &LocomotionState::Grounded,
                0.0,
                Some(fixed_record),
                1.0 / 60.0,
            );
            impacts.push(air.last_impact_vy);
        }
        let mut roll_30 = crate::roll::AlsRollState::default();
        let mut roll_120 = crate::roll::AlsRollState::default();
        let roll_30_started = roll_30
            .start_from_landing(impacts[0], Vec3::NEG_Z, 0.0, Stance::Standing)
            .started;
        let roll_120_started = roll_120
            .start_from_landing(impacts[1], Vec3::NEG_Z, 0.0, Stance::Standing)
            .started;
        let passed = (fixed_record + 7.2).abs() < 1e-6
            && (impacts[0] - 7.2).abs() < 1e-6
            && (impacts[1] - 7.2).abs() < 1e-6
            && roll_30_started
            && roll_120_started;
        let verdict = if passed { "PASS" } else { "FAIL" };
        println!(
            "[BF102.36][VERDICT] fixed_record={fixed_record:.3} render30_impact={:.3} render120_impact={:.3} roll30={} roll120={} -> {verdict}",
            impacts[0], impacts[1], roll_30_started, roll_120_started,
        );
        assert!(passed);
    }

    #[test]
    fn grounded_idle_stays_all_zero() {
        let mut s = AlsAirState::default();
        s.update(&LocomotionState::Grounded, 0.0, DT);
        assert_eq!(s.phase, AirPhase::Idle);
        assert_eq!(s.air_weight, 0.0);
        assert_eq!(s.land_weight, 0.0);
    }

    #[test]
    fn takeoff_enters_airborne_and_ramps_air_weight() {
        let mut s = AlsAirState::default();
        s.update(&LocomotionState::Air, 4.2, DT);
        assert_eq!(s.phase, AirPhase::Airborne);
        for _ in 0..12 {
            s.update(&LocomotionState::Air, 4.2, DT);
        }
        assert!((s.air_weight - 1.0).abs() < 1e-3);
        assert_eq!(s.fall_fast_weight, 0.0);
    }

    #[test]
    fn fall_fast_blends_in_beyond_threshold() {
        let mut s = AlsAirState::default();
        s.phase = AirPhase::Airborne;
        s.air_weight = 1.0;
        for _ in 0..30 {
            s.update(&LocomotionState::Air, -9.5, DT);
        }
        let expected = clamp01((9.5 - FALL_FAST_ENTER_VY) / FALL_FAST_SPAN_VY);
        assert!((s.fall_fast_weight - expected).abs() < 1e-3);
        assert!(expected > 0.0 && expected < 1.0);
        for _ in 0..60 {
            s.update(&LocomotionState::Air, -20.0, DT);
        }
        assert!((s.fall_fast_weight - 1.0).abs() < 1e-3);
    }

    #[test]
    fn light_landing_selected_below_impact_threshold() {
        let mut s = AlsAirState::default();
        s.phase = AirPhase::Airborne;
        s.air_weight = 1.0;
        s.down_vy_smooth = 4.5;
        s.last_air_vertical_velocity = -4.5;
        s.update(&LocomotionState::Grounded, 0.0, DT);
        assert_eq!(s.phase, AirPhase::Landing);
        assert!(!s.landing_heavy);
        assert!((s.last_impact_vy - 4.5).abs() < 1e-5);
        let recovery = land_recovery_secs(false);
        for _ in 0..((recovery / DT) as usize + 2) {
            s.update(&LocomotionState::Grounded, 0.0, DT);
        }
        assert_eq!(s.phase, AirPhase::Idle);
        assert!(s.land_weight.abs() < 1e-2);
    }

    #[test]
    fn heavy_landing_selected_at_high_impact() {
        let mut s = AlsAirState::default();
        s.phase = AirPhase::Airborne;
        s.down_vy_smooth = 11.7;
        s.last_air_vertical_velocity = -11.7;
        s.update(&LocomotionState::Grounded, 0.0, DT);
        assert!(s.landing_heavy);
        assert!((s.last_impact_vy - 11.7).abs() < 1e-5);
        for _ in 0..10 {
            s.update(&LocomotionState::Grounded, 0.0, DT);
        }
        assert!(s.land_heavy_weight > 0.5);
        assert!(s.land_weight > 0.5);
    }

    #[test]
    fn landing_latches_the_last_airborne_vertical_velocity() {
        let mut s = AlsAirState::default();
        s.update(&LocomotionState::Air, -10.0, DT);
        s.update(&LocomotionState::Grounded, 0.0, DT);
        assert_eq!(s.phase, AirPhase::Landing);
        assert!((s.last_air_vertical_velocity + 10.0).abs() < 1e-6);
        assert!((s.last_impact_vy - 10.0).abs() < 1e-6);
        assert_eq!(s.landing_vertical_velocity_world_space(), Some(-10.0));
        assert!(s.landing_heavy);
    }

    #[test]
    fn jump_during_landing_returns_to_airborne() {
        let mut s = AlsAirState::default();
        s.phase = AirPhase::Landing;
        s.landing_time = 0.2;
        s.land_weight = 0.8;
        s.update(&LocomotionState::Air, 4.2, DT);
        assert_eq!(s.phase, AirPhase::Airborne);
        assert_eq!(s.landing_time, 0.0);
        for _ in 0..15 {
            s.update(&LocomotionState::Air, 4.2, DT);
        }
        assert!((s.air_weight - 1.0).abs() < 1e-2);
        assert!(s.land_weight < 0.5);
    }

    #[test]
    fn source_walkable_floor_cosine_is_preserved() {
        assert!((WALKABLE_FLOOR_COS - 0.71).abs() < 1e-6);
    }

    #[test]
    fn ground_prediction_curve_matches_the_exported_source_samples() {
        assert!((ground_prediction_amount(0.0) - 1.0).abs() < 1e-6);
        assert!((ground_prediction_amount(0.25) - 0.553_206_3).abs() < 1e-6);
        assert!((ground_prediction_amount(0.5) - 0.241_738_95).abs() < 1e-6);
        assert!((ground_prediction_amount(0.75) - 0.059_402_11).abs() < 1e-6);
        assert!(ground_prediction_amount(1.0).abs() < 1e-6);
    }

    #[test]
    fn ground_prediction_source_velocity_contract_is_preserved() {
        assert!((GROUND_PREDICTION_VERTICAL_VY_THRESHOLD + 2.0).abs() < 1e-6);
        assert!((GROUND_PREDICTION_MIN_VY + 40.0).abs() < 1e-6);
        assert!((GROUND_PREDICTION_MAX_VY + 2.0).abs() < 1e-6);
        assert!((GROUND_PREDICTION_MIN_SWEEP_M - 1.5).abs() < 1e-6);
        assert!((GROUND_PREDICTION_MAX_SWEEP_M - 20.0).abs() < 1e-6);
    }
}
