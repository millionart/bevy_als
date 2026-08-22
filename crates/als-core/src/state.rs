use bevy::prelude::*;

#[derive(
    Component, Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Reflect, serde::Serialize,
    serde::Deserialize,
)]
#[reflect(Default)]
pub enum LocomotionState {
    #[default]
    Grounded,
    Air,
    Ragdolling,
}

#[derive(
    Component, Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Reflect, serde::Serialize,
    serde::Deserialize,
)]
#[reflect(Default)]
pub enum RotationMode {
    #[default]
    VelocityDirection,
    ViewDirection,
    Aiming,
}

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
    use serde::de::DeserializeOwned;

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
            T: PartialEq + std::fmt::Debug + serde::Serialize + DeserializeOwned,
        {
            let ser = ron::to_string(&value).unwrap();
            assert_eq!(ser.trim(), text, "{value:?} should serialize to its ALS tag name");
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
