use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;

use crate::character::AlsCharacter;
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
    pub landing_heavy: bool,
    pub landing_time: f32,
    pub air_weight: f32,
    pub fall_fast_weight: f32,
    pub land_weight: f32,
    pub land_heavy_weight: f32,
    pub down_vy_smooth: f32,
    pub last_impact_vy: f32,
}

impl Default for AlsAirState {
    fn default() -> Self {
        Self {
            phase: AirPhase::Idle,
            landing_heavy: false,
            landing_time: 0.0,
            air_weight: 0.0,
            fall_fast_weight: 0.0,
            land_weight: 0.0,
            land_heavy_weight: 0.0,
            down_vy_smooth: 0.0,
            last_impact_vy: 0.0,
        }
    }
}

impl AlsAirState {
    pub fn update(&mut self, loco: &LocomotionState, vy: f32, dt: f32) {
        let dt = dt.max(0.0);
        match self.phase {
            AirPhase::Idle => {
                if *loco == LocomotionState::Air {
                    self.phase = AirPhase::Airborne;
                    self.down_vy_smooth = (-vy).max(0.0);
                }
            }
            AirPhase::Airborne => {
                if *loco == LocomotionState::Grounded {
                    self.last_impact_vy = self.down_vy_smooth;
                    self.landing_heavy = self.last_impact_vy >= LAND_HEAVY_IMPACT_VY;
                    self.landing_time = 0.0;
                    self.phase = AirPhase::Landing;
                } else {
                    let target = (-vy).max(0.0);
                    self.down_vy_smooth += (target - self.down_vy_smooth) * IMPACT_VY_SMOOTH_ALPHA;
                }
            }
            AirPhase::Landing => {
                if *loco == LocomotionState::Air {
                    self.landing_time = 0.0;
                    self.phase = AirPhase::Airborne;
                    self.down_vy_smooth = (-vy).max(0.0);
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
}

pub fn land_recovery_secs(heavy: bool) -> f32 {
    if heavy {
        LAND_RECOVERY_HEAVY_SECS
    } else {
        LAND_RECOVERY_LIGHT_SECS
    }
}

type AirQueryItem<'w, 's> = (
    &'w LocomotionState,
    &'w LinearVelocity,
    Option<&'s mut AlsAirState>,
);
fn air_state_system(time: Res<Time>, mut q: Query<AirQueryItem, With<AlsCharacter>>) {
    let dt = time.delta_secs();
    for (loco, vel, state) in &mut q {
        if let Some(mut s) = state {
            s.update(loco, vel.y, dt);
        }
    }
}

pub struct AlsAirPlugin;

impl Plugin for AlsAirPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (ensure_air_state, air_state_system.after(ensure_air_state)),
        );
    }
}

fn ensure_air_state(
    mut commands: Commands,
    q: Query<Entity, (With<AlsCharacter>, Without<AlsAirState>)>,
) {
    for entity in &q {
        commands.entity(entity).insert(AlsAirState::default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

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
    fn contact_frame_velocity_spike_is_filtered_by_smoothing() {
        let mut s = AlsAirState::default();
        s.update(&LocomotionState::Air, -19.6, DT);
        assert!((s.down_vy_smooth - 19.6).abs() < 1e-4);
        for _ in 0..20 {
            s.update(&LocomotionState::Air, -5.0, DT);
        }
        assert!(
            (s.down_vy_smooth - 5.0).abs() < 1.0,
            "ema converges toward sustained fall speed"
        );
        s.update(&LocomotionState::Air, -8.1, DT);
        let pre_edge = s.down_vy_smooth;
        s.update(&LocomotionState::Grounded, 0.06, DT);
        assert!(
            !s.landing_heavy && (pre_edge - s.last_impact_vy).abs() < 1e-4,
            "single-frame spike must not classify as heavy"
        );
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
}
