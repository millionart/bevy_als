use crate::als::character::AlsCharacter;
use bevy::ecs::hierarchy::ChildOf;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;

use crate::animation_context_projection::AlsRootAnimationContext;
use crate::character_assembly::AlsCharacterAvatar;

const ZERO_ANIM_WEIGHT_THRESHOLD: f32 = 0.00001;

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

pub struct LayeringLayerPlugin;

impl Plugin for LayeringLayerPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            PostUpdate,
            AlsLayeringLayerSet.before(
                crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate,
            ),
        )
        .add_systems(
            PostUpdate,
            layering_layer_writer.in_set(AlsLayeringLayerSet),
        );
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

#[doc(hidden)]
pub fn refresh_state_from_curves(
    context: Option<&AlsRootAnimationContext>,
) -> AlsLayeringState {
    let mut state = AlsLayeringState::default();
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

#[doc(hidden)]
pub fn layering_state_inputs(state: AlsLayeringState) -> [(&'static str, f32); 22] {
    [
        ("layering_state.head_blend_amount", state.head_blend_amount),
        (
            "layering_state.head_additive_blend_amount",
            state.head_additive_blend_amount,
        ),
        (
            "layering_state.head_slot_blend_amount",
            state.head_slot_blend_amount,
        ),
        (
            "layering_state.arm_left_blend_amount",
            state.arm_left_blend_amount,
        ),
        (
            "layering_state.arm_left_additive_blend_amount",
            state.arm_left_additive_blend_amount,
        ),
        (
            "layering_state.arm_left_slot_blend_amount",
            state.arm_left_slot_blend_amount,
        ),
        (
            "layering_state.arm_left_local_space_blend_amount",
            state.arm_left_local_space_blend_amount,
        ),
        (
            "layering_state.arm_left_mesh_space_blend_amount",
            state.arm_left_mesh_space_blend_amount,
        ),
        (
            "layering_state.arm_right_blend_amount",
            state.arm_right_blend_amount,
        ),
        (
            "layering_state.arm_right_additive_blend_amount",
            state.arm_right_additive_blend_amount,
        ),
        (
            "layering_state.arm_right_slot_blend_amount",
            state.arm_right_slot_blend_amount,
        ),
        (
            "layering_state.arm_right_local_space_blend_amount",
            state.arm_right_local_space_blend_amount,
        ),
        (
            "layering_state.arm_right_mesh_space_blend_amount",
            state.arm_right_mesh_space_blend_amount,
        ),
        (
            "layering_state.hand_left_blend_amount",
            state.hand_left_blend_amount,
        ),
        (
            "layering_state.hand_right_blend_amount",
            state.hand_right_blend_amount,
        ),
        (
            "layering_state.spine_blend_amount",
            state.spine_blend_amount,
        ),
        (
            "layering_state.spine_additive_blend_amount",
            state.spine_additive_blend_amount,
        ),
        (
            "layering_state.spine_slot_blend_amount",
            state.spine_slot_blend_amount,
        ),
        (
            "layering_state.pelvis_blend_amount",
            state.pelvis_blend_amount,
        ),
        (
            "layering_state.pelvis_slot_blend_amount",
            state.pelvis_slot_blend_amount,
        ),
        ("layering_state.legs_blend_amount", state.legs_blend_amount),
        (
            "layering_state.legs_slot_blend_amount",
            state.legs_slot_blend_amount,
        ),
    ]
}

fn write_state(player: &mut AnimationGraphPlayer, state: AlsLayeringState) {
    for (key, value) in layering_state_inputs(state) {
        player.set_global_input_data(key, value);
    }
}

#[allow(clippy::type_complexity)]
fn layering_layer_writer(
    mut commands: Commands,
    mut characters: Query<(Entity, Option<&mut AlsLayeringState>), With<AlsCharacter>>,
    avatars: Query<(&ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    mut players: Query<(&mut AnimationGraphPlayer, Option<&AlsRootAnimationContext>)>,
) {
    for (parent, instance) in &avatars {
        let Ok((character, current_state)) = characters.get_mut(parent.parent()) else {
            continue;
        };
        let Ok((mut player, context)) = players.get_mut(instance.player_entity()) else {
            continue;
        };
        let state = refresh_state_from_curves(context);
        if let Some(mut current_state) = current_state {
            *current_state = state;
        } else {
            commands.entity(character).insert(state);
        }
        write_state(&mut player, state);
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
    fn layering_state_writes_direct_generated_inputs() {
        let fields = [
            ("LayerHead", "head_blend_amount"),
            ("LayerHeadAdditive", "head_additive_blend_amount"),
            ("LayerHeadSlot", "head_slot_blend_amount"),
            ("LayerArmLeft", "arm_left_blend_amount"),
            ("LayerArmLeftAdditive", "arm_left_additive_blend_amount"),
            ("LayerArmLeftSlot", "arm_left_slot_blend_amount"),
            (
                "LayerArmLeftLocalSpace",
                "arm_left_local_space_blend_amount",
            ),
            ("LayerArmRight", "arm_right_blend_amount"),
            ("LayerArmRightAdditive", "arm_right_additive_blend_amount"),
            ("LayerArmRightSlot", "arm_right_slot_blend_amount"),
            (
                "LayerArmRightLocalSpace",
                "arm_right_local_space_blend_amount",
            ),
            ("LayerHandLeft", "hand_left_blend_amount"),
            ("LayerHandRight", "hand_right_blend_amount"),
            ("LayerSpine", "spine_blend_amount"),
            ("LayerSpineAdditive", "spine_additive_blend_amount"),
            ("LayerSpineSlot", "spine_slot_blend_amount"),
            ("LayerPelvis", "pelvis_blend_amount"),
            ("LayerPelvisSlot", "pelvis_slot_blend_amount"),
            ("LayerLegs", "legs_blend_amount"),
            ("LayerLegsSlot", "legs_slot_blend_amount"),
        ];
        let mut player = AnimationGraphPlayer::new(default());
        for local_space in [0.99998, 0.99999, 1.2] {
            let context = AlsRootAnimationContext {
                curves: fields
                    .iter()
                    .enumerate()
                    .map(|(index, (curve, _))| {
                        (
                            (*curve).into(),
                            if curve.ends_with("LocalSpace") {
                                local_space
                            } else {
                                index as f32 * 0.075 - 0.1
                            },
                        )
                    })
                    .collect(),
            };
            for context in [Some(&context), None] {
                let state = refresh_state_from_curves(context);
                write_state(&mut player, state);
                for (curve, property) in fields {
                    let expected = context
                        .and_then(|context| {
                            context
                                .curves
                                .get(&crate::extensions::SourceCurveName::from(curve))
                                .copied()
                        })
                        .unwrap_or(0.0);
                    assert_eq!(
                        player
                            .get_global_input_data(format!("layering_state.{property}"))
                            .and_then(|value| value.as_f32().ok()),
                        Some(expected),
                        "{curve}"
                    );
                }
                let expected = if context.is_some() && local_space >= 0.99999 {
                    0.0
                } else {
                    1.0
                };
                for side in ["left", "right"] {
                    assert_eq!(
                        player
                            .get_global_input_data(format!(
                                "layering_state.arm_{side}_mesh_space_blend_amount"
                            ))
                            .and_then(|value| value.as_f32().ok()),
                        Some(expected)
                    );
                }
            }
        }
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
