//! als-core：ALS-Refactored 的 Bevy ECS 核心状态模型。
//!
//! M1 起落地 `LocomotionState` / `RotationMode` / `Stance` / `Gait` /
//! `OverlayState` 等状态组件（取值对照 ALS GameplayTags 核对后定稿）。

pub mod input;
pub mod params;
pub mod state;
pub mod transition;

pub use state::{Gait, LocomotionState, OverlayState, RotationMode, Stance};
