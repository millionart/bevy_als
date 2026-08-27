use als_core::character::AlsCharacter;
use als_core::mantle::MantleKind;
use als_core::state::{LocomotionState, OverlayState};
use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;

use crate::character_assembly::AlsCharacterAssembly;

pub const LEFT_GUN_BONE: &str = "VB hand_l_to_ik_hand_gun";
pub const RIGHT_GUN_BONE: &str = "VB hand_r_to_ik_hand_gun";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayObjectKind {
    Static,
    Skeletal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OverlayObjectSpec {
    pub kind: OverlayObjectKind,
    pub asset_path: &'static str,
    pub socket_name: &'static str,
    pub use_left_gun_bone: bool,
}

pub fn overlay_object_spec(overlay: OverlayState) -> Option<OverlayObjectSpec> {
    match overlay {
        OverlayState::Default
        | OverlayState::Masculine
        | OverlayState::Feminine
        | OverlayState::Injured
        | OverlayState::HandsTied => None,
        OverlayState::Rifle => Some(OverlayObjectSpec {
            kind: OverlayObjectKind::Skeletal,
            asset_path: "models/SKM_Als_Rifle.gltf",
            socket_name: "Rifle",
            use_left_gun_bone: false,
        }),
        OverlayState::PistolOneHanded | OverlayState::PistolTwoHanded => Some(OverlayObjectSpec {
            kind: OverlayObjectKind::Skeletal,
            asset_path: "models/SKM_Als_Pistol.gltf",
            socket_name: "Pistol",
            use_left_gun_bone: false,
        }),
        OverlayState::Bow => Some(OverlayObjectSpec {
            kind: OverlayObjectKind::Skeletal,
            asset_path: "models/SKM_Als_Bow.gltf",
            socket_name: "Bow",
            use_left_gun_bone: true,
        }),
        OverlayState::Torch => Some(OverlayObjectSpec {
            kind: OverlayObjectKind::Static,
            asset_path: "models/overlay/SM_Als_Torch.gltf",
            socket_name: "Torch",
            use_left_gun_bone: true,
        }),
        OverlayState::Binoculars => Some(OverlayObjectSpec {
            kind: OverlayObjectKind::Static,
            asset_path: "models/overlay/SM_Als_Binoculars.gltf",
            socket_name: "Binoculars",
            use_left_gun_bone: false,
        }),
        OverlayState::Box => Some(OverlayObjectSpec {
            kind: OverlayObjectKind::Static,
            asset_path: "models/overlay/SM_Als_Box.gltf",
            socket_name: "Box",
            use_left_gun_bone: false,
        }),
        OverlayState::Barrel => Some(OverlayObjectSpec {
            kind: OverlayObjectKind::Static,
            asset_path: "models/overlay/SM_Als_Barrel.gltf",
            socket_name: "Barrel",
            use_left_gun_bone: true,
        }),
    }
}

pub fn attachment_bone(spec: OverlayObjectSpec, use_gun_bones: bool) -> &'static str {
    if use_gun_bones {
        if spec.use_left_gun_bone {
            LEFT_GUN_BONE
        } else {
            RIGHT_GUN_BONE
        }
    } else if spec.use_left_gun_bone {
        "hand_l"
    } else {
        "hand_r"
    }
}

pub fn overlay_object_visible(
    overlay: OverlayState,
    locomotion: LocomotionState,
    mantle_active: bool,
    mantle_kind: Option<MantleKind>,
) -> bool {
    overlay_object_spec(overlay).is_some()
        && locomotion != LocomotionState::Ragdolling
        && !(mantle_active && mantle_kind == Some(MantleKind::High))
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlsOverlayObjectConfig {
    pub use_gun_bones: bool,
}

impl Default for AlsOverlayObjectConfig {
    fn default() -> Self {
        Self {
            use_gun_bones: true,
        }
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlsOverlayObject {
    pub overlay: OverlayState,
    pub kind: OverlayObjectKind,
    pub attachment_bone: &'static str,
}

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct AlsOverlayObjectState {
    pub prop: Option<Entity>,
    pub applied_overlay: Option<OverlayState>,
    pub visible: bool,
}

pub struct OverlayObjectPlugin;

impl Plugin for OverlayObjectPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (ensure_overlay_object_state, sync_overlay_object).chain(),
        );
    }
}

fn ensure_overlay_object_state(
    mut commands: Commands,
    characters: Query<Entity, (With<AlsCharacter>, Without<AlsOverlayObjectState>)>,
) {
    for entity in &characters {
        commands.entity(entity).insert((
            AlsOverlayObjectConfig::default(),
            AlsOverlayObjectState::default(),
        ));
    }
}

