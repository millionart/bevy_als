//! als-core：ALS-Refactored 的 Bevy ECS 核心状态模型。
//!
//! M0.1 引导占位；M1 起落地 `LocomotionState` / `RotationMode` / `Stance` /
//! `Gait` / `OverlayState` 等状态组件（取值对照 ALS GameplayTags 核对后定稿）。

pub(crate) mod placeholder {
    /// 占位：M1 将替换为真实状态组件集。
    pub const CRATE_ROLE: &str = "als-core: ALS state model (ECS)";
}

pub use placeholder::CRATE_ROLE;
