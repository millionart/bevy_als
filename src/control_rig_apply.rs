use als_core::character::AlsCharacter;
use als_core::foot_ik::{AlsFootLockState, AlsFootTargetSelection};
use als_core::input_request::AlsFirstPersonView;
use als_core::state::{LocomotionState, RotationMode};
use als_ext::ik::{solve_two_bone_ik, world_rotation_to_local};
use als_ext::source_instance::source_player_property;
use bevy::animation::AnimationTargetId;
use bevy::ecs::hierarchy::ChildOf;
use bevy::prelude::*;
use bevy::transform::{TransformSystems, systems as transform_systems};
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use serde::Deserialize;
use std::sync::Arc;
use avian3d::{collider_tree::ColliderTrees, prelude::{Collider, Position, Rotation, SpatialQueryFilter}};
use als_ext::{control_rig_hierarchy::RigTransform, control_rig_query::{RigQueryContext, RigQueryWorld},
    source_gameplay_tag_input::set_rig_query_context, source_instance::set_source_frame_input};

use crate::animation_context_projection::{AlsRootAnimationContext, AlsRootAnimationContextSet};
use crate::character_assembly::AlsCharacterAvatar;
use crate::foot_ik_apply::FootIkApplySet;
use crate::head_layer::{AlsHeadLayerSet, inv_exp_approx, unwind_degrees};

pub(crate) const CR_ALS_RUNTIME_PROFILE: &str =
    include_str!("../assets/control_rigs/cr_als.runtime.json");
const SPINE_ENTER_HALF_LIFE: f32 = 0.1;
const SPINE_EXIT_HALF_LIFE: f32 = 0.7;
const SPINE_REFERENCE_VIEW_YAW_SPEED: f32 = 40.0;
const SPINE_MAX_WORLD_OFFSET: f32 = 30.0;

#[derive(Clone, Deserialize)]
pub(crate) struct CrAlsRuntimeProfileDocument {
    pub(crate) construction: CrAlsRuntimeConstructionProfile,
    pub(crate) target: CrAlsTargetProfile,
    pub(crate) execution_order: Vec<String>,
}

#[derive(Clone, Deserialize)]
pub(crate) struct CrAlsRuntimeConstructionProfile {
    pub(crate) spine_rotation_bones: Vec<String>,
    pub(crate) foot_root_if_ik: String,
    pub(crate) foot_root_if_virtual: String,
    pub(crate) hand_left_if_ik: String,
    pub(crate) hand_left_if_virtual: String,
    pub(crate) hand_right_if_ik: String,
    pub(crate) hand_right_if_virtual: String,
}

#[derive(Clone, Deserialize)]
pub(crate) struct CrAlsTargetProfile {
    pub(crate) foot_initial_deltas_m: [[[f32; 3]; 2]; 2],
    pub(crate) foot_initial_scales: [[[f32; 3]; 2]; 2],
    pub(crate) foot_height_m: f32,
    pub(crate) leg_length_m: f32,
    pub(crate) diagonal_scale_delta: [f32; 3],
    pub(crate) hand_left_pole_location_m: [f32; 3],
    pub(crate) hand_right_pole_location_m: [f32; 3],
    pub(crate) foot_left_primary_axis: [f32; 3],
    pub(crate) foot_left_secondary_axis: [f32; 3],
    pub(crate) foot_right_primary_axis: [f32; 3],
    pub(crate) foot_right_secondary_axis: [f32; 3],
    pub(crate) foot_left_thigh_axis_pelvis_space: [f32; 3],
    pub(crate) foot_right_thigh_axis_pelvis_space: [f32; 3],
    pub(crate) foot_left_initial_rotation_calf_space: [f32; 4],
    pub(crate) foot_right_initial_rotation_calf_space: [f32; 4],
    pub(crate) trace_distance_up_m: f32,
    pub(crate) trace_distance_down_m: f32,
    pub(crate) walkable_floor_angle_degrees: f32,
    pub(crate) max_leg_stretch_ratio: f32,
    pub(crate) foot_offset_frequency_hz: f32,
    pub(crate) foot_offset_damping_ratio: f32,
    pub(crate) foot_offset_half_life_seconds: f32,
    pub(crate) pelvis_offset_frequency_hz: f32,
    pub(crate) pelvis_offset_damping_ratio: f32,
    pub(crate) pelvis_offset_min_m: f32,
    pub(crate) pelvis_offset_max_m: f32,
    pub(crate) pelvis_min_to_foot_standing_m: f32,
    pub(crate) pelvis_min_to_foot_moving_m: f32,
    pub(crate) foot_swing1_min_degrees: f32,
    pub(crate) foot_swing1_max_degrees: f32,
    pub(crate) foot_swing2_min_idle_degrees: f32,
    pub(crate) foot_swing2_max_idle_degrees: f32,
    pub(crate) foot_pole_distance_m: f32,
    pub(crate) foot_pole_half_life_seconds: f32,
}

#[derive(Resource, Clone)]
pub(crate) struct CrAlsRuntimeProfileTemplate(pub(crate) CrAlsRuntimeProfileDocument);

fn assert_profile_vec3(actual: [f32; 3], expected: Vec3) {
    assert!(Vec3::from(actual).distance(expected) < 1e-6);
}

