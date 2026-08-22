//! ALS 状态组件（M1.1）。
//!
//! 取值对照 ALS-Refactored v4.18 `AlsGameplayTags.h` 逐一核对：
//! - `AlsLocomotionModeTags`: Grounded / InAir（Ragdoll 在 ALS 属 LocomotionAction 层，
//!   此处按 CHECKLIST M1.1 要求并入顶层 `LocomotionState::Ragdolling`）
//! - `AlsRotationModeTags`: VelocityDirection / ViewDirection / Aiming
//! - `AlsStanceTags`: Standing / Crouching
//! - `AlsGaitTags`: Walking / Running / Sprinting
//! - `AlsOverlayModeTags`: Default..Barrel 共 13 项

use bevy::prelude::*;

/// 顶层运动状态。对应 ALS 的 LocomotionMode + LocomotionAction(Ragdolling) 合并视图。
#[derive(
    Component, Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Reflect, serde::Serialize,
    serde::Deserialize,
)]
#[reflect(Default)]
pub enum LocomotionState {
    /// 对应 `AlsLocomotionModeTags.Grounded`
    #[default]
    Grounded,
    /// 对应 `AlsLocomotionModeTags.InAir`
    Air,
    /// 对应 `AlsLocomotionActionTags.Ragdolling`
    Ragdolling,
}

/// 旋转朝向模式。对应 `AlsRotationModeTags`。
#[derive(
    Component, Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Reflect, serde::Serialize,
    serde::Deserialize,
)]
#[reflect(Default)]
pub enum RotationMode {
    /// 朝向速度方向：`AlsRotationModeTags.VelocityDirection`
    #[default]
    VelocityDirection,
    /// 朝向视线水平方向：`AlsRotationModeTags.ViewDirection`
    ViewDirection,
    /// 瞄准态朝向（含俯仰跟随相机）：`AlsRotationModeTags.Aiming`
    Aiming,
}

/// 蹲立姿态。对应 `AlsStanceTags`。
#[derive(
    Component, Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Reflect, serde::Serialize,
    serde::Deserialize,
)]
#[reflect(Default)]
pub enum Stance {
    #[default]
    Standing,
    Crouching,
}

/// 步态三档。对应 `AlsGaitTags`。
#[derive(
    Component, Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Reflect, serde::Serialize,
    serde::Deserialize,
)]
#[reflect(Default)]
pub enum Gait {
    #[default]
    Walking,
    Running,
    Sprinting,
}

/// Overlay 动画层状态。对应 `AlsOverlayModeTags` 全部 13 项。
#[derive(
    Component, Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Reflect, serde::Serialize,
    serde::Deserialize,
)]
#[reflect(Default)]
pub enum OverlayState {
    #[default]
    Default,
    Masculine,
    Feminine,
    Injured,
    HandsTied,
    Rifle,
    PistolOneHanded,
    PistolTwoHanded,
    Bow,
    Torch,
    Binoculars,
    Box,
    Barrel,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_als_baseline() {
        assert_eq!(LocomotionState::default(), LocomotionState::Grounded);
        assert_eq!(RotationMode::default(), RotationMode::VelocityDirection);
        assert_eq!(Stance::default(), Stance::Standing);
        assert_eq!(Gait::default(), Gait::Walking);
        assert_eq!(OverlayState::default(), OverlayState::Default);
    }

    #[test]
    fn ron_roundtrip_matches_als_tag_names() {
        fn check<T>(text: &str, value: T)
        where
            T: PartialEq + std::fmt::Debug + serde::Serialize + serde::de::DeserializeOwned,
        {
            let ser = ron::to_string(&value).unwrap();
            assert_eq!(ser.trim(), text, "{value:?} 序列化名应与 ALS Tag 名一致");
            let de: T = ron::from_str(&ser).unwrap();
            assert_eq!(de, value);
        }
        check("Grounded", LocomotionState::Grounded);
        check("Air", LocomotionState::Air);
        check("Ragdolling", LocomotionState::Ragdolling);
        check("VelocityDirection", RotationMode::VelocityDirection);
        check("ViewDirection", RotationMode::ViewDirection);
        check("Aiming", RotationMode::Aiming);
        check("Standing", Stance::Standing);
        check("Crouching", Stance::Crouching);
        check("Walking", Gait::Walking);
        check("Running", Gait::Running);
        check("Sprinting", Gait::Sprinting);
        check("Default", OverlayState::Default);
        check("PistolTwoHanded", OverlayState::PistolTwoHanded);
        check("Torch", OverlayState::Torch);
        check("Barrel", OverlayState::Barrel);
    }
}
