use std::num::NonZeroU8;

use als_camera::{AlsCameraParams, field_of_view_degrees};
use als_core::input::{ROLL_TAP_SECS, SPRINT_HOLD_SECS, roll_pressed_from_events};
use als_core::mantle::MantleKind;
use als_core::motion::{JumpChain, jump_chain, view_relative_move_direction};
use als_core::state::{LocomotionState, OverlayState, Stance};
use bevy::prelude::*;
use bevy_als::overlay_mode_menu::{COLOR_OPTION_NOT_SELECTED, COLOR_OPTION_SELECTED};
use bevy_als::overlay_object::{
    LEFT_GUN_BONE, OverlayObjectKind, RIGHT_GUN_BONE, attachment_bone, overlay_object_spec,
    overlay_object_visible,
};
use bevy_enhanced_input::prelude::ActionEvents;

fn main() {
    App::new()
        .add_plugins(MinimalPlugins)
        .add_systems(Startup, verify_static_contract)
        .run();
}

fn camera_params() -> AlsCameraParams {
    AlsCameraParams {
        ignore_time_dilation: true,
        teleport_distance_threshold: 2.0,
        trace_radius: 0.15,
        trace_override_offset: Vec3::Y * 0.4,
        trace_distance_smoothing: true,
        trace_distance_smoothing_half_life: 0.2,
        third_person_field_of_view_deg: 90.0,
        first_person_field_of_view_deg: 90.0,
        field_of_view_min_deg: 5.0,
        field_of_view_max_deg: 175.0,
    }
}

fn verify_static_contract(mut exit: MessageWriter<AppExit>) {
    let camera = camera_params();
    let camera_basis = [
        (0.0, Vec3::NEG_Z),
        (std::f32::consts::FRAC_PI_2, Vec3::NEG_X),
        (-std::f32::consts::FRAC_PI_2, Vec3::X),
        (std::f32::consts::PI, Vec3::Z),
    ]
    .into_iter()
    .all(|(yaw, expected)| view_relative_move_direction(Vec2::Y, yaw).distance(expected) < 1e-5);
    let tap_hold = !roll_pressed_from_events(ActionEvents::START | ActionEvents::ONGOING)
        && roll_pressed_from_events(ActionEvents::FIRE)
        && ROLL_TAP_SECS < SPRINT_HOLD_SECS;
    let jump_priority = jump_chain(LocomotionState::Ragdolling, Stance::Crouching, true)
        == JumpChain::ExitRagdoll
        && jump_chain(LocomotionState::Grounded, Stance::Standing, true) == JumpChain::Mantle
        && jump_chain(LocomotionState::Grounded, Stance::Crouching, false) == JumpChain::StandUp
        && jump_chain(LocomotionState::Air, Stance::Standing, false) == JumpChain::Jump;
    let fov_contract = (field_of_view_degrees(false, 12.0, &camera) - 102.0).abs() < 1e-5
        && (field_of_view_degrees(true, 12.0, &camera) - 90.0).abs() < 1e-5
        && (field_of_view_degrees(false, 500.0, &camera) - 175.0).abs() < 1e-5;
    let rifle = overlay_object_spec(OverlayState::Rifle).unwrap();
    let torch = overlay_object_spec(OverlayState::Torch).unwrap();
    let overlay_contract = rifle.kind == OverlayObjectKind::Skeletal
        && torch.kind == OverlayObjectKind::Static
        && attachment_bone(rifle, true) == RIGHT_GUN_BONE
        && attachment_bone(torch, true) == LEFT_GUN_BONE
        && !overlay_object_visible(
            OverlayState::Rifle,
            LocomotionState::Grounded,
            true,
            Some(MantleKind::High),
        )
        && overlay_object_visible(
            OverlayState::Rifle,
            LocomotionState::Grounded,
            true,
            Some(MantleKind::LowLeft),
        )
        && !overlay_object_visible(
            OverlayState::Rifle,
            LocomotionState::Ragdolling,
            false,
            None,
        );
    let selected = COLOR_OPTION_SELECTED.to_linear();
    let unselected = COLOR_OPTION_NOT_SELECTED.to_linear();
    let overlay_highlight = selected.red / unselected.red >= 9.9;
    let pass = camera_basis
        && tap_hold
        && jump_priority
        && fov_contract
        && overlay_contract
        && overlay_highlight;
    println!(
        "[BCHAR99][STATIC] camera_basis={camera_basis} tap_hold={tap_hold} jump_priority={jump_priority} fov_contract={fov_contract} overlay_contract={overlay_contract} overlay_highlight={overlay_highlight} -> {}",
        if pass { "PASS" } else { "FAIL" }
    );
    if pass {
        exit.write(AppExit::Success);
    } else {
        exit.write(AppExit::Error(NonZeroU8::new(18).unwrap()));
    }
}