fn validate_cr_als_runtime_profile(profile: &CrAlsRuntimeProfileDocument) {
    let execution_order = profile
        .execution_order
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    assert_eq!(
        execution_order,
        [
            "RefreshDiagonalScaling",
            "RefreshSpineRotation",
            "RefreshFootOffset",
            "RefreshPelvisOffset",
            "RefreshFootIk",
            "RefreshHandIk",
        ],
    );
    assert!((profile.target.foot_height_m - 0.134_657_34).abs() < 1e-6);
    assert!((profile.target.leg_length_m - 0.827_687_4).abs() < 1e-6);
    assert_profile_vec3(
        profile.target.diagonal_scale_delta,
        Vec3::new(0.4, 0.0, 0.4),
    );
    assert_profile_vec3(
        profile.target.hand_left_pole_location_m,
        Vec3::new(0.0, 0.0, -0.3),
    );
    assert_profile_vec3(
        profile.target.hand_right_pole_location_m,
        Vec3::new(0.0, 0.0, 0.3),
    );
    assert_profile_vec3(profile.target.foot_left_primary_axis, Vec3::NEG_X);
    assert_profile_vec3(profile.target.foot_left_secondary_axis, Vec3::Z);
    assert_profile_vec3(profile.target.foot_right_primary_axis, Vec3::X);
    assert_profile_vec3(profile.target.foot_right_secondary_axis, Vec3::NEG_Z);
    assert_profile_vec3(
        profile.target.foot_left_thigh_axis_pelvis_space,
        Vec3::new(-0.158_565_91, -0.985_633_85, 0.058_161_234),
    );
    assert_profile_vec3(
        profile.target.foot_right_thigh_axis_pelvis_space,
        Vec3::new(-0.158_546_39, 0.985_636_95, 0.058_161_905),
    );
}

impl FromWorld for CrAlsRuntimeProfileTemplate {
    fn from_world(_world: &mut World) -> Self {
        let profile: CrAlsRuntimeProfileDocument =
            serde_json::from_str(CR_ALS_RUNTIME_PROFILE).expect("CR_Als runtime profile parses");
        validate_cr_als_runtime_profile(&profile);
        Self(profile)
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsControlRigDiscoverySet;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsControlRigConstructionSet;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsControlRigInputSet;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsControlRigSet;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsControlRigPrePropagationSet;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsControlRigHandSet;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsControlRigPropagationSet;

pub struct ControlRigApplyPlugin;

impl Plugin for ControlRigApplyPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            PostUpdate,
            (
                AlsControlRigDiscoverySet
                    .before(AlsControlRigConstructionSet),
                AlsControlRigConstructionSet.before(FootIkApplySet::UpdateInputs),
                AlsControlRigInputSet
                    .after(AlsControlRigConstructionSet)
                    .after(AlsHeadLayerSet)
                    .before(als_ext::post_physics_animation::PostPhysicsAnimationSet::Evaluate),
                AlsControlRigSet.after(AlsControlRigInputSet)
                    .after(AnimationGraphSet::Final)
                    .after(AlsRootAnimationContextSet)
                    .after(TransformSystems::Propagate),
                AlsControlRigPrePropagationSet.after(AlsControlRigSet),
                AlsControlRigHandSet.after(FootIkApplySet::Propagate),
                als_ext::post_physics_animation::PostPhysicsAnimationSet::Readback
                    .after(AlsControlRigHandSet).after(FootIkApplySet::Telemetry),
                AlsControlRigPropagationSet.after(als_ext::post_physics_animation::PostPhysicsAnimationSet::Readback),
            ),
        )
        .init_resource::<CrAlsRuntimeProfileTemplate>()
        .add_systems(
            PostUpdate,
            refresh_control_rig_bones.in_set(AlsControlRigDiscoverySet),
        )
        .add_systems(
            PostUpdate,
            initialize_control_rig.in_set(AlsControlRigConstructionSet),
        )
        .add_systems(
            PostUpdate,
            (refresh_control_rig_input, publish_control_rig_inputs).chain().in_set(AlsControlRigInputSet),
        )
        .add_systems(
            PostUpdate,
            control_rig_pre_apply_system.in_set(AlsControlRigSet),
        )
        .add_systems(
            PostUpdate,
            (
                transform_systems::mark_dirty_trees,
                transform_systems::propagate_parent_transforms,
                transform_systems::sync_simple_transforms,
            )
                .chain()
                .in_set(AlsControlRigPrePropagationSet),
        )
        .add_systems(
            PostUpdate,
            control_rig_hand_apply_system.in_set(AlsControlRigHandSet),
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
    ik_target: Option<Entity>,
    virtual_target: Option<Entity>,
}

impl ArmChain {
    fn target(self, use_ik_bones: bool) -> Option<Entity> {
        if use_ik_bones {
            self.ik_target
        } else {
            self.virtual_target
        }
    }
}

#[derive(Component, Clone, Debug, Default, PartialEq)]
struct AlsControlRigBones {
    spine: [Option<Entity>; 4],
    foot_root_ik: Option<Entity>,
    foot_root_virtual: Option<Entity>,
    left_arm: Option<ArmChain>,
    right_arm: Option<ArmChain>,
}

impl AlsControlRigBones {
    fn ready(&self) -> bool {
        self.spine.iter().all(Option::is_some)
            && self.foot_root_ik.is_some()
            && self.foot_root_virtual.is_some()
            && self
                .left_arm
                .is_some_and(|chain| chain.ik_target.is_some() && chain.virtual_target.is_some())
            && self
                .right_arm
                .is_some_and(|chain| chain.ik_target.is_some() && chain.virtual_target.is_some())
    }

