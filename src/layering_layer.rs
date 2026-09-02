use als_core::character::AlsCharacter;
use als_core::state::Stance;
use als_ext::{BranchFilter, build_branch_filter_bone_mask};
use bevy::ecs::hierarchy::ChildOf;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::{DataValue, bone_mask::BoneMask};
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use bevy_animation_graph::core::skeleton::Skeleton;

use crate::animation_context_projection::AlsRootAnimationContext;
use crate::character_assembly::AlsCharacterAvatar;
use crate::locomotion_layer::AlsLocomotionLayerSet;
use crate::stance_layer::AlsStanceBlend;

const SKELETON_ASSET: &str = "skeletons/SKM_Als.skn.ron";
const ZERO_ANIM_WEIGHT_THRESHOLD: f32 = 0.00001;

const PELVIS_FILTERS: &[BranchFilter] = &[
    BranchFilter {
        bone_name: "pelvis",
        blend_depth: 0,
    },
    BranchFilter {
        bone_name: "thigh_l",
        blend_depth: -1,
    },
    BranchFilter {
        bone_name: "thigh_r",
        blend_depth: -1,
    },
];

const SPINE_FILTERS: &[BranchFilter] = &[BranchFilter {
    bone_name: "spine_01",
    blend_depth: 3,
}];

const HEAD_FILTERS: &[BranchFilter] = &[BranchFilter {
    bone_name: "neck_01",
    blend_depth: 0,
}];

const ARM_LEFT_FILTERS: &[BranchFilter] = &[BranchFilter {
    bone_name: "clavicle_l",
    blend_depth: 1,
}];

const ARM_RIGHT_FILTERS: &[BranchFilter] = &[BranchFilter {
    bone_name: "clavicle_r",
    blend_depth: 1,
}];

const HAND_LEFT_FILTERS: &[BranchFilter] = &[
    BranchFilter {
        bone_name: "index_01_l",
        blend_depth: 0,
    },
    BranchFilter {
        bone_name: "middle_01_l",
        blend_depth: 0,
    },
    BranchFilter {
        bone_name: "ring_01_l",
        blend_depth: 0,
    },
    BranchFilter {
        bone_name: "pinky_01_l",
        blend_depth: 0,
    },
    BranchFilter {
        bone_name: "thumb_01_l",
        blend_depth: 0,
    },
    BranchFilter {
        bone_name: "VB hand_l_to_hand_r",
        blend_depth: 0,
    },
    BranchFilter {
        bone_name: "VB hand_l_to_ik_hand_gun",
        blend_depth: 0,
    },
];

const HAND_RIGHT_FILTERS: &[BranchFilter] = &[
    BranchFilter {
        bone_name: "index_01_r",
        blend_depth: 0,
    },
    BranchFilter {
        bone_name: "middle_01_r",
        blend_depth: 0,
    },
    BranchFilter {
        bone_name: "ring_01_r",
        blend_depth: 0,
    },
    BranchFilter {
        bone_name: "pinky_01_r",
        blend_depth: 0,
    },
    BranchFilter {
        bone_name: "thumb_01_r",
        blend_depth: 0,
    },
    BranchFilter {
        bone_name: "VB hand_r_to_hand_r",
        blend_depth: 0,
    },
    BranchFilter {
        bone_name: "VB hand_r_to_ik_hand_gun",
        blend_depth: 0,
    },
];

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct AlsLayeringState {
    pub head_blend_amount: f32,
    pub head_additive_blend_amount: f32,
    pub head_slot_blend_amount: f32,
    pub arm_left_blend_amount: f32,
    pub arm_left_additive_blend_amount: f32,
    pub arm_left_slot_blend_amount: f32,
    pub arm_left_local_space_blend_amount: f32,
    pub arm_left_mesh_space_blend_amount: f32,
    pub arm_right_blend_amount: f32,
    pub arm_right_additive_blend_amount: f32,
    pub arm_right_slot_blend_amount: f32,
    pub arm_right_local_space_blend_amount: f32,
    pub arm_right_mesh_space_blend_amount: f32,
    pub hand_left_blend_amount: f32,
    pub hand_right_blend_amount: f32,
    pub spine_blend_amount: f32,
    pub spine_additive_blend_amount: f32,
    pub spine_slot_blend_amount: f32,
    pub pelvis_blend_amount: f32,
    pub pelvis_slot_blend_amount: f32,
    pub legs_blend_amount: f32,
    pub legs_slot_blend_amount: f32,
}

