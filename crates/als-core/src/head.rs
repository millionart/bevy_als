use crate::state::RotationMode;
use bevy::prelude::*;

const COUNTER_CLOCKWISE_ROTATION_ANGLE_THRESHOLD: f32 = 5.0;
const LOCOMOTION_YAW_SPEED_THRESHOLD: f32 = 20.0;
const SWITCH_SIDES_CURRENT_YAW_THRESHOLD: f32 = 90.0;
const SWITCH_SIDES_TARGET_YAW_THRESHOLD: f32 = 160.0;
const SWITCH_SIDES_COMPLETE_YAW_DELTA: f32 = 10.0;

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct AlsHeadState {
    pub initialization_required: bool,
    pub switching_look_sides: bool,
    pub pitch_angle: f32,
    pub yaw_angle: f32,
    pub yaw_velocity: f32,
    pub yaw_amount: f32,
}

impl Default for AlsHeadState {
    fn default() -> Self {
        Self {
            initialization_required: true,
            switching_look_sides: false,
            pitch_angle: 0.0,
            yaw_angle: 0.0,
            yaw_velocity: 0.0,
            yaw_amount: 0.5,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AlsHeadSettings {
    pub pitch_half_life: f32,
    pub yaw_half_life: f32,
    pub switch_look_sides_yaw_half_life: f32,
    pub first_person_pitch_half_life: f32,
    pub first_person_yaw_half_life: f32,
}

impl Default for AlsHeadSettings {
    fn default() -> Self {
        Self {
            pitch_half_life: 0.1,
            yaw_half_life: 0.1,
            switch_look_sides_yaw_half_life: 0.2,
            first_person_pitch_half_life: 0.01,
            first_person_yaw_half_life: 0.01,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AlsHeadUpdateInput {
    pub delta_seconds: f32,
    pub delta_seconds_without_time_dilation: f32,
    pub first_person: bool,
    pub rotation_mode: RotationMode,
    pub has_input: bool,
    pub input_yaw_world: f32,
    pub target_yaw_world: f32,
    pub body_yaw_world: f32,
    pub body_yaw_velocity_world: f32,
    pub view_yaw_angle: f32,
    pub view_pitch_angle: f32,
    pub movement_base_relative_yaw: f32,
}

pub fn unwind_degrees(mut angle: f32) -> f32 {
    while angle > 180.0 {
        angle -= 360.0;
    }
    while angle < -180.0 {
        angle += 360.0;
    }
    angle
}

pub use crate::animation::inv_exp_approx;

pub fn damper_exact_angle(current: f32, target: f32, delta_seconds: f32, half_life: f32) -> f32 {
    let mut delta = unwind_degrees(target - current);
    if delta.abs() <= 1e-4 {
        return target;
    }
    if delta > 175.0 {
        delta -= 360.0;
    }
    let alpha =
        1.0 - inv_exp_approx(std::f32::consts::LN_2 * delta_seconds.max(0.0) / (half_life + 1e-8));
    unwind_degrees(current + delta * alpha)
}

pub fn critical_spring_damper(
    current: f32,
    velocity: &mut f32,
    target: f32,
    delta_seconds: f32,
    half_life: f32,
) -> f32 {
    let smoothing_time = half_life / std::f32::consts::LN_2;
    if smoothing_time < 1e-8 {
        *velocity = 0.0;
        return target;
    }
    let y = 2.0 / smoothing_time.max(1e-8);
    let j0 = current - target;
    let j1 = *velocity + j0 * y;
    let e = inv_exp_approx(y * delta_seconds.max(0.0));
    let result = e * (j0 + j1 * delta_seconds.max(0.0)) + target;
    *velocity = e * (*velocity - j1 * y * delta_seconds.max(0.0));
    result
}

pub fn update_head_state(
    state: &mut AlsHeadState,
    input: AlsHeadUpdateInput,
    settings: AlsHeadSettings,
) {
    if input.movement_base_relative_yaw != 0.0 {
        state.yaw_angle += input.movement_base_relative_yaw;
    }
    state.yaw_angle -= input.body_yaw_velocity_world * input.delta_seconds;
    state.yaw_angle = state.yaw_angle.clamp(-180.0, 180.0);
    let mut ready_to_switch_look_sides = false;
    let (target_pitch, target_yaw) = if input.rotation_mode == RotationMode::VelocityDirection {
        let mut target_yaw = unwind_degrees(
            (if input.has_input {
                input.input_yaw_world
            } else {
                input.target_yaw_world
            }) - input.body_yaw_world,
        );
        if input.body_yaw_velocity_world.abs() > LOCOMOTION_YAW_SPEED_THRESHOLD {
            target_yaw = input.body_yaw_velocity_world.signum() * target_yaw.abs();
        } else if target_yaw.abs() > 180.0 - COUNTER_CLOCKWISE_ROTATION_ANGLE_THRESHOLD {
            target_yaw = state.yaw_angle;
        }
        state.switching_look_sides = false;
        (0.0, target_yaw)
    } else {
        let mut target_yaw = input.view_yaw_angle.clamp(
            -180.0 + COUNTER_CLOCKWISE_ROTATION_ANGLE_THRESHOLD,
            180.0 - COUNTER_CLOCKWISE_ROTATION_ANGLE_THRESHOLD,
        );
        if !input.first_person {
            if state.yaw_angle >= SWITCH_SIDES_CURRENT_YAW_THRESHOLD
                && target_yaw <= -SWITCH_SIDES_TARGET_YAW_THRESHOLD
            {
                target_yaw = 180.0 - COUNTER_CLOCKWISE_ROTATION_ANGLE_THRESHOLD;
                ready_to_switch_look_sides = !input.has_input;
            } else if state.yaw_angle <= -SWITCH_SIDES_CURRENT_YAW_THRESHOLD
                && target_yaw >= SWITCH_SIDES_TARGET_YAW_THRESHOLD
            {
                target_yaw = -180.0 + COUNTER_CLOCKWISE_ROTATION_ANGLE_THRESHOLD;
                ready_to_switch_look_sides = !input.has_input;
            }
        }
        (input.view_pitch_angle, target_yaw)
    };
    if state.initialization_required {
        state.initialization_required = false;
        state.pitch_angle = target_pitch;
        state.yaw_angle = target_yaw;
        state.yaw_velocity = 0.0;
        state.switching_look_sides = false;
    } else {
        let effective_delta_seconds = if input.first_person {
            input.delta_seconds_without_time_dilation
        } else {
            input.delta_seconds
        };
        state.pitch_angle = damper_exact_angle(
            state.pitch_angle,
            target_pitch,
            effective_delta_seconds,
            if input.first_person {
                settings.first_person_pitch_half_life
            } else {
                settings.pitch_half_life
            },
        );
        if ready_to_switch_look_sides {
            state.switching_look_sides = true;
        } else if unwind_degrees(target_yaw - state.yaw_angle).abs()
            <= SWITCH_SIDES_COMPLETE_YAW_DELTA
        {
            state.switching_look_sides = false;
        }
        state.yaw_angle = critical_spring_damper(
            state.yaw_angle,
            &mut state.yaw_velocity,
            target_yaw,
            input.delta_seconds,
            if state.switching_look_sides {
                settings.switch_look_sides_yaw_half_life
            } else if input.first_person {
                settings.first_person_yaw_half_life
            } else {
                settings.yaw_half_life
            },
        );
    }
    state.yaw_amount = state.yaw_angle / 360.0 + 0.5;
}
