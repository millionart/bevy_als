use als_core::character::AlsCharacter;
use als_core::input_request::AlsFirstPersonView;
use als_core::motion::AlsViewYaw;
use als_core::orientation::world_forward_to_yaw;
use als_core::state::{LocomotionState, RotationMode};
use als_ext::ik::{solve_two_bone_ik, world_rotation_to_local};
use avian3d::prelude::LinearVelocity;
use bevy::animation::AnimationTargetId;
use bevy::ecs::hierarchy::ChildOf;
use bevy::prelude::*;
use bevy::transform::{TransformSystems, systems as transform_systems};
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;
use bevy_animation_graph::core::plugin::AnimationGraphSet;

use crate::character_assembly::AlsCharacterAvatar;
use crate::head_layer::{inv_exp_approx, unwind_degrees};

const SPINE_BONES: [&str; 4] = ["pelvis", "spine_01", "spine_02", "spine_03"];
const FOOT_ROOT_BONE: &str = "ik_foot_root";
const LEFT_ARM_BONES: [&str; 4] = ["upperarm_l", "lowerarm_l", "hand_l", "VB ik_hand_l"];
const RIGHT_ARM_BONES: [&str; 4] = ["upperarm_r", "lowerarm_r", "hand_r", "VB ik_hand_r"];
const SPINE_ENTER_HALF_LIFE: f32 = 0.1;
const SPINE_EXIT_HALF_LIFE: f32 = 0.7;
const SPINE_REFERENCE_VIEW_YAW_SPEED: f32 = 40.0;
const SPINE_MAX_WORLD_OFFSET: f32 = 30.0;
const SPINE_DELTA_FRACTION: f32 = 0.25;
const HAND_POLE_OFFSET_METERS: f32 = 0.3;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsControlRigSet;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsControlRigPropagationSet;

pub struct ControlRigApplyPlugin;

