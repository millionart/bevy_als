use bevy::prelude::*;
use bevy::ecs::intern::Interner;
use bevy_gameplay_tag::{GameplayTag, InvalidTagName};

static TAG_NAMES: Interner<str> = Interner::new();

macro_rules! tag_state {
    ($name:ident, $default:ident, {$($variant:ident: $tag:literal),+ $(,)?}) => {
        #[derive(Component, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
        #[reflect(opaque, Default)]
        pub struct $name(&'static str);

        #[allow(non_upper_case_globals)]
        impl $name {
            $(pub const $variant: Self = Self($tag);)+

            pub fn new(value: &str) -> Result<Self, InvalidTagName> {
                if !value.is_empty() { GameplayTag::try_new(value)?; }
                Ok(Self(TAG_NAMES.intern(value).0))
            }

            pub fn from_tag(tag: &GameplayTag) -> Self { Self(TAG_NAMES.intern(tag.as_str()).0) }
            pub const fn as_str(self) -> &'static str { self.0 }
            pub fn leaf_name(self) -> &'static str { self.0.rsplit('.').next().unwrap_or("") }
            pub fn tag(self) -> GameplayTag { GameplayTag::new(self.0) }
        }

        impl Default for $name { fn default() -> Self { Self::$default } }
        impl std::fmt::Debug for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(if self.0.is_empty() { "None" } else { self.leaf_name() })
            }
        }
        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> { serializer.serialize_str(self.0) }
        }
        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = <String as serde::Deserialize>::deserialize(deserializer)?;
                Self::new(&value).map_err(serde::de::Error::custom)
            }
        }
    };
}

tag_state!(AlsNotifyLocomotionAction, None, {
    None: "", Mantling: "Als.LocomotionAction.Mantling", GettingUp: "Als.LocomotionAction.GettingUp",
    Rolling: "Als.LocomotionAction.Rolling", Ragdolling: "Als.LocomotionAction.Ragdolling",
});

#[derive(
    Component,
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Reflect,
    serde::Serialize,
    serde::Deserialize,
)]
#[reflect(Default)]
pub enum LocomotionState {
    #[default]
    Grounded,
    Air,
    Custom,
    None,
}

tag_state!(RotationMode, VelocityDirection, {
    VelocityDirection: "Als.RotationMode.VelocityDirection", ViewDirection: "Als.RotationMode.ViewDirection", Aiming: "Als.RotationMode.Aiming",
});
tag_state!(Stance, Standing, { Standing: "Als.Stance.Standing", Crouching: "Als.Stance.Crouching" });
tag_state!(Gait, Walking, { Walking: "Als.Gait.Walking", Running: "Als.Gait.Running", Sprinting: "Als.Gait.Sprinting" });
tag_state!(OverlayState, Default, {
    Default: "Als.OverlayMode.Default", Masculine: "Als.OverlayMode.Masculine", Feminine: "Als.OverlayMode.Feminine",
    Injured: "Als.OverlayMode.Injured", HandsTied: "Als.OverlayMode.HandsTied", Rifle: "Als.OverlayMode.Rifle",
    PistolOneHanded: "Als.OverlayMode.PistolOneHanded", PistolTwoHanded: "Als.OverlayMode.PistolTwoHanded",
    Bow: "Als.OverlayMode.Bow", Torch: "Als.OverlayMode.Torch", Binoculars: "Als.OverlayMode.Binoculars",
    Box: "Als.OverlayMode.Box", Barrel: "Als.OverlayMode.Barrel",
});

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::DeserializeOwned;

    #[test]
    fn state_data_accepts_custom_gameplay_tags() {
        fn check<T: DeserializeOwned + serde::Serialize>(name: &str) {
            let value = ron::from_str::<T>(&format!("{name:?}")).expect("Custom source tag was rejected");
            assert_eq!(ron::to_string(&value).unwrap(), format!("{name:?}"));
        }
        check::<Gait>("Als.Gait.Custom");
        check::<Stance>("Als.Stance.Custom");
        check::<RotationMode>("Als.RotationMode.Custom");
        check::<OverlayState>("Als.OverlayMode.Custom");
        check::<AlsNotifyLocomotionAction>("Als.LocomotionAction.Custom");
        assert_eq!(Gait::new("Als.Gait.Walking").unwrap(), Gait::Walking);
        assert!(Gait::new("Als..Gait").is_err());
        assert!(Gait::new("Als.Gait.Custom").unwrap().tag().matches_tag_exact(&GameplayTag::new("Als.Gait.Custom")));
    }

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
            assert_eq!(
                ser.trim(),
                text,
                "{value:?} should serialize to its ALS tag name"
            );
            let de: T = ron::from_str(&ser).unwrap();
            assert_eq!(de, value);
        }
        check("Grounded", LocomotionState::Grounded);
        check("Air", LocomotionState::Air);
        check("Custom", LocomotionState::Custom);
        check("r#None", LocomotionState::None);
        check("\"Als.RotationMode.VelocityDirection\"", RotationMode::VelocityDirection);
        check("\"Als.RotationMode.ViewDirection\"", RotationMode::ViewDirection);
        check("\"Als.RotationMode.Aiming\"", RotationMode::Aiming);
        check("\"Als.Stance.Standing\"", Stance::Standing);
        check("\"Als.Stance.Crouching\"", Stance::Crouching);
        check("\"Als.Gait.Walking\"", Gait::Walking);
        check("\"Als.Gait.Running\"", Gait::Running);
        check("\"Als.Gait.Sprinting\"", Gait::Sprinting);
        check("\"Als.OverlayMode.Default\"", OverlayState::Default);
        check("\"Als.OverlayMode.PistolTwoHanded\"", OverlayState::PistolTwoHanded);
        check("\"Als.OverlayMode.Torch\"", OverlayState::Torch);
        check("\"Als.OverlayMode.Barrel\"", OverlayState::Barrel);
    }
}
