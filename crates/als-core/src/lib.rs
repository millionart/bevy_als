//! ALS-Refactored equivalent ECS core: state model, motion mapping, curve infra, foot IK probes.

pub mod aim;
pub mod animation;
pub mod air;
pub mod air_jump;
pub mod bones;
pub mod character;
pub mod control_scheme;
pub mod debug_hud;
pub mod dynamics;
pub mod flail;
pub mod foot_ik;
pub mod gait;
mod generated_movement_constants;
pub mod graph_router;
pub mod head;
pub mod input;
pub mod input_request;
pub mod mantle;
pub mod motion;
pub mod movement_direction;
pub mod orientation;
pub mod overlay_blend;
pub mod params;
pub mod pivot;
pub mod player_start;
pub mod roll;
pub mod rotate_in_place;
pub mod start_stop;
pub mod state;
pub mod stride_curves;
pub mod transition;
pub mod transitions;
pub mod turn_in_place;
pub mod view;

pub use state::{Gait, LocomotionState, OverlayState, RotationMode, Stance};