impl Default for AlsLayeringState {
    fn default() -> Self {
        Self {
            head_blend_amount: 0.0,
            head_additive_blend_amount: 0.0,
            head_slot_blend_amount: 1.0,
            arm_left_blend_amount: 0.0,
            arm_left_additive_blend_amount: 0.0,
            arm_left_slot_blend_amount: 1.0,
            arm_left_local_space_blend_amount: 0.0,
            arm_left_mesh_space_blend_amount: 0.0,
            arm_right_blend_amount: 0.0,
            arm_right_additive_blend_amount: 0.0,
            arm_right_slot_blend_amount: 1.0,
            arm_right_local_space_blend_amount: 0.0,
            arm_right_mesh_space_blend_amount: 0.0,
            hand_left_blend_amount: 0.0,
            hand_right_blend_amount: 0.0,
            spine_blend_amount: 0.0,
            spine_additive_blend_amount: 0.0,
            spine_slot_blend_amount: 1.0,
            pelvis_blend_amount: 0.0,
            pelvis_slot_blend_amount: 1.0,
            legs_blend_amount: 0.0,
            legs_slot_blend_amount: 1.0,
        }
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsLayeringLayerSet;

#[derive(Resource)]
struct LayeringMasks {
    skeleton: Handle<Skeleton>,
    pelvis: Option<BoneMask>,
    spine: Option<BoneMask>,
    head: Option<BoneMask>,
    arm_left: Option<BoneMask>,
    arm_right: Option<BoneMask>,
    hand_left: Option<BoneMask>,
    hand_right: Option<BoneMask>,
}

pub struct LayeringLayerPlugin;

impl Plugin for LayeringLayerPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            PostUpdate,
            AlsLayeringLayerSet
                .after(AlsLocomotionLayerSet)
                .before(AnimationGraphSet::PrePhysics),
        )
        .add_systems(Startup, load_layering_masks)
        .add_systems(
            PostUpdate,
            layering_layer_writer.in_set(AlsLayeringLayerSet),
        );
    }
}

fn load_layering_masks(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(LayeringMasks {
        skeleton: assets.load(SKELETON_ASSET),
        pelvis: None,
        spine: None,
        head: None,
        arm_left: None,
        arm_right: None,
        hand_left: None,
        hand_right: None,
    });
}

fn update_masks(masks: &mut LayeringMasks, skeletons: &Assets<Skeleton>) {
    let Some(skeleton) = skeletons.get(&masks.skeleton) else {
        return;
    };
    if masks.pelvis.is_none() {
        masks.pelvis = Some(build_branch_filter_bone_mask(skeleton, PELVIS_FILTERS));
    }
    if masks.spine.is_none() {
        masks.spine = Some(build_branch_filter_bone_mask(skeleton, SPINE_FILTERS));
    }
    if masks.head.is_none() {
        masks.head = Some(build_branch_filter_bone_mask(skeleton, HEAD_FILTERS));
    }
    if masks.arm_left.is_none() {
        masks.arm_left = Some(build_branch_filter_bone_mask(skeleton, ARM_LEFT_FILTERS));
    }
    if masks.arm_right.is_none() {
        masks.arm_right = Some(build_branch_filter_bone_mask(skeleton, ARM_RIGHT_FILTERS));
    }
    if masks.hand_left.is_none() {
        masks.hand_left = Some(build_branch_filter_bone_mask(skeleton, HAND_LEFT_FILTERS));
    }
    if masks.hand_right.is_none() {
        masks.hand_right = Some(build_branch_filter_bone_mask(skeleton, HAND_RIGHT_FILTERS));
    }
}