    fn foot_root(&self, use_ik_bones: bool) -> Option<Entity> {
        if use_ik_bones {
            self.foot_root_ik
        } else {
            self.foot_root_virtual
        }
    }
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct AlsControlRigSettings {
    pub use_hand_ik_bones: bool,
    pub use_foot_ik_bones: bool,
}

impl Default for AlsControlRigSettings {
    fn default() -> Self {
        Self {
            use_hand_ik_bones: true,
            use_foot_ik_bones: true,
        }
    }
}

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct AlsControlRigConstruction {
    pub use_hand_ik_bones: bool,
    pub use_foot_ik_bones: bool,
    pub foot_height_m: f32,
    pub leg_length_m: f32,
}

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct AlsControlRigInput {
    pub use_hand_ik_bones: bool,
    pub use_foot_ik_bones: bool,
    pub foot_transforms_valid: bool,
    pub velocity_blend_forward_amount: f32,
    pub velocity_blend_backward_amount: f32,
    pub spine_yaw_angle_degrees: f32,
    pub pelvis_offset_amount: f32,
    pub foot_left_location_armature: Vec3,
    pub foot_left_rotation_armature: Quat,
    pub foot_right_location_armature: Vec3,
    pub foot_right_rotation_armature: Quat,
}

impl Default for AlsControlRigInput {
    fn default() -> Self {
        Self {
            use_hand_ik_bones: true,
            use_foot_ik_bones: true,
            foot_transforms_valid: false,
            velocity_blend_forward_amount: 0.0,
            velocity_blend_backward_amount: 0.0,
            spine_yaw_angle_degrees: 0.0,
            pelvis_offset_amount: 0.0,
            foot_left_location_armature: Vec3::ZERO,
            foot_left_rotation_armature: Quat::IDENTITY,
            foot_right_location_armature: Vec3::ZERO,
            foot_right_rotation_armature: Quat::IDENTITY,
        }
    }
}

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AlsControlRigStageTrace {
    pub construction_complete: bool,
    pub initialization_count: u64,
    pub stage_mask: u8,
    pub completed_frames: u64,
}

impl AlsControlRigStageTrace {
    pub const DIAGONAL_SCALING: u8 = 1 << 0;
    pub const SPINE_ROTATION: u8 = 1 << 1;
    pub const FOOT_OFFSET: u8 = 1 << 2;
    pub const PELVIS_OFFSET: u8 = 1 << 3;
    pub const FOOT_IK: u8 = 1 << 4;
    pub const HAND_IK: u8 = 1 << 5;
    pub const COMPLETE: u8 = Self::DIAGONAL_SCALING
        | Self::SPINE_ROTATION
        | Self::FOOT_OFFSET
        | Self::PELVIS_OFFSET
        | Self::FOOT_IK
        | Self::HAND_IK;
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
        }
    }
}

#[derive(Clone, Copy)]
struct SpineInput {
    allowed: bool,
    pending_update: bool,
    view_yaw_angle: f32,
    view_yaw_speed: f32,
    body_yaw_world: f32,
    blend_amount: f32,
    delta_seconds: f32,
}

fn is_relevant(value: f32) -> bool {
    value > 1e-5
}

fn is_full(value: f32) -> bool {
    value >= 1.0 - 1e-5
}

fn damper_exact(current: f32, target: f32, delta_seconds: f32, half_life: f32) -> f32 {
    let alpha =
        1.0 - inv_exp_approx(std::f32::consts::LN_2 / (half_life + 1e-8) * delta_seconds);
    current + (target - current) * alpha
}

fn lerp_angle(from: f32, to: f32, amount: f32) -> f32 {
    let delta = unwind_degrees(to - from);
    unwind_degrees(from + (if delta > 175.0 { delta - 360.0 } else { delta }) * amount)
}

fn armature_yaw_rotation(yaw_degrees: f32) -> Quat {
    Quat::from_rotation_y(-yaw_degrees.to_radians())
}

