//! ALS-Refactored equivalent ECS core: state model, motion mapping, curve infra, foot IK probes.

pub mod aim;
pub mod air;
pub mod air_jump;
pub mod bones;
pub mod character;
pub mod control_scheme;
pub mod debug_hud;
pub mod foot_ik;
pub mod gait;
pub mod graph_router;
pub mod input;
pub mod input_request;
pub mod mantle;
pub mod movement_direction;
pub mod motion;
pub mod overlay_blend;
pub mod params;
pub mod pivot;
pub mod start_stop;
pub mod state;
pub mod roll;
pub mod rotate_in_place;
pub mod transition;
pub mod transitions;
pub mod turn_in_place;
pub mod view;

pub use state::{Gait, LocomotionState, OverlayState, RotationMode, Stance};