fn arm_mesh_space_blend_amount(local_space_blend_amount: f32) -> f32 {
    if local_space_blend_amount >= 1.0 - ZERO_ANIM_WEIGHT_THRESHOLD {
        0.0
    } else {
        1.0
    }
}

fn curve_value(context: Option<&AlsRootAnimationContext>, name: &str) -> f32 {
    context
        .and_then(|context| context.f32(name))
        .unwrap_or_default()
}

fn refresh_state_from_curves(
    mut state: AlsLayeringState,
    context: Option<&AlsRootAnimationContext>,
) -> AlsLayeringState {
    state.head_blend_amount = curve_value(context, "LayerHead");
    state.head_additive_blend_amount = curve_value(context, "LayerHeadAdditive");
    state.head_slot_blend_amount = curve_value(context, "LayerHeadSlot");
    state.arm_left_blend_amount = curve_value(context, "LayerArmLeft");
    state.arm_left_additive_blend_amount = curve_value(context, "LayerArmLeftAdditive");
    state.arm_left_slot_blend_amount = curve_value(context, "LayerArmLeftSlot");
    state.arm_left_local_space_blend_amount = curve_value(context, "LayerArmLeftLocalSpace");
    state.arm_right_blend_amount = curve_value(context, "LayerArmRight");
    state.arm_right_additive_blend_amount = curve_value(context, "LayerArmRightAdditive");
    state.arm_right_slot_blend_amount = curve_value(context, "LayerArmRightSlot");
    state.arm_right_local_space_blend_amount = curve_value(context, "LayerArmRightLocalSpace");
    state.hand_left_blend_amount = curve_value(context, "LayerHandLeft");
    state.hand_right_blend_amount = curve_value(context, "LayerHandRight");
    state.spine_blend_amount = curve_value(context, "LayerSpine");
    state.spine_additive_blend_amount = curve_value(context, "LayerSpineAdditive");
    state.spine_slot_blend_amount = curve_value(context, "LayerSpineSlot");
    state.pelvis_blend_amount = curve_value(context, "LayerPelvis");
    state.pelvis_slot_blend_amount = curve_value(context, "LayerPelvisSlot");
    state.legs_blend_amount = curve_value(context, "LayerLegs");
    state.legs_slot_blend_amount = curve_value(context, "LayerLegsSlot");
    state.arm_left_mesh_space_blend_amount =
        arm_mesh_space_blend_amount(state.arm_left_local_space_blend_amount);
    state.arm_right_mesh_space_blend_amount =
        arm_mesh_space_blend_amount(state.arm_right_local_space_blend_amount);
    state
}

fn write_state(player: &mut AnimationGraphPlayer, state: AlsLayeringState, crouching_amount: f32) {
    for (key, value) in [
        ("als_layering_standing_amount", 1.0 - crouching_amount),
        ("als_layering_crouching_amount", crouching_amount),
        ("als_layering_head", state.head_blend_amount),
        (
            "als_layering_head_additive",
            state.head_additive_blend_amount,
        ),
        ("als_layering_head_slot", state.head_slot_blend_amount),
        ("als_layering_arm_left", state.arm_left_blend_amount),
        (
            "als_layering_arm_left_additive",
            state.arm_left_additive_blend_amount,
        ),
        (
            "als_layering_arm_left_slot",
            state.arm_left_slot_blend_amount,
        ),
        (
            "als_layering_arm_left_local_space",
            state.arm_left_local_space_blend_amount,
        ),
        (
            "als_layering_arm_left_mesh_space",
            state.arm_left_mesh_space_blend_amount,
        ),
        ("als_layering_arm_right", state.arm_right_blend_amount),
        (
            "als_layering_arm_right_additive",
            state.arm_right_additive_blend_amount,
        ),
        (
            "als_layering_arm_right_slot",
            state.arm_right_slot_blend_amount,
        ),
        (
            "als_layering_arm_right_local_space",
            state.arm_right_local_space_blend_amount,
        ),
        (
            "als_layering_arm_right_mesh_space",
            state.arm_right_mesh_space_blend_amount,
        ),
        ("als_layering_hand_left", state.hand_left_blend_amount),
        ("als_layering_hand_right", state.hand_right_blend_amount),
        ("als_layering_spine", state.spine_blend_amount),
        (
            "als_layering_spine_additive",
            state.spine_additive_blend_amount,
        ),
        ("als_layering_spine_slot", state.spine_slot_blend_amount),
        ("als_layering_pelvis", state.pelvis_blend_amount),
        ("als_layering_pelvis_slot", state.pelvis_slot_blend_amount),
        ("als_layering_legs", state.legs_blend_amount),
        ("als_layering_legs_slot", state.legs_slot_blend_amount),
    ] {
        player.set_global_input_data(key, value);
    }
}