fn update_spine_state(state: &mut AlsControlRigState, input: SpineInput) {
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
        if input.pending_update || is_full(state.spine_amount) {
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
    } else if input.pending_update || !is_relevant(state.spine_amount) {
        state.spine_amount = 0.0;
        state.yaw_angle = 0.0;
    } else {
        let multiplier = if input.view_yaw_speed > SPINE_REFERENCE_VIEW_YAW_SPEED {
            SPINE_REFERENCE_VIEW_YAW_SPEED / input.view_yaw_speed
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
    state.final_yaw_angle = lerp_angle(0.0, state.yaw_angle, input.blend_amount);
}

fn output_f32(context: Option<&AlsRootAnimationContext>, name: &str) -> f32 {
    context
        .and_then(|context| context.f32(name))
        .unwrap_or_default()
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

fn arm_chain(
    bones: &[(String, Entity)],
    upper: &str,
    lower: &str,
    hand: &str,
    ik_target: &str,
    virtual_target: &str,
) -> Option<ArmChain> {
    Some(ArmChain {
        upper: find_bone(bones, upper)?,
        lower: find_bone(bones, lower)?,
        hand: find_bone(bones, hand)?,
        ik_target: find_bone(bones, ik_target),
        virtual_target: find_bone(bones, virtual_target),
    })
}

fn build_control_rig_bones(
    bones: &[(String, Entity)],
    profile: &CrAlsRuntimeProfileDocument,
) -> AlsControlRigBones {
    let mut spine = [None; 4];
    for (index, name) in profile
        .construction
        .spine_rotation_bones
        .iter()
        .take(spine.len())
        .enumerate()
    {
        spine[index] = find_bone(bones, name);
    }
    AlsControlRigBones {
        spine,
        foot_root_ik: find_bone(bones, &profile.construction.foot_root_if_ik),
        foot_root_virtual: find_bone(bones, &profile.construction.foot_root_if_virtual),
        left_arm: arm_chain(
            bones,
            "upperarm_l",
            "lowerarm_l",
            "hand_l",
            &profile.construction.hand_left_if_ik,
            &profile.construction.hand_left_if_virtual,
        ),
        right_arm: arm_chain(
            bones,
            "upperarm_r",
            "lowerarm_r",
            "hand_r",
            &profile.construction.hand_right_if_ik,
            &profile.construction.hand_right_if_virtual,
        ),
    }
}

fn refresh_control_rig_bones(
    mut commands: Commands,
    characters: Query<(Entity, Option<&AlsControlRigBones>), With<AlsCharacter>>,
    children_q: Query<&Children>,
    name_q: Query<&Name>,
    target_q: Query<(), With<AnimationTargetId>>,
    profile: Res<CrAlsRuntimeProfileTemplate>,
) {
    for (entity, existing) in &characters {
        if existing.is_some_and(AlsControlRigBones::ready) {
            continue;
        }
        let bones = collect_bones(entity, &children_q, &name_q, &target_q);
        let candidate = build_control_rig_bones(&bones, &profile.0);
        if candidate.ready() && existing != Some(&candidate) {
            commands.entity(entity).insert(candidate);
        }
    }
}

fn initialize_control_rig(
    mut commands: Commands,
    characters: Query<
        (
            Entity,
            &AlsControlRigBones,
            Option<&AlsControlRigSettings>,
            Option<&AlsControlRigConstruction>,
        ),
        (With<AlsCharacter>, Added<AlsControlRigBones>),
    >,
    profile: Res<CrAlsRuntimeProfileTemplate>,
) {
    for (entity, bones, configured_settings, existing) in &characters {
        if existing.is_some() || !bones.ready() {
            continue;
        }
        let settings = configured_settings.copied().unwrap_or_default();
        let foot_root_ready = bones.foot_root(settings.use_foot_ik_bones).is_some();
        let hand_targets_ready = bones
            .left_arm
            .and_then(|chain| chain.target(settings.use_hand_ik_bones))
            .is_some()
            && bones
                .right_arm
                .and_then(|chain| chain.target(settings.use_hand_ik_bones))
                .is_some();
        if !foot_root_ready || !hand_targets_ready {
            continue;
        }
        commands.entity(entity).insert((
            settings,
            AlsControlRigConstruction {
                use_hand_ik_bones: settings.use_hand_ik_bones,
                use_foot_ik_bones: settings.use_foot_ik_bones,
                foot_height_m: profile.0.target.foot_height_m,
                leg_length_m: profile.0.target.leg_length_m,
            },
            AlsFootTargetSelection {
                use_ik_bones: settings.use_foot_ik_bones,
            },
            AlsControlRigInput::default(),
            AlsControlRigStageTrace {
                construction_complete: true,
                initialization_count: 1,
                stage_mask: 0,
                completed_frames: 0,
            },
        ));
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
    scale_delta: Vec3,
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
    local.scale += scale_delta * amount;
}

fn apply_arm_ik(
    chain: ArmChain,
    target: Entity,
    pole_offset: Vec3,
    weight: f32,
    bone_locals: &mut Query<(&mut Transform, &GlobalTransform, &ChildOf), Without<AlsCharacter>>,
    bone_globals: &Query<&GlobalTransform, Without<AlsCharacter>>,
) {
    if !(weight > 1e-4) {
        return;
    }
    let (Ok(upper_global), Ok(lower_global), Ok(hand_global), Ok(target_global)) = (
        bone_globals.get(chain.upper),
        bone_globals.get(chain.lower),
        bone_globals.get(chain.hand),
        bone_globals.get(target),
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
    armature_rotation: Quat,
    bone_locals: &mut Query<(&mut Transform, &GlobalTransform, &ChildOf), Without<AlsCharacter>>,
    bone_globals: &Query<&GlobalTransform, Without<AlsCharacter>>,
) {
    let Some(spine) = spine.into_iter().collect::<Option<Vec<_>>>() else {
        return;
    };
    if yaw_degrees.abs() <= 1e-4 {
        return;
    }
    let component_delta = Quat::IDENTITY.slerp(
        armature_yaw_rotation(yaw_degrees),
        (spine.len() as f32).recip(),
    );
    let delta = (armature_rotation * component_delta * armature_rotation.inverse()).normalize();
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

fn source_pelvis_offset_amount(player: &AnimationGraphPlayer, ground_prediction: f32) -> f32 {
    let amount = |field| player.get_global_input_data(field)
        .and_then(|value| value.as_f32().ok()).unwrap_or(0.0);
    (amount("pose_state.grounded_amount") + amount("pose_state.in_air_amount") * ground_prediction).clamp(0.0, 1.0)
}

#[allow(clippy::type_complexity)]
fn refresh_control_rig_input(
    time: Res<Time>,
    mut commands: Commands,
    mut characters: Query<
        (
            Entity,
            &RotationMode,
            Has<AlsFirstPersonView>,
            Option<&AlsControlRigSettings>,
            Option<&AlsFootLockState>,
            Option<&mut AlsControlRigState>,
            Option<&mut AlsControlRigInput>,
        ),
        With<AlsCharacter>,
    >,
    avatars: Query<(&ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    players: Query<(&AnimationGraphPlayer, Option<&AlsRootAnimationContext>)>,
) {
    for (
        entity,
        rotation_mode,
        first_person,
        settings,
        foot_lock,
        current_state,
        current_input,
    ) in &mut characters
    {
        let Some((player, context)) = avatars
            .iter()
            .find(|(parent, _)| parent.parent() == entity)
            .and_then(|(_, instance)| players.get(instance.player_entity()).ok())
        else {
            continue;
        };
        let scalar = |key| player.get_global_input_data(key)
            .and_then(|value| value.as_f32().ok()).unwrap_or(0.0);
        let mut state = current_state.as_deref().copied().unwrap_or_default();
        update_spine_state(&mut state, SpineInput {
            allowed: *rotation_mode == RotationMode::Aiming || first_person,
            pending_update: player.get_global_input_data("animation.pending_update")
                .and_then(|value| value.as_bool().ok()).unwrap_or(true),
            view_yaw_angle: scalar("view_state.yaw_angle"),
            view_yaw_speed: scalar("view_state.yaw_speed"),
            body_yaw_world: scalar("locomotion_state.rotation_yaw"),
            blend_amount: (1.0 - output_f32(context, "ViewBlock").clamp(0.0, 1.0))
                * output_f32(context, "PoseAiming").clamp(0.0, 1.0),
            delta_seconds: time.delta_secs(),
        });
        if let Some(mut existing) = current_state {
            *existing = state;
        } else {
            commands.entity(entity).insert(state);
        }
        let Some(mut current_input) = current_input else { continue; };
        let source_scalar = |key: &str| source_player_property(player, key)
            .and_then(|value| value.as_f32().ok()).unwrap_or(0.0);
        let pelvis_offset_amount = source_pelvis_offset_amount(player,
            source_scalar("in_air_state.ground_prediction_amount"));
        let velocity_blend = ["forward_amount", "backward_amount"].map(|direction| {
            source_scalar(&format!("grounded_state.velocity_blend.{direction}"))
        });
        let settings = settings.copied().unwrap_or_default();
        let input = AlsControlRigInput {
            use_hand_ik_bones: settings.use_hand_ik_bones,
            use_foot_ik_bones: settings.use_foot_ik_bones,
            foot_transforms_valid: foot_lock.is_some_and(|state| state.valid) && current_input.foot_transforms_valid,
            velocity_blend_forward_amount: velocity_blend[0],
            velocity_blend_backward_amount: velocity_blend[1],
            spine_yaw_angle_degrees: state.final_yaw_angle,
            pelvis_offset_amount,
            ..*current_input
        };
        *current_input = input;
    }
}

fn publish_control_rig_inputs(
    characters: Query<(Entity, &AlsControlRigInput), With<AlsCharacter>>,
    avatars: Query<(Entity, &ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    mut players: Query<&mut AnimationGraphPlayer>,
    transforms: TransformHelper,
    trees: Option<Res<ColliderTrees>>,
    colliders: Query<(Entity, &Position, &Rotation, &Collider)>,
) {
    let mut ignored: Vec<_> = characters.iter().map(|(entity, _)| entity).collect();
    for (_, parent, instance) in &avatars {
        if characters.contains(parent.parent())
            && let Ok(player) = players.get(instance.player_entity())
            && let Some(ragdoll) = player.get_spawned_ragdoll() {
            ignored.extend(ragdoll.bodies.values().copied());
            ignored.extend(ragdoll.colliders.values().copied());
        }
    }
    let filter = SpatialQueryFilter::from_excluded_entities(ignored);
    let mut world: Option<Arc<RigQueryWorld>> = None;
    for (entity, input) in &characters {
        let Some((avatar, _, instance)) = avatars.iter().find(|(_, parent, _)| parent.parent() == entity) else { continue; };
        let root = instance.player_entity();
        let Ok(mut player) = players.get_mut(root) else { continue; };
        use bevy_animation_graph::core::edge_data::DataValue;
        for (field, value) in [
            ("b_use_hand_ik_bones", DataValue::Bool(input.use_hand_ik_bones)),
            ("b_use_foot_ik_bones", DataValue::Bool(input.use_foot_ik_bones)),
            ("b_foot_transforms_valid", DataValue::Bool(input.foot_transforms_valid)),
            ("velocity_blend_forward_amount", DataValue::F32(input.velocity_blend_forward_amount)),
            ("velocity_blend_backward_amount", DataValue::F32(input.velocity_blend_backward_amount)),
            ("spine_yaw_angle", DataValue::F32(-input.spine_yaw_angle_degrees)),
            ("pelvis_offset_amount", DataValue::F32(input.pelvis_offset_amount)),
            ("foot_left_location", DataValue::Vec3(input.foot_left_location_armature)),
            ("foot_left_rotation", DataValue::Quat(input.foot_left_rotation_armature)),
            ("foot_right_location", DataValue::Vec3(input.foot_right_location_armature)),
            ("foot_right_rotation", DataValue::Quat(input.foot_right_rotation_armature)),
        ] {
            set_source_frame_input(&mut player, &format!("control_rig_input.{field}"), value);
        }
        set_source_frame_input(&mut player, als_ext::root_graph_boundaries::RIG_GAME_WORLD_KEY, true);
        let context = if input.foot_transforms_valid {
            trees.as_ref().and_then(|trees| transforms.compute_global_transform(avatar).ok().map(|transform| {
                let world = world.get_or_insert_with(|| Arc::new(RigQueryWorld {
                    trees: (**trees).clone(), colliders: colliders.iter().map(|(entity, position, rotation, collider)|
                        (entity, (*position, *rotation, collider.clone()))).collect(),
                })).clone();
                let transform = transform.compute_transform();
                RigQueryContext { world, filter: filter.clone(), channel: "ECC_Visibility".to_owned(),
                    to_world: RigTransform { translation: transform.translation.as_dvec3(), rotation: transform.rotation.as_dquat(), scale: transform.scale.as_dvec3() } }
            }))
        } else { None };
        set_rig_query_context(root, context);
    }
}

#[allow(clippy::type_complexity)]
fn control_rig_pre_apply_system(
    characters: Query<
        (
            Entity,
            &LocomotionState,
            Option<&AlsControlRigBones>,
            Option<&AlsControlRigInput>,
        ),
        With<AlsCharacter>,
    >,
    avatars: Query<(&ChildOf, &AnimatedSceneInstance, &GlobalTransform), With<AlsCharacterAvatar>>,
    players: Query<(&AnimationGraphPlayer, Option<&AlsRootAnimationContext>)>,
    profile: Res<CrAlsRuntimeProfileTemplate>,
    mut bone_locals: Query<(&mut Transform, &GlobalTransform, &ChildOf), Without<AlsCharacter>>,
    bone_globals: Query<&GlobalTransform, Without<AlsCharacter>>,
    mut traces: Query<&mut AlsControlRigStageTrace>,
) {
    for (
        entity,
        locomotion,
        bones,
        input,
    ) in &characters
    {
        if let Ok(mut trace) = traces.get_mut(entity) { trace.stage_mask = 0; }
        if *locomotion == LocomotionState::Ragdolling {
            continue;
        }
        let Some((_, context)) = avatars
            .iter()
            .find(|(parent, _, _)| parent.parent() == entity)
            .and_then(|(_, instance, _)| players.get(instance.player_entity()).ok())
        else {
            continue;
        };
        let Some(bones) = bones else {
            continue;
        };
        let input = input.copied().unwrap_or_default();
        apply_diagonal_scaling(
            bones.foot_root(input.use_foot_ik_bones),
            input.velocity_blend_forward_amount,
            input.velocity_blend_backward_amount,
            output_f32(context, "PoseMoving"),
            Vec3::from(profile.0.target.diagonal_scale_delta),
            &mut bone_locals,
        );
        apply_spine_rotation(
            bones.spine,
            input.spine_yaw_angle_degrees,
            avatars
                .iter()
                .find(|(parent, _, _)| parent.parent() == entity)
                .map(|(_, _, armature)| armature.rotation())
                .unwrap_or(Quat::IDENTITY),
            &mut bone_locals,
            &bone_globals,
        );
        if let Ok(mut trace) = traces.get_mut(entity) {
            trace.stage_mask |=
                AlsControlRigStageTrace::DIAGONAL_SCALING | AlsControlRigStageTrace::SPINE_ROTATION;
        }
    }
}

#[allow(clippy::type_complexity)]
fn control_rig_hand_apply_system(
    characters: Query<
        (
            Entity,
            &LocomotionState,
            Option<&AlsControlRigBones>,
            Option<&AlsControlRigInput>,
        ),
        With<AlsCharacter>,
    >,
    avatars: Query<(&ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    players: Query<(&AnimationGraphPlayer, Option<&AlsRootAnimationContext>)>,
    profile: Res<CrAlsRuntimeProfileTemplate>,
    mut bone_locals: Query<(&mut Transform, &GlobalTransform, &ChildOf), Without<AlsCharacter>>,
    bone_globals: Query<&GlobalTransform, Without<AlsCharacter>>,
    mut traces: Query<&mut AlsControlRigStageTrace>,
) {
    for (entity, locomotion, bones, input) in &characters {
        if *locomotion == LocomotionState::Ragdolling {
            continue;
        }
        let Some((_, context)) = avatars
            .iter()
            .find(|(parent, _)| parent.parent() == entity)
            .and_then(|(_, instance)| players.get(instance.player_entity()).ok())
        else {
            continue;
        };
        let (Some(bones), input) = (bones, input.copied().unwrap_or_default()) else {
            continue;
        };
        if let Some(chain) = bones.left_arm {
            if let Some(target) = chain.target(input.use_hand_ik_bones) {
                apply_arm_ik(
                    chain,
                    target,
                    Vec3::from(profile.0.target.hand_left_pole_location_m),
                    output_f32(context, "LayerArmLeft").clamp(0.0, 1.0)
                        * output_f32(context, "HandLeftIk").clamp(0.0, 1.0),
                    &mut bone_locals,
                    &bone_globals,
                );
            }
        }
        if let Some(chain) = bones.right_arm {
            if let Some(target) = chain.target(input.use_hand_ik_bones) {
                apply_arm_ik(
                    chain,
                    target,
                    Vec3::from(profile.0.target.hand_right_pole_location_m),
                    output_f32(context, "LayerArmRight").clamp(0.0, 1.0)
                        * output_f32(context, "HandRightIk").clamp(0.0, 1.0),
                    &mut bone_locals,
                    &bone_globals,
                );
            }
        }
        if let Ok(mut trace) = traces.get_mut(entity) {
            trace.stage_mask |= AlsControlRigStageTrace::HAND_IK;
            if trace.stage_mask == AlsControlRigStageTrace::COMPLETE {
                trace.completed_frames += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use als_core::motion::AlsViewYaw;
    use crate::locomotion_layer::AlsGroundPredictionState;

    #[test]
    fn rig_input_snapshots_prepared_actor_state_before_source_update() {
        use als_ext::{post_physics_animation::PostPhysicsAnimationSet, source_instance::set_source_frame_input};
        use bevy::world_serialization::{WorldAsset, WorldInstanceReady, WorldInstanceSpawner};
        use bevy_animation_graph::core::animated_scene::AnimatedSceneHandle;

        fn source_update(mut players: Query<(&mut AnimationGraphPlayer, &mut AlsRootAnimationContext)>) {
            for (mut player, mut context) in &mut players {
                for (key, value) in [
                    ("grounded_state.velocity_blend.forward_amount", 0.9),
                    ("grounded_state.velocity_blend.backward_amount", 0.05),
                    ("in_air_state.ground_prediction_amount", 0.8),
                ] {
                    set_source_frame_input(&mut player, key, value);
                }
                context.curves.insert("ViewBlock".into(), 1.0);
                context.curves.insert("PoseAiming".into(), 0.0);
            }
        }

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin,
            bevy_animation_graph::AnimationGraphPlugin::default()))
            .init_resource::<ColliderTrees>();
        let actor = app.world_mut().spawn((
            AlsCharacter, LocomotionState::Grounded, RotationMode::Aiming,
            Transform::default(), AlsViewYaw(60.0_f32.to_radians()),
            AlsControlRigConstruction {
                use_hand_ik_bones: true, use_foot_ik_bones: true,
                foot_height_m: 0.1, leg_length_m: 0.8,
            },
            AlsFootLockState { valid: true, ..default() },
            AlsGroundPredictionState { amount: 0.95 },
            AlsControlRigInput {
                foot_transforms_valid: true,
                foot_left_location_armature: Vec3::new(1.0, 2.0, 3.0),
                ..default()
            },
            AlsControlRigStageTrace::default(),
        )).id();
        let avatar = app.world_mut().spawn((AlsCharacterAvatar { character: actor },
            AnimatedSceneHandle::default(), ChildOf(actor))).id();
        let avatar_transform = Transform::from_xyz(4.0, 1.0, 2.0)
            .with_rotation(Quat::from_rotation_y(0.4)).with_scale(Vec3::new(0.5, 2.0, 3.0));
        app.world_mut().entity_mut(avatar).insert(avatar_transform);
        let player = app.world_mut().spawn((AnimationGraphPlayer::default(),
            AlsRootAnimationContext::default(), Transform::default(), ChildOf(avatar))).id();
        let mut parent = avatar;
        let spine = std::array::from_fn::<_, 4, _>(|_| {
            let bone = app.world_mut().spawn((Transform::default(), GlobalTransform::default(), ChildOf(parent))).id();
            parent = bone;
            Some(bone)
        });
        app.world_mut().entity_mut(actor).insert(AlsControlRigBones { spine, ..default() });
        app.world_mut().trigger(WorldInstanceReady { entity: avatar,
            instance_id: WorldInstanceSpawner::default().spawn(Handle::<WorldAsset>::default()) });
        app.world_mut().flush();
        assert_eq!(app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity(), player);
        app.world_mut().resource_mut::<bevy::ecs::schedule::Schedules>().remove(PostUpdate);
        app.add_plugins(ControlRigApplyPlugin)
            .configure_sets(PostUpdate, PostPhysicsAnimationSet::Evaluate.before(AnimationGraphSet::Final))
            .add_systems(PostUpdate, source_update.in_set(PostPhysicsAnimationSet::Evaluate));
        for (locomotion, mode, first_person, view_angle) in [
            (LocomotionState::Grounded, RotationMode::Aiming, false, -60.0),
            (LocomotionState::Ragdolling, RotationMode::Aiming, false, -35.0),
            (LocomotionState::Grounded, RotationMode::ViewDirection, true, 45.0),
        ] {
            app.world_mut().entity_mut(actor).insert((locomotion, mode));
            for bone in spine.into_iter().flatten() {
                app.world_mut().entity_mut(bone).insert((Transform::default(), GlobalTransform::default()));
            }
            if first_person { app.world_mut().entity_mut(actor).insert(AlsFirstPersonView); }
            {
                let mut player = app.world_mut().get_mut::<AnimationGraphPlayer>(player).unwrap();
                for (key, value) in [
                    ("grounded_state.velocity_blend.forward_amount", 0.25),
                    ("grounded_state.velocity_blend.backward_amount", 0.1),
                    ("in_air_state.ground_prediction_amount", 0.2),
                ] { set_source_frame_input(&mut player, key, value); }
                for (key, value) in [
                    ("pose_state.grounded_amount", 0.25), ("pose_state.in_air_amount", 0.75),
                    ("view_state.yaw_angle", view_angle), ("view_state.yaw_speed", 120.0),
                    ("locomotion_state.rotation_yaw", -10.0),
                ] { player.set_global_input_data(key, value); }
                player.set_global_input_data("animation.pending_update", true);
            }
            app.world_mut().get_mut::<AlsRootAnimationContext>(player).unwrap().curves =
                [("ViewBlock".into(), 0.25), ("PoseAiming".into(), 0.5)].into_iter().collect();
            app.world_mut().run_schedule(PostUpdate);
            let input = app.world().get::<AlsControlRigInput>(actor).unwrap();
            assert_eq!((input.velocity_blend_forward_amount, input.velocity_blend_backward_amount), (0.25, 0.1));
            assert!((input.pelvis_offset_amount - 0.4).abs() < 1e-6, "{input:?}");
            assert!((input.spine_yaw_angle_degrees - view_angle * 0.375).abs() < 1e-6, "{input:?}");
            assert_eq!(input.foot_left_location_armature, Vec3::new(1.0, 2.0, 3.0));
            assert!(input.foot_transforms_valid);
            let published = app.world().get::<AnimationGraphPlayer>(player).unwrap();
            assert_eq!(source_player_property(published, "control_rig_input.velocity_blend_forward_amount").and_then(|value| value.as_f32().ok()), Some(0.25));
            assert_eq!(source_player_property(published, "control_rig_input.foot_left_location").and_then(|value| value.as_vec3().ok()), Some(Vec3::new(1.0, 2.0, 3.0)));
            assert_eq!(source_player_property(published, "control_rig_input.spine_yaw_angle").and_then(|value| value.as_f32().ok()), Some(-input.spine_yaw_angle_degrees));
            let query = als_ext::source_gameplay_tag_input::rig_query_context(player).unwrap();
            let avatar_world = GlobalTransform::from(avatar_transform).compute_transform();
            assert_eq!(query.to_world.translation, avatar_world.translation.as_dvec3());
            assert_eq!(query.to_world.rotation, avatar_world.rotation.as_dquat());
            assert_eq!(query.to_world.scale, avatar_world.scale.as_dvec3());
            assert!(query.filter.excluded_entities.contains(&actor));
            assert_eq!(app.world().get::<AlsControlRigState>(actor).unwrap().final_yaw_angle,
                input.spine_yaw_angle_degrees);
            let expected = if locomotion == LocomotionState::Ragdolling { Quat::IDENTITY }
                else { Quat::from_rotation_y((-input.spine_yaw_angle_degrees / 4.0).to_radians()) };
            let actual = app.world().get::<Transform>(spine[0].unwrap()).unwrap().rotation;
            assert!(actual.abs_diff_eq(expected, 1e-6), "{locomotion:?}: {actual:?} != {expected:?}");
        }
        app.world_mut().get_mut::<AlsControlRigInput>(actor).unwrap().foot_transforms_valid = false;
        app.world_mut().run_schedule(PostUpdate);
        assert!(als_ext::source_gameplay_tag_input::rig_query_context(player).is_none());
    }

    #[test]
    fn spine_interpolation_uses_source_counterclockwise_and_weight_boundaries() {
        assert!((lerp_angle(0.0, 179.0, 0.5) + 90.5).abs() < 1e-6);
        assert!((lerp_angle(0.0, 175.0, 0.5) - 87.5).abs() < 1e-6);
        assert!(is_relevant(0.000_02));
        assert!(!is_full(0.999_98));
    }

    #[test]
    fn pelvis_offset_reads_the_prepared_actor_pose_inputs() {
        let mut player = AnimationGraphPlayer::default();
        for (grounded, in_air, prediction, expected) in [
            (1.0_f32, 0.0_f32, 0.0_f32, 1.0_f32),
            (0.25, 0.75, 0.4, 0.55),
            (0.0, 1.0, 0.0, 0.0),
        ] {
            player.set_global_input_data("pose_state.grounded_amount", grounded);
            player.set_global_input_data("pose_state.in_air_amount", in_air);
            assert!((source_pelvis_offset_amount(&player, prediction) - expected).abs() < 1e-6);
        }
    }

    #[test]
    fn diagonal_scaling_matches_the_control_rig_formula() {
        assert!((diagonal_scale_factor(1.0, 0.0) - 0.0).abs() < 1e-6);
        assert!((diagonal_scale_factor(0.5, 0.0) - 1.0).abs() < 1e-6);
        assert!((diagonal_scale_factor(0.0, 0.0) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn diagonal_scaling_consumes_the_current_source_pose_scale() {
        use bevy::ecs::system::RunSystemOnce;
        fn apply(
            In(bone): In<Entity>,
            mut locals: Query<(&mut Transform, &GlobalTransform, &ChildOf), Without<AlsCharacter>>,
        ) {
            apply_diagonal_scaling(Some(bone), 0.5, 0.0, 1.0, Vec3::new(0.4, 0.0, 0.4), &mut locals);
        }
        let mut world = World::new();
        let parent = world.spawn_empty().id();
        let bone = world.spawn((
            Transform::from_scale(Vec3::splat(2.0)), GlobalTransform::default(), ChildOf(parent),
        )).id();
        for input_scale in [2.0, 1.0, 3.0, 1.0] {
            world.get_mut::<Transform>(bone).unwrap().scale = Vec3::splat(input_scale);
            world.run_system_once_with(apply, bone).unwrap();
            assert_eq!(world.get::<Transform>(bone).unwrap().scale, Vec3::new(input_scale + 0.4, input_scale, input_scale + 0.4));
        }
    }

    #[test]
    fn spine_uses_the_actor_pending_update_and_view_speed() {
        let mut state = AlsControlRigState::default();
        let mut input = SpineInput {
            allowed: true, pending_update: false, view_yaw_angle: 80.0,
            view_yaw_speed: 0.0, body_yaw_world: 0.0,
            blend_amount: 0.25, delta_seconds: 0.1,
        };
        update_spine_state(&mut state, input);
        assert!(state.spine_amount > 0.49 && state.spine_amount < 0.51);
        input.pending_update = true;
        update_spine_state(&mut state, input);
        assert!((state.spine_amount - 1.0).abs() < 1e-6);
        assert!((state.yaw_angle - 80.0).abs() < 1e-6);
        assert!((state.final_yaw_angle - 20.0).abs() < 1e-6);
        input.allowed = false;
        input.pending_update = false;
        input.view_yaw_speed = 3580.0;
        update_spine_state(&mut state, input);
        let x = std::f32::consts::LN_2 / (0.7 * 40.0 / 3580.0 + 1e-8) * 0.1;
        let expected = 1.0 / (1.0 + 1.00746054 * x + 0.45053901 * x * x + 0.25724632 * x * x * x);
        assert!((state.spine_amount - expected).abs() < 1e-6);
        assert!((state.final_yaw_angle - 20.0 * expected).abs() < 1e-5);
        input.pending_update = true;
        update_spine_state(&mut state, input);
        assert_eq!((state.spine_amount, state.yaw_angle, state.final_yaw_angle), (0.0, 0.0, 0.0));
    }
}