fn find_named_descendant(
    root: Entity,
    name: &str,
    children: &Query<&Children>,
    names: &Query<&Name>,
) -> Option<Entity> {
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        if names
            .get(entity)
            .is_ok_and(|candidate| candidate.as_str() == name)
        {
            return Some(entity);
        }
        if let Ok(descendants) = children.get(entity) {
            stack.extend(descendants.iter());
        }
    }
    None
}

fn sync_overlay_object(
    mut commands: Commands,
    assets: Res<AssetServer>,
    children: Query<&Children>,
    names: Query<&Name>,
    props: Query<(), With<AlsOverlayObject>>,
    mut characters: Query<
        (
            &OverlayState,
            &LocomotionState,
            Option<&als_core::mantle::AlsMantleState>,
            &AlsCharacterAssembly,
            Ref<AlsOverlayObjectConfig>,
            &mut AlsOverlayObjectState,
        ),
        With<AlsCharacter>,
    >,
) {
    for (overlay, locomotion, mantle, assembly, config, mut state) in &mut characters {
        let visible = overlay_object_visible(
            *overlay,
            *locomotion,
            mantle.is_some_and(|state| state.active),
            mantle.map(|state| state.kind),
        );
        let desired_overlay = visible.then_some(*overlay);
        let prop_exists = state.prop.is_some_and(|prop| props.get(prop).is_ok());
        if state.applied_overlay == desired_overlay
            && state.visible == visible
            && prop_exists
            && !config.is_changed()
        {
            continue;
        }
        if state.applied_overlay == desired_overlay
            && desired_overlay.is_none()
            && state.prop.is_none()
            && !config.is_changed()
        {
            continue;
        }
        if let Some(prop) = state.prop.take()
            && props.get(prop).is_ok()
        {
            commands.entity(prop).despawn();
        }
        state.applied_overlay = desired_overlay;
        state.visible = visible;
        let Some(overlay) = desired_overlay else {
            continue;
        };
        let Some(spec) = overlay_object_spec(overlay) else {
            continue;
        };
        let bone_name = attachment_bone(spec, config.use_gun_bones);
        let Some(bone) = find_named_descendant(assembly.avatar, bone_name, &children, &names)
        else {
            state.applied_overlay = None;
            state.visible = false;
            continue;
        };
        let prop = commands
            .spawn((
                Name::new(format!("AlsOverlayProp::{overlay:?}")),
                WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(spec.asset_path))),
                Transform::IDENTITY,
                AlsOverlayObject {
                    overlay,
                    kind: spec.kind,
                    attachment_bone: bone_name,
                },
            ))
            .id();
        commands.entity(bone).add_child(prop);
        state.prop = Some(prop);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_overlay_specs_keep_asset_kind_and_gun_side() {
        assert!(overlay_object_spec(OverlayState::Default).is_none());
        let rifle = overlay_object_spec(OverlayState::Rifle).unwrap();
        assert_eq!(rifle.kind, OverlayObjectKind::Skeletal);
        assert_eq!(rifle.socket_name, "Rifle");
        assert!(!rifle.use_left_gun_bone);
        let torch = overlay_object_spec(OverlayState::Torch).unwrap();
        assert_eq!(torch.kind, OverlayObjectKind::Static);
        assert_eq!(torch.socket_name, "Torch");
        assert!(torch.use_left_gun_bone);
        assert_eq!(
            overlay_object_spec(OverlayState::PistolTwoHanded),
            overlay_object_spec(OverlayState::PistolOneHanded)
        );
    }

    #[test]
    fn gun_bone_mode_uses_source_virtual_bones() {
        let rifle = overlay_object_spec(OverlayState::Rifle).unwrap();
        let torch = overlay_object_spec(OverlayState::Torch).unwrap();
        assert_eq!(attachment_bone(rifle, true), RIGHT_GUN_BONE);
        assert_eq!(attachment_bone(torch, true), LEFT_GUN_BONE);
        assert_eq!(attachment_bone(rifle, false), "hand_r");
        assert_eq!(attachment_bone(torch, false), "hand_l");
    }

    #[test]
    fn source_lifecycle_clears_high_mantle_and_ragdoll_but_keeps_low_mantle() {
        assert!(overlay_object_visible(
            OverlayState::Rifle,
            LocomotionState::Grounded,
            false,
            None
        ));
        assert!(!overlay_object_visible(
            OverlayState::Rifle,
            LocomotionState::Grounded,
            true,
            Some(MantleKind::High)
        ));
        assert!(overlay_object_visible(
            OverlayState::Rifle,
            LocomotionState::Grounded,
            true,
            Some(MantleKind::LowLeft)
        ));
        assert!(!overlay_object_visible(
            OverlayState::Rifle,
            LocomotionState::Ragdolling,
            false,
            None
        ));
    }
}