impl Plugin for ControlRigApplyPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            PostUpdate,
            AlsControlRigSet
                .after(AnimationGraphSet::Final)
                .after(TransformSystems::Propagate),
        )
        .configure_sets(
            PostUpdate,
            AlsControlRigPropagationSet.after(AlsControlRigSet),
        )
        .add_systems(
            PostUpdate,
            (refresh_control_rig_bones, control_rig_apply_system)
                .chain()
                .in_set(AlsControlRigSet),
        )
        .add_systems(
            PostUpdate,
            (
                transform_systems::mark_dirty_trees,
                transform_systems::propagate_parent_transforms,
                transform_systems::sync_simple_transforms,
            )
                .chain()
                .in_set(AlsControlRigPropagationSet),
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ArmChain {
    upper: Entity,
    lower: Entity,
    hand: Entity,
    target: Entity,
}

#[derive(Component, Clone, Debug, Default, PartialEq)]
struct AlsControlRigBones {
    spine: [Option<Entity>; 4],
    foot_root: Option<Entity>,
    left_arm: Option<ArmChain>,
    right_arm: Option<ArmChain>,
}

impl AlsControlRigBones {
    fn ready(&self) -> bool {
        self.spine.iter().all(Option::is_some)
    }
}

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct AlsControlRigState {
    pub spine_rotation_allowed: bool,
    pub spine_amount: f32,
    pub spine_amount_scale: f32,
    pub spine_amount_bias: f32,
    pub last_yaw_angle: f32,
    pub last_yaw_angle_world_space: f32,
    pub yaw_angle: f32,
    pub final_yaw_angle: f32,
    previous_view_yaw_world: f32,
    initialized: bool,
}

impl Default for AlsControlRigState {
    fn default() -> Self {
        Self {
            spine_rotation_allowed: false,
            spine_amount: 0.0,
            spine_amount_scale: 1.0,
            spine_amount_bias: 0.0,
            last_yaw_angle: 0.0,
            last_yaw_angle_world_space: 0.0,
            yaw_angle: 0.0,
            final_yaw_angle: 0.0,
            previous_view_yaw_world: 0.0,
            initialized: false,
        }
    }
}

#[derive(Clone, Copy)]
struct SpineInput {
    allowed: bool,
    view_yaw_angle: f32,
    view_yaw_world: f32,
    body_yaw_world: f32,
    blend_amount: f32,
    delta_seconds: f32,
}

fn is_relevant(value: f32) -> bool {
    value > 1e-4
}

fn is_full(value: f32) -> bool {
    value >= 1.0 - 1e-4
}

fn damper_exact(current: f32, target: f32, delta_seconds: f32, half_life: f32) -> f32 {
    let alpha =
        1.0 - inv_exp_approx(std::f32::consts::LN_2 * delta_seconds.max(0.0) / (half_life + 1e-8));
    current + (target - current) * alpha
}

fn lerp_angle(from: f32, to: f32, amount: f32) -> f32 {
    unwind_degrees(from + unwind_degrees(to - from) * amount)
}

fn body_yaw_degrees(transform: &Transform) -> f32 {
    let forward = transform.rotation * Vec3::NEG_Z;
    world_forward_to_yaw(forward).to_degrees()
}

fn velocity_blend_amounts(velocity: Vec3, transform: &Transform) -> (f32, f32) {
    let forward = transform.rotation * Vec3::NEG_Z;
    let right = transform.rotation * Vec3::X;
    let local = Vec2::new(velocity.dot(forward), velocity.dot(right));
    let Some(unit) = local.try_normalize() else {
        return (0.0, 0.0);
    };
    let normalized = unit / (unit.x.abs() + unit.y.abs()).max(f32::EPSILON);
    (normalized.x.max(0.0), (-normalized.x).max(0.0))
}

fn update_spine_state(state: &mut AlsControlRigState, input: SpineInput) {
    let view_yaw_speed = if state.initialized && input.delta_seconds > 1e-6 {
        unwind_degrees(input.view_yaw_world - state.previous_view_yaw_world).abs()
            / input.delta_seconds
    } else {
        0.0
    };
    if state.spine_rotation_allowed != input.allowed {
        state.spine_rotation_allowed = input.allowed;
        if state.spine_rotation_allowed {
            if is_full(state.spine_amount) {
                state.spine_amount_scale = 1.0;
                state.spine_amount_bias = 0.0;
            } else {
                state.spine_amount_scale = 1.0 / (1.0 - state.spine_amount);
                state.spine_amount_bias = -state.spine_amount * state.spine_amount_scale;
            }
        } else {
            state.spine_amount_scale = if is_relevant(state.spine_amount) {
                1.0 / state.spine_amount
            } else {
                1.0
            };
            state.spine_amount_bias = 0.0;
        }
        state.last_yaw_angle = state.yaw_angle;
        state.last_yaw_angle_world_space = input.body_yaw_world;
    }
    if state.spine_rotation_allowed {
        if !state.initialized || is_full(state.spine_amount) {
            state.spine_amount = 1.0;
            state.yaw_angle = input.view_yaw_angle;
        } else {
            state.spine_amount = damper_exact(
                state.spine_amount,
                1.0,
                input.delta_seconds,
                SPINE_ENTER_HALF_LIFE,
            );
            state.yaw_angle = lerp_angle(
                state.last_yaw_angle,
                input.view_yaw_angle,
                state.spine_amount * state.spine_amount_scale + state.spine_amount_bias,
            );
        }
    } else if !state.initialized || !is_relevant(state.spine_amount) {
        state.spine_amount = 0.0;
        state.yaw_angle = 0.0;
    } else {
        let multiplier = if view_yaw_speed > SPINE_REFERENCE_VIEW_YAW_SPEED {
            SPINE_REFERENCE_VIEW_YAW_SPEED / view_yaw_speed
        } else {
            1.0
        };
        state.spine_amount = damper_exact(
            state.spine_amount,
            0.0,
            input.delta_seconds,
            SPINE_EXIT_HALF_LIFE * multiplier,
        );
        let yaw_offset = unwind_degrees(state.last_yaw_angle_world_space - input.body_yaw_world)
            .clamp(-SPINE_MAX_WORLD_OFFSET, SPINE_MAX_WORLD_OFFSET);
        state.last_yaw_angle_world_space = unwind_degrees(yaw_offset + input.body_yaw_world);
        state.yaw_angle = lerp_angle(
            0.0,
            state.last_yaw_angle + yaw_offset,
            state.spine_amount * state.spine_amount_scale + state.spine_amount_bias,
        );
    }
    state.final_yaw_angle = lerp_angle(0.0, state.yaw_angle, input.blend_amount.clamp(0.0, 1.0));
    state.previous_view_yaw_world = input.view_yaw_world;
    state.initialized = true;
}

fn output_f32(player: &AnimationGraphPlayer, name: &str) -> f32 {
    match player.get_outputs().get(name) {
        Some(DataValue::F32(value)) => *value,
        _ => 0.0,
    }
}

fn collect_bones(
    root: Entity,
    children_q: &Query<&Children>,
    name_q: &Query<&Name>,
    target_q: &Query<(), With<AnimationTargetId>>,
) -> Vec<(String, Entity)> {
    let mut stack = vec![root];
    let mut bones = Vec::new();
    while let Some(entity) = stack.pop() {
        let Ok(children) = children_q.get(entity) else {
            continue;
        };
        for child in children.iter() {
            if target_q.get(child).is_ok() {
                if let Ok(name) = name_q.get(child) {
                    if !bones
                        .iter()
                        .any(|(bone_name, _)| bone_name == name.as_str())
                    {
                        bones.push((name.as_str().to_owned(), child));
                    }
                }
            }
            stack.push(child);
        }
    }
    bones
}

fn find_bone(bones: &[(String, Entity)], name: &str) -> Option<Entity> {
    bones
        .iter()
        .find(|(bone_name, _)| bone_name == name)
        .map(|(_, entity)| *entity)
}

fn arm_chain(bones: &[(String, Entity)], names: [&str; 4]) -> Option<ArmChain> {
    Some(ArmChain {
        upper: find_bone(bones, names[0])?,
        lower: find_bone(bones, names[1])?,
        hand: find_bone(bones, names[2])?,
        target: find_bone(bones, names[3])?,
    })
}

fn build_control_rig_bones(bones: &[(String, Entity)]) -> AlsControlRigBones {
    AlsControlRigBones {
        spine: SPINE_BONES.map(|name| find_bone(bones, name)),
        foot_root: find_bone(bones, FOOT_ROOT_BONE),
        left_arm: arm_chain(bones, LEFT_ARM_BONES),
        right_arm: arm_chain(bones, RIGHT_ARM_BONES),
    }
}

fn refresh_control_rig_bones(
    mut commands: Commands,
    characters: Query<(Entity, Option<&AlsControlRigBones>), With<AlsCharacter>>,
    children_q: Query<&Children>,
    name_q: Query<&Name>,
    target_q: Query<(), With<AnimationTargetId>>,
) {
    for (entity, existing) in &characters {
        if existing.is_some_and(AlsControlRigBones::ready) {
            continue;
        }
        let bones = collect_bones(entity, &children_q, &name_q, &target_q);
        let candidate = build_control_rig_bones(&bones);
        if candidate.ready() && existing != Some(&candidate) {
            commands.entity(entity).insert(candidate);
        }
    }
}

fn diagonal_scale_factor(forward_amount: f32, backward_amount: f32) -> f32 {
    1.0 - (2.0 * ((forward_amount + backward_amount).clamp(0.0, 1.0) - 0.5)).abs()
}

fn apply_diagonal_scaling(
    foot_root: Option<Entity>,
    forward_amount: f32,
    backward_amount: f32,
    pose_moving: f32,
    bone_locals: &mut Query<(&mut Transform, &GlobalTransform, &ChildOf), Without<AlsCharacter>>,
) {
    let Some(foot_root) = foot_root else {
        return;
    };
    let Ok((mut local, _, _)) = bone_locals.get_mut(foot_root) else {
        return;
    };
    let amount =
        diagonal_scale_factor(forward_amount, backward_amount) * pose_moving.clamp(0.0, 1.0);
    local.scale = Vec3::ONE + Vec3::new(0.4, 0.4, 0.0) * amount;
}

fn apply_arm_ik(
    chain: ArmChain,
    pole_offset: Vec3,
    weight: f32,
    bone_locals: &mut Query<(&mut Transform, &GlobalTransform, &ChildOf), Without<AlsCharacter>>,
    bone_globals: &Query<&GlobalTransform, Without<AlsCharacter>>,
) {
    if !is_relevant(weight) {
        return;
    }
    let (Ok(upper_global), Ok(lower_global), Ok(hand_global), Ok(target_global)) = (
        bone_globals.get(chain.upper),
        bone_globals.get(chain.lower),
        bone_globals.get(chain.hand),
        bone_globals.get(chain.target),
    ) else {
        return;
    };
    let upper = upper_global.translation();
    let lower = lower_global.translation();
    let hand = hand_global.translation();
    let target = target_global.translation();
    let pole = lower_global.affine().transform_point3(pole_offset) - upper;
    let upper_length = lower.distance(upper);
    let lower_length = hand.distance(lower);
    let Some(solve) =
        solve_two_bone_ik(upper, lower, hand, target, upper_length, lower_length, pole)
    else {
        return;
    };
    let weight = weight.clamp(0.0, 1.0);
    let upper_world =
        Quat::IDENTITY.slerp(solve.hip_rotation_world, weight) * upper_global.rotation();
    let lower_world =
        Quat::IDENTITY.slerp(solve.knee_rotation_world, weight) * lower_global.rotation();
    let Ok((_, _, upper_parent)) = bone_locals.get(chain.upper) else {
        return;
    };
    let parent_rotation = bone_globals
        .get(upper_parent.parent())
        .map(GlobalTransform::rotation)
        .unwrap_or(Quat::IDENTITY);
    let Ok((mut upper_local, _, _)) = bone_locals.get_mut(chain.upper) else {
        return;
    };
    upper_local.rotation = world_rotation_to_local(parent_rotation, upper_world);
    let Ok((mut lower_local, _, _)) = bone_locals.get_mut(chain.lower) else {
        return;
    };
    lower_local.rotation = world_rotation_to_local(upper_world, lower_world);
}

fn apply_spine_rotation(
    spine: [Option<Entity>; 4],
    yaw_degrees: f32,
    bone_locals: &mut Query<(&mut Transform, &GlobalTransform, &ChildOf), Without<AlsCharacter>>,
    bone_globals: &Query<&GlobalTransform, Without<AlsCharacter>>,
) {
    let Some(spine) = spine.into_iter().collect::<Option<Vec<_>>>() else {
        return;
    };
    if yaw_degrees.abs() <= 1e-4 {
        return;
    }
    let delta = Quat::IDENTITY.slerp(
        Quat::from_rotation_y(-yaw_degrees.to_radians()),
        SPINE_DELTA_FRACTION,
    );
    let mut accumulated = Quat::IDENTITY;
    let mut previous_bone = None;
    let mut previous_rotation = None;
    for bone in spine {
        let Ok((_, global, parent)) = bone_locals.get(bone) else {
            return;
        };
        if previous_bone.is_some_and(|previous| parent.parent() != previous) {
            return;
        }
        accumulated = delta * accumulated;
        let world_rotation = accumulated * global.rotation();
        let parent_rotation = if let Some(previous_rotation) = previous_rotation {
            previous_rotation
        } else {
            bone_globals
                .get(parent.parent())
                .map(GlobalTransform::rotation)
                .unwrap_or(Quat::IDENTITY)
        };
        let Ok((mut local, _, _)) = bone_locals.get_mut(bone) else {
            return;
        };
        local.rotation = world_rotation_to_local(parent_rotation, world_rotation);
        previous_bone = Some(bone);
        previous_rotation = Some(world_rotation);
    }
}

#[allow(clippy::type_complexity)]
fn control_rig_apply_system(
    time: Res<Time>,
    mut commands: Commands,
    mut characters: Query<
        (
            Entity,
            &LocomotionState,
            &RotationMode,
            Has<AlsFirstPersonView>,
            &Transform,
            &LinearVelocity,
            &AlsViewYaw,
            Option<&AlsControlRigBones>,
            Option<&mut AlsControlRigState>,
        ),
        With<AlsCharacter>,
    >,
    avatars: Query<(&ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    players: Query<&AnimationGraphPlayer>,
    mut bone_locals: Query<(&mut Transform, &GlobalTransform, &ChildOf), Without<AlsCharacter>>,
    bone_globals: Query<&GlobalTransform, Without<AlsCharacter>>,
) {
    for (
        entity,
        locomotion,
        rotation_mode,
        first_person,
        transform,
        velocity,
        view_yaw,
        bones,
        current_state,
    ) in &mut characters
    {
        if *locomotion == LocomotionState::Ragdolling {
            continue;
        }
        let Some(player) = avatars
            .iter()
            .find(|(parent, _)| parent.parent() == entity)
            .and_then(|(_, instance)| players.get(instance.player_entity()).ok())
        else {
            continue;
        };
        let mut state = current_state.as_deref().copied().unwrap_or_default();
        let body_yaw = body_yaw_degrees(transform);
        let view_yaw_world = view_yaw.0.to_degrees();
        let view_yaw_angle = unwind_degrees(view_yaw_world - body_yaw);
        let view_amount = 1.0 - output_f32(player, "ViewBlock").clamp(0.0, 1.0);
        let pose_aiming = output_f32(player, "PoseAiming").clamp(0.0, 1.0);
        update_spine_state(
            &mut state,
            SpineInput {
                allowed: *rotation_mode == RotationMode::Aiming || first_person,
                view_yaw_angle,
                view_yaw_world,
                body_yaw_world: body_yaw,
                blend_amount: view_amount * pose_aiming,
                delta_seconds: time.delta_secs(),
            },
        );
        if let Some(mut existing) = current_state {
            *existing = state;
        } else {
            commands.entity(entity).insert(state);
        }
        let Some(bones) = bones else {
            continue;
        };
        let (forward_amount, backward_amount) = velocity_blend_amounts(velocity.0, transform);
        apply_diagonal_scaling(
            bones.foot_root,
            forward_amount,
            backward_amount,
            output_f32(player, "PoseMoving"),
            &mut bone_locals,
        );
        if let Some(chain) = bones.left_arm {
            apply_arm_ik(
                chain,
                Vec3::new(0.0, 0.0, -HAND_POLE_OFFSET_METERS),
                output_f32(player, "LayerArmLeft").clamp(0.0, 1.0)
                    * output_f32(player, "HandLeftIk").clamp(0.0, 1.0),
                &mut bone_locals,
                &bone_globals,
            );
        }
        if let Some(chain) = bones.right_arm {
            apply_arm_ik(
                chain,
                Vec3::new(0.0, 0.0, HAND_POLE_OFFSET_METERS),
                output_f32(player, "LayerArmRight").clamp(0.0, 1.0)
                    * output_f32(player, "HandRightIk").clamp(0.0, 1.0),
                &mut bone_locals,
                &bone_globals,
            );
        }
        apply_spine_rotation(
            bones.spine,
            state.final_yaw_angle,
            &mut bone_locals,
            &bone_globals,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagonal_scaling_matches_the_control_rig_formula() {
        assert!((diagonal_scale_factor(1.0, 0.0) - 0.0).abs() < 1e-6);
        assert!((diagonal_scale_factor(0.5, 0.0) - 1.0).abs() < 1e-6);
        assert!((diagonal_scale_factor(0.0, 0.0) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn spine_enters_with_the_source_view_yaw() {
        let mut state = AlsControlRigState::default();
        update_spine_state(
            &mut state,
            SpineInput {
                allowed: true,
                view_yaw_angle: 90.0,
                view_yaw_world: 90.0,
                body_yaw_world: 0.0,
                blend_amount: 1.0,
                delta_seconds: 1.0 / 60.0,
            },
        );
        assert!((state.spine_amount - 1.0).abs() < 1e-6);
        assert!((state.final_yaw_angle - 90.0).abs() < 1e-6);
    }

    #[test]
    fn spine_blend_applies_after_the_yaw_state() {
        let mut state = AlsControlRigState::default();
        update_spine_state(
            &mut state,
            SpineInput {
                allowed: true,
                view_yaw_angle: 80.0,
                view_yaw_world: 80.0,
                body_yaw_world: 0.0,
                blend_amount: 0.25,
                delta_seconds: 1.0 / 60.0,
            },
        );
        assert!((state.yaw_angle - 80.0).abs() < 1e-6);
        assert!((state.final_yaw_angle - 20.0).abs() < 1e-6);
    }
}