fn write_masks(player: &mut AnimationGraphPlayer, masks: &LayeringMasks) {
    for (key, value) in [
        ("als_layering_pelvis_mask", masks.pelvis.clone()),
        ("als_layering_spine_mask", masks.spine.clone()),
        ("als_layering_head_mask", masks.head.clone()),
        ("als_layering_arm_left_mask", masks.arm_left.clone()),
        ("als_layering_arm_right_mask", masks.arm_right.clone()),
        ("als_layering_hand_left_mask", masks.hand_left.clone()),
        ("als_layering_hand_right_mask", masks.hand_right.clone()),
    ] {
        player.set_global_input_data(
            key,
            DataValue::BoneMask(value.unwrap_or_else(BoneMask::none)),
        );
    }
}

#[allow(clippy::type_complexity)]
fn layering_layer_writer(
    mut commands: Commands,
    mut masks: ResMut<LayeringMasks>,
    skeletons: Res<Assets<Skeleton>>,
    mut characters: Query<
        (Entity, Option<&Stance>, Option<&mut AlsLayeringState>),
        With<AlsCharacter>,
    >,
    avatars: Query<
        (&ChildOf, &AnimatedSceneInstance, Option<&AlsStanceBlend>),
        With<AlsCharacterAvatar>,
    >,
    mut players: Query<(&mut AnimationGraphPlayer, Option<&AlsRootAnimationContext>)>,
) {
    update_masks(&mut masks, &skeletons);
    for (parent, instance, stance_blend) in &avatars {
        let Ok((character, stance, current_state)) = characters.get_mut(parent.parent()) else {
            continue;
        };
        let state = current_state.as_deref().copied().unwrap_or_default();
        let crouching_amount = stance_blend.map_or_else(
            || {
                if stance.is_some_and(|stance| *stance == Stance::Crouching) {
                    1.0
                } else {
                    0.0
                }
            },
            |blend| blend.weight,
        );
        let Ok((mut player, context)) = players.get_mut(instance.player_entity()) else {
            continue;
        };
        let state = refresh_state_from_curves(state, context);
        if let Some(mut current_state) = current_state {
            *current_state = state;
        } else {
            commands.entity(character).insert(state);
        }
        write_state(&mut player, state, crouching_amount);
        write_masks(&mut player, &masks);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arm_mesh_space_rule_uses_the_source_full_weight_threshold() {
        assert_eq!(arm_mesh_space_blend_amount(0.99998), 1.0);
        assert_eq!(arm_mesh_space_blend_amount(0.99999), 0.0);
        assert_eq!(arm_mesh_space_blend_amount(1.0), 0.0);
    }

    #[test]
    fn source_defaults_keep_slots_open_and_layers_disabled() {
        let state = AlsLayeringState::default();
        assert_eq!(state.head_slot_blend_amount, 1.0);
        assert_eq!(state.arm_left_slot_blend_amount, 1.0);
        assert_eq!(state.arm_right_slot_blend_amount, 1.0);
        assert_eq!(state.spine_slot_blend_amount, 1.0);
        assert_eq!(state.pelvis_slot_blend_amount, 1.0);
        assert_eq!(state.legs_slot_blend_amount, 1.0);
        assert_eq!(state.head_blend_amount, 0.0);
        assert_eq!(state.legs_blend_amount, 0.0);
    }
}
